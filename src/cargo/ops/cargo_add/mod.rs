//! Core of cargo-add command

mod crate_spec;
mod dependency;
mod manifest;

use anyhow::Context;
use std::collections::BTreeSet;
use std::collections::VecDeque;
use std::path::Path;

use cargo_util::paths;
use indexmap::IndexSet;
use termcolor::Color::Green;
use termcolor::Color::Red;
use termcolor::ColorSpec;
use toml_edit::Item as TomlItem;

use crate::core::dependency::DepKind;
use crate::core::registry::PackageRegistry;
use crate::core::Package;
use crate::core::Registry;
use crate::core::Shell;
use crate::core::Workspace;
use crate::CargoResult;
use crate::Config;
use crate_spec::CrateSpec;
use dependency::Dependency;
use dependency::GitSource;
use dependency::PathSource;
use dependency::RegistrySource;
use dependency::Source;
use manifest::LocalManifest;

use crate::ops::cargo_add::dependency::MaybeWorkspace;
pub use manifest::DepTable;

/// Information on what dependencies should be added
#[derive(Clone, Debug)]
pub struct AddOptions<'a> {
    /// Configuration information for cargo operations
    pub config: &'a Config,
    /// Package to add dependencies to
    pub spec: &'a Package,
    /// Dependencies to add or modify
    pub dependencies: Vec<DepOp>,
    /// Which dependency section to add these to
    pub section: DepTable,
    /// Act as if dependencies will be added
    pub dry_run: bool,
}

/// Add dependencies to a manifest
pub fn add(workspace: &Workspace<'_>, options: &AddOptions<'_>) -> CargoResult<()> {
    let dep_table = options
        .section
        .to_table()
        .into_iter()
        .map(String::from)
        .collect::<Vec<_>>();

    let manifest_path = options.spec.manifest_path().to_path_buf();
    let mut manifest = LocalManifest::try_new(&manifest_path)?;
    let legacy = manifest.get_legacy_sections();
    if !legacy.is_empty() {
        anyhow::bail!(
            "Deprecated dependency sections are unsupported: {}",
            legacy.join(", ")
        );
    }

    let mut registry = PackageRegistry::new(options.config)?;

    let deps = {
        let _lock = options.config.acquire_package_cache_lock()?;
        registry.lock_patches();
        options
            .dependencies
            .iter()
            .map(|raw| {
                resolve_dependency(
                    &manifest,
                    raw,
                    workspace,
                    &options.section,
                    options.config,
                    &mut registry,
                )
            })
            .collect::<CargoResult<Vec<_>>>()?
    };

    let was_sorted = manifest
        .get_table(&dep_table)
        .map(TomlItem::as_table)
        .map_or(true, |table_option| {
            table_option.map_or(true, |table| is_sorted(table.iter().map(|(name, _)| name)))
        });
    for dep in deps {
        print_msg(&mut options.config.shell(), &dep, &dep_table)?;
        if let Some(Source::Path(src)) = dep.source() {
            if src.path == manifest.path.parent().unwrap_or_else(|| Path::new("")) {
                anyhow::bail!(
                    "cannot add `{}` as a dependency to itself",
                    manifest.package_name()?
                )
            }
        }

        let available_features = dep
            .available_features
            .keys()
            .map(|s| s.as_ref())
            .collect::<BTreeSet<&str>>();
        let mut unknown_features: Vec<&str> = Vec::new();
        if let Some(req_feats) = dep.features.as_ref() {
            let req_feats: BTreeSet<_> = req_feats.iter().map(|s| s.as_str()).collect();
            unknown_features.extend(req_feats.difference(&available_features).copied());
        }
        if let Some(inherited_features) = dep.inherited_features.as_ref() {
            let inherited_features: BTreeSet<_> =
                inherited_features.iter().map(|s| s.as_str()).collect();
            unknown_features.extend(inherited_features.difference(&available_features).copied());
        }
        unknown_features.sort();
        if !unknown_features.is_empty() {
            anyhow::bail!("unrecognized features: {unknown_features:?}");
        }

        manifest.insert_into_table(&dep_table, &dep)?;
        manifest.gc_dep(dep.toml_key());
    }

    if was_sorted {
        if let Some(table) = manifest
            .get_table_mut(&dep_table)
            .ok()
            .and_then(TomlItem::as_table_like_mut)
        {
            table.sort_values();
        }
    }

    if options.dry_run {
        options.config.shell().warn("aborting add due to dry run")?;
    } else {
        manifest.write()?;
    }

    Ok(())
}

/// Dependency entry operation
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DepOp {
    /// Describes the crate
    pub crate_spec: Option<String>,
    /// Dependency key, overriding the package name in crate_spec
    pub rename: Option<String>,

    /// Feature flags to activate
    pub features: Option<IndexSet<String>>,
    /// Whether the default feature should be activated
    pub default_features: Option<bool>,

    /// Whether dependency is optional
    pub optional: Option<bool>,

    /// Registry for looking up dependency version
    pub registry: Option<String>,

    /// Git repo for dependency
    pub path: Option<String>,
    /// Git repo for dependency
    pub git: Option<String>,
    /// Specify an alternative git branch
    pub branch: Option<String>,
    /// Specify a specific git rev
    pub rev: Option<String>,
    /// Specify a specific git tag
    pub tag: Option<String>,
}

fn resolve_dependency(
    manifest: &LocalManifest,
    arg: &DepOp,
    ws: &Workspace<'_>,
    section: &DepTable,
    config: &Config,
    registry: &mut PackageRegistry<'_>,
) -> CargoResult<Dependency> {
    let crate_spec = arg
        .crate_spec
        .as_deref()
        .map(CrateSpec::resolve)
        .transpose()?;
    let mut selected_dep = if let Some(url) = &arg.git {
        let mut src = GitSource::new(url);
        if let Some(branch) = &arg.branch {
            src = src.set_branch(branch);
        }
        if let Some(tag) = &arg.tag {
            src = src.set_tag(tag);
        }
        if let Some(rev) = &arg.rev {
            src = src.set_rev(rev);
        }

        let selected = if let Some(crate_spec) = &crate_spec {
            if let Some(v) = crate_spec.version_req() {
                // crate specifier includes a version (e.g. `docopt@0.8`)
                anyhow::bail!("cannot specify a git URL (`{url}`) with a version (`{v}`).");
            }
            let dependency = crate_spec.to_dependency()?.set_source(src);
            let selected = select_package(&dependency, config, registry)?;
            if dependency.name != selected.name {
                config.shell().warn(format!(
                    "translating `{}` to `{}`",
                    dependency.name, selected.name,
                ))?;
            }
            selected
        } else {
            let mut source = crate::sources::GitSource::new(src.source_id()?, config)?;
            let packages = source.read_packages()?;
            let package = infer_package(packages, &src)?;
            Dependency::from(package.summary())
        };
        selected
    } else if let Some(raw_path) = &arg.path {
        let path = paths::normalize_path(&std::env::current_dir()?.join(raw_path));
        let src = PathSource::new(&path);

        let selected = if let Some(crate_spec) = &crate_spec {
            if let Some(v) = crate_spec.version_req() {
                // crate specifier includes a version (e.g. `docopt@0.8`)
                anyhow::bail!("cannot specify a path (`{raw_path}`) with a version (`{v}`).");
            }
            let dependency = crate_spec.to_dependency()?.set_source(src);
            let selected = select_package(&dependency, config, registry)?;
            if dependency.name != selected.name {
                config.shell().warn(format!(
                    "translating `{}` to `{}`",
                    dependency.name, selected.name,
                ))?;
            }
            selected
        } else {
            let source = crate::sources::PathSource::new(&path, src.source_id()?, config);
            let packages = source.read_packages()?;
            let package = infer_package(packages, &src)?;
            Dependency::from(package.summary())
        };
        selected
    } else if let Some(crate_spec) = &crate_spec {
        crate_spec.to_dependency()?
    } else {
        anyhow::bail!("dependency name is required");
    };
    selected_dep = populate_dependency(selected_dep, arg);

    let old_dep = get_existing_dependency(manifest, selected_dep.toml_key(), section)?;
    let mut dependency = if let Some(mut old_dep) = old_dep.clone() {
        if old_dep.name != selected_dep.name {
            // Assuming most existing keys are not relevant when the package changes
            if selected_dep.optional.is_none() {
                selected_dep.optional = old_dep.optional;
            }
            selected_dep
        } else {
            if selected_dep.source().is_some() {
                // Overwrite with `crate_spec`
                old_dep.source = selected_dep.source;
            }
            old_dep = populate_dependency(old_dep, arg);
            old_dep.available_features = selected_dep.available_features;
            old_dep
        }
    } else {
        selected_dep
    };

    if dependency.source().is_none() {
        if let Some(package) = ws.members().find(|p| p.name().as_str() == dependency.name) {
            // Only special-case workspaces when the user doesn't provide any extra
            // information, otherwise, trust the user.
            let mut src = PathSource::new(package.root());
            // dev-dependencies do not need the version populated
            if section.kind() != DepKind::Development {
                let op = "";
                let v = format!("{op}{version}", version = package.version());
                src = src.set_version(v);
            }
            dependency = dependency.set_source(src);
        } else {
            let latest = get_latest_dependency(&dependency, false, config, registry)?;

            if dependency.name != latest.name {
                config.shell().warn(format!(
                    "translating `{}` to `{}`",
                    dependency.name, latest.name,
                ))?;
                dependency.name = latest.name; // Normalize the name
            }
            dependency = dependency
                .set_source(latest.source.expect("latest always has a source"))
                .set_available_features(latest.available_features);
        }
    }

    let version_required = dependency.source().and_then(|s| s.as_registry()).is_some();
    let version_optional_in_section = section.kind() == DepKind::Development;
    let preserve_existing_version = old_dep
        .as_ref()
        .map(|d| d.version().is_some())
        .unwrap_or(false);
    if !version_required && !preserve_existing_version && version_optional_in_section {
        // dev-dependencies do not need the version populated
        dependency = dependency.clear_version();
    }

    dependency = populate_available_features(dependency, config, registry, ws)?;

    Ok(dependency)
}

/// Provide the existing dependency for the target table
///
/// If it doesn't exist but exists in another table, let's use that as most likely users
/// want to use the same version across all tables unless they are renaming.
fn get_existing_dependency(
    manifest: &LocalManifest,
    dep_key: &str,
    section: &DepTable,
) -> CargoResult<Option<Dependency>> {
    #[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
    enum Key {
        Error,
        Dev,
        Build,
        Normal,
        Existing,
    }

    let mut possible: Vec<_> = manifest
        .get_dependency_versions(dep_key)
        .map(|(path, dep)| {
            let key = if path == *section {
                (Key::Existing, true)
            } else if dep.is_err() {
                (Key::Error, path.target().is_some())
            } else {
                let key = match path.kind() {
                    DepKind::Normal => Key::Normal,
                    DepKind::Build => Key::Build,
                    DepKind::Development => Key::Dev,
                };
                (key, path.target().is_some())
            };
            (key, dep)
        })
        .collect();
    possible.sort_by_key(|(key, _)| *key);
    let (key, dep) = if let Some(item) = possible.pop() {
        item
    } else {
        return Ok(None);
    };
    let mut dep = dep?;

    if key.0 != Key::Existing {
        // When the dep comes from a different section, we only care about the source and not any
        // of the other fields, like `features`
        let unrelated = dep;
        dep = Dependency::new(&unrelated.name);
        dep.source = unrelated.source.clone();
        dep.registry = unrelated.registry.clone();

        // dev-dependencies do not need the version populated when path is set though we
        // should preserve it if the user chose to populate it.
        let version_required = unrelated.source().and_then(|s| s.as_registry()).is_some();
        let version_optional_in_section = section.kind() == DepKind::Development;
        if !version_required && version_optional_in_section {
            dep = dep.clear_version();
        }
    }

    Ok(Some(dep))
}

fn get_latest_dependency(
    dependency: &Dependency,
    _flag_allow_prerelease: bool,
    config: &Config,
    registry: &mut PackageRegistry<'_>,
) -> CargoResult<Dependency> {
    let query = dependency.query(config)?;
    match query {
        MaybeWorkspace::Workspace(_) => {
            unreachable!("registry dependencies required, found a workspace dependency");
        }
        MaybeWorkspace::Other(query) => {
            let possibilities = loop {
                let fuzzy = true;
                match registry.query_vec(&query, fuzzy) {
                    std::task::Poll::Ready(res) => {
                        break res?;
                    }
                    std::task::Poll::Pending => registry.block_until_ready()?,
                }
            };
            let latest = possibilities
                .iter()
                .max_by_key(|s| {
                    // Fallback to a pre-release if no official release is available by sorting them as
                    // less.
                    let stable = s.version().pre.is_empty();
                    (stable, s.version())
                })
                .ok_or_else(|| {
                    anyhow::format_err!(
                        "the crate `{dependency}` could not be found in registry index."
                    )
                })?;
            let mut dep = Dependency::from(latest);
            if let Some(reg_name) = dependency.registry.as_deref() {
                dep = dep.set_registry(reg_name);
            }
            Ok(dep)
        }
    }
}

fn select_package(
    dependency: &Dependency,
    config: &Config,
    registry: &mut PackageRegistry<'_>,
) -> CargoResult<Dependency> {
    let query = dependency.query(config)?;
    match query {
        MaybeWorkspace::Workspace(_) => {
            unreachable!("path or git dependency expected, found workspace dependency");
        }
        MaybeWorkspace::Other(query) => {
            let possibilities = loop {
                let fuzzy = false; // Returns all for path/git
                match registry.query_vec(&query, fuzzy) {
                    std::task::Poll::Ready(res) => {
                        break res?;
                    }
                    std::task::Poll::Pending => registry.block_until_ready()?,
                }
            };
            match possibilities.len() {
                0 => {
                    let source = dependency
                        .source()
                        .expect("source should be resolved before here");
                    anyhow::bail!("the crate `{dependency}` could not be found at `{source}`")
                }
                1 => {
                    let mut dep = Dependency::from(&possibilities[0]);
                    if let Some(reg_name) = dependency.registry.as_deref() {
                        dep = dep.set_registry(reg_name);
                    }
                    Ok(dep)
                }
                _ => {
                    let source = dependency
                        .source()
                        .expect("source should be resolved before here");
                    anyhow::bail!(
                        "unexpectedly found multiple copies of crate `{dependency}` at `{source}`"
                    )
                }
            }
        }
    }
}

fn infer_package(mut packages: Vec<Package>, src: &dyn std::fmt::Display) -> CargoResult<Package> {
    let package = match packages.len() {
        0 => {
            anyhow::bail!("no packages found at `{src}`");
        }
        1 => packages.pop().expect("match ensured element is present"),
        _ => {
            let mut names: Vec<_> = packages
                .iter()
                .map(|p| p.name().as_str().to_owned())
                .collect();
            names.sort_unstable();
            anyhow::bail!("multiple packages found at `{src}`: {}", names.join(", "));
        }
    };
    Ok(package)
}

fn populate_dependency(mut dependency: Dependency, arg: &DepOp) -> Dependency {
    if let Some(registry) = &arg.registry {
        if registry.is_empty() {
            dependency.registry = None;
        } else {
            dependency.registry = Some(registry.to_owned());
        }
    }
    if let Some(value) = arg.optional {
        if value {
            dependency.optional = Some(true);
        } else {
            dependency.optional = None;
        }
    }
    if let Some(value) = arg.default_features {
        if value {
            dependency.default_features = None;
        } else {
            dependency.default_features = Some(false);
        }
    }
    if let Some(value) = arg.features.as_ref() {
        dependency = dependency.extend_features(value.iter().cloned());
    }

    if let Some(rename) = &arg.rename {
        dependency = dependency.set_rename(rename);
    }

    dependency
}

/// Lookup available features
fn populate_available_features(
    mut dependency: Dependency,
    config: &Config,
    registry: &mut PackageRegistry<'_>,
    ws: &Workspace<'_>,
) -> CargoResult<Dependency> {
    if !dependency.available_features.is_empty() {
        return Ok(dependency);
    }

    let query = dependency.query(config)?;
    let query = match query {
        MaybeWorkspace::Workspace(_workspace) => {
            let dep = find_workspace_dep(dependency.toml_key(), ws.root_manifest())?;
            if let Some(features) = dep.features.clone() {
                dependency = dependency.set_inherited_features(features);
            }
            let query = dep.query(config)?;
            match query {
                MaybeWorkspace::Workspace(_) => {
                    unreachable!("This should have been caught when parsing a workspace root")
                }
                MaybeWorkspace::Other(query) => query,
            }
        }
        MaybeWorkspace::Other(query) => query,
    };
    let possibilities = loop {
        match registry.query_vec(&query, true) {
            std::task::Poll::Ready(res) => {
                break res?;
            }
            std::task::Poll::Pending => registry.block_until_ready()?,
        }
    };
    // Ensure widest feature flag compatibility by picking the earliest version that could show up
    // in the lock file for a given version requirement.
    let lowest_common_denominator = possibilities
        .iter()
        .min_by_key(|s| {
            // Fallback to a pre-release if no official release is available by sorting them as
            // more.
            let is_pre = !s.version().pre.is_empty();
            (is_pre, s.version())
        })
        .ok_or_else(|| {
            anyhow::format_err!("the crate `{dependency}` could not be found in registry index.")
        })?;
    dependency = dependency.set_available_features_from_cargo(lowest_common_denominator.features());

    Ok(dependency)
}

fn print_msg(shell: &mut Shell, dep: &Dependency, section: &[String]) -> CargoResult<()> {
    use std::fmt::Write;

    if matches!(shell.verbosity(), crate::core::shell::Verbosity::Quiet) {
        return Ok(());
    }

    let mut message = String::new();
    write!(message, "{}", dep.name)?;
    match dep.source() {
        Some(Source::Registry(src)) => {
            if src.version.chars().next().unwrap_or('0').is_ascii_digit() {
                write!(message, " v{}", src.version)?;
            } else {
                write!(message, " {}", src.version)?;
            }
        }
        Some(Source::Path(_)) => {
            write!(message, " (local)")?;
        }
        Some(Source::Git(_)) => {
            write!(message, " (git)")?;
        }
        Some(Source::Workspace(_)) => {
            write!(message, " (workspace)")?;
        }
        None => {}
    }
    write!(message, " to")?;
    if dep.optional().unwrap_or(false) {
        write!(message, " optional")?;
    }
    let section = if section.len() == 1 {
        section[0].clone()
    } else {
        format!("{} for target `{}`", &section[2], &section[1])
    };
    write!(message, " {section}")?;
    write!(message, ".")?;
    shell.status("Adding", message)?;

    let mut activated: IndexSet<_> = dep.features.iter().flatten().map(|s| s.as_str()).collect();
    if dep.default_features().unwrap_or(true) {
        activated.insert("default");
    }
    activated.extend(dep.inherited_features.iter().flatten().map(|s| s.as_str()));
    let mut walk: VecDeque<_> = activated.iter().cloned().collect();
    while let Some(next) = walk.pop_front() {
        walk.extend(
            dep.available_features
                .get(next)
                .into_iter()
                .flatten()
                .map(|s| s.as_str()),
        );
        activated.extend(
            dep.available_features
                .get(next)
                .into_iter()
                .flatten()
                .map(|s| s.as_str()),
        );
    }
    activated.remove("default");
    activated.sort();
    let mut deactivated = dep
        .available_features
        .keys()
        .filter(|f| !activated.contains(f.as_str()) && *f != "default")
        .collect::<Vec<_>>();
    deactivated.sort();
    if !activated.is_empty() || !deactivated.is_empty() {
        let prefix = format!("{:>13}", " ");
        shell.write_stderr(format_args!("{}Features:\n", prefix), &ColorSpec::new())?;
        for feat in activated {
            shell.write_stderr(&prefix, &ColorSpec::new())?;
            shell.write_stderr('+', &ColorSpec::new().set_bold(true).set_fg(Some(Green)))?;
            shell.write_stderr(format_args!(" {}\n", feat), &ColorSpec::new())?;
        }
        for feat in deactivated {
            shell.write_stderr(&prefix, &ColorSpec::new())?;
            shell.write_stderr('-', &ColorSpec::new().set_bold(true).set_fg(Some(Red)))?;
            shell.write_stderr(format_args!(" {}\n", feat), &ColorSpec::new())?;
        }
    }

    Ok(())
}

// Based on Iterator::is_sorted from nightly std; remove in favor of that when stabilized.
fn is_sorted(mut it: impl Iterator<Item = impl PartialOrd>) -> bool {
    let mut last = match it.next() {
        Some(e) => e,
        None => return true,
    };

    for curr in it {
        if curr < last {
            return false;
        }
        last = curr;
    }

    true
}

fn find_workspace_dep(toml_key: &str, root_manifest: &Path) -> CargoResult<Dependency> {
    let manifest = LocalManifest::try_new(root_manifest)?;
    let manifest = manifest
        .data
        .as_item()
        .as_table_like()
        .context("could not make `manifest.data` into a table")?;
    let workspace = manifest
        .get("workspace")
        .context("could not find `workspace`")?
        .as_table_like()
        .context("could not make `manifest.data.workspace` into a table")?;
    let dependencies = workspace
        .get("dependencies")
        .context("could not find `dependencies` table in `workspace`")?
        .as_table_like()
        .context("could not make `dependencies` into a table")?;
    let dep_item = dependencies.get(toml_key).context(format!(
        "could not find {} in `workspace.dependencies`",
        toml_key
    ))?;
    Dependency::from_toml(root_manifest.parent().unwrap(), toml_key, dep_item)
}
