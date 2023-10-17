//! Core of cargo-add command

mod crate_spec;

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::VecDeque;
use std::fmt::Write;
use std::path::Path;

use anyhow::Context as _;
use cargo_util::paths;
use indexmap::IndexSet;
use itertools::Itertools;
use toml_edit::Item as TomlItem;

use crate::core::dependency::DepKind;
use crate::core::registry::PackageRegistry;
use crate::core::FeatureValue;
use crate::core::Package;
use crate::core::Registry;
use crate::core::Shell;
use crate::core::Summary;
use crate::core::Workspace;
use crate::sources::source::QueryKind;
use crate::util::cache_lock::CacheLockMode;
use crate::util::style;
use crate::util::toml_mut::dependency::Dependency;
use crate::util::toml_mut::dependency::GitSource;
use crate::util::toml_mut::dependency::MaybeWorkspace;
use crate::util::toml_mut::dependency::PathSource;
use crate::util::toml_mut::dependency::Source;
use crate::util::toml_mut::dependency::WorkspaceSource;
use crate::util::toml_mut::manifest::DepTable;
use crate::util::toml_mut::manifest::LocalManifest;
use crate::util::RustVersion;
use crate::CargoResult;
use crate::Config;
use crate_spec::CrateSpec;

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
    /// Whether the minimum supported Rust version should be considered during resolution
    pub honor_rust_version: bool,
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
    let original_raw_manifest = manifest.to_string();
    let legacy = manifest.get_legacy_sections();
    if !legacy.is_empty() {
        anyhow::bail!(
            "Deprecated dependency sections are unsupported: {}",
            legacy.join(", ")
        );
    }

    let mut registry = PackageRegistry::new(options.config)?;

    let deps = {
        let _lock = options
            .config
            .acquire_package_cache_lock(CacheLockMode::DownloadExclusive)?;
        registry.lock_patches();
        options
            .dependencies
            .iter()
            .map(|raw| {
                resolve_dependency(
                    &manifest,
                    raw,
                    workspace,
                    &options.spec,
                    &options.section,
                    options.honor_rust_version,
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
            table_option.map_or(true, |table| {
                is_sorted(table.get_values().iter_mut().map(|(key, _)| {
                    // get_values key paths always have at least one key.
                    key.remove(0)
                }))
            })
        });
    for dep in deps {
        print_action_msg(&mut options.config.shell(), &dep, &dep_table)?;
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
            let (mut activated, mut deactivated) = dep.features();
            // Since the unknown features have been added to the DependencyUI we need to remove
            // them to present the "correct" features that can be specified for the crate.
            deactivated.retain(|f| !unknown_features.contains(f));
            activated.retain(|f| !unknown_features.contains(f));

            let mut message = format!(
                "unrecognized feature{} for crate {}: {}\n",
                if unknown_features.len() == 1 { "" } else { "s" },
                dep.name,
                unknown_features.iter().format(", "),
            );
            if activated.is_empty() && deactivated.is_empty() {
                write!(message, "no features available for crate {}", dep.name)?;
            } else {
                if !deactivated.is_empty() {
                    writeln!(
                        message,
                        "disabled features:\n    {}",
                        deactivated
                            .iter()
                            .map(|s| s.to_string())
                            .coalesce(|x, y| if x.len() + y.len() < 78 {
                                Ok(format!("{x}, {y}"))
                            } else {
                                Err((x, y))
                            })
                            .into_iter()
                            .format("\n    ")
                    )?
                }
                if !activated.is_empty() {
                    writeln!(
                        message,
                        "enabled features:\n    {}",
                        activated
                            .iter()
                            .map(|s| s.to_string())
                            .coalesce(|x, y| if x.len() + y.len() < 78 {
                                Ok(format!("{x}, {y}"))
                            } else {
                                Err((x, y))
                            })
                            .into_iter()
                            .format("\n    ")
                    )?
                }
            }
            anyhow::bail!(message.trim().to_owned());
        }

        print_dep_table_msg(&mut options.config.shell(), &dep)?;

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

    if options.config.locked() {
        let new_raw_manifest = manifest.to_string();
        if original_raw_manifest != new_raw_manifest {
            anyhow::bail!(
                "the manifest file {} needs to be updated but --locked was passed to prevent this",
                manifest.path.display()
            );
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
    spec: &Package,
    section: &DepTable,
    honor_rust_version: bool,
    config: &Config,
    registry: &mut PackageRegistry<'_>,
) -> CargoResult<DependencyUI> {
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
            let package = infer_package_for_git_source(packages, &src)?;
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
            let package = source
                .read_packages()?
                .pop()
                .expect("read_packages errors when no packages");
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
            populate_dependency(old_dep, arg)
        }
    } else {
        selected_dep
    };

    if dependency.source().is_none() {
        // Checking for a workspace dependency happens first since a member could be specified
        // in the workspace dependencies table as a dependency
        if let Some(_dep) = find_workspace_dep(dependency.toml_key(), ws.root_manifest()).ok() {
            dependency = dependency.set_source(WorkspaceSource::new());
        } else if let Some(package) = ws.members().find(|p| p.name().as_str() == dependency.name) {
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
            let latest = get_latest_dependency(
                spec,
                &dependency,
                false,
                honor_rust_version,
                config,
                registry,
            )?;

            if dependency.name != latest.name {
                config.shell().warn(format!(
                    "translating `{}` to `{}`",
                    dependency.name, latest.name,
                ))?;
                dependency.name = latest.name; // Normalize the name
            }
            dependency = dependency.set_source(latest.source.expect("latest always has a source"));
        }
    }

    if let Some(Source::Workspace(_)) = dependency.source() {
        check_invalid_ws_keys(dependency.toml_key(), arg)?;
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

    let dependency = populate_available_features(dependency, &query, registry)?;

    Ok(dependency)
}

/// When { workspace = true } you cannot define other keys that configure
/// the source of the dependency such as `version`, `registry`, `registry-index`,
/// `path`, `git`, `branch`, `tag`, `rev`, or `package`. You can also not define
/// `default-features`.
///
/// Only `default-features`, `registry` and `rename` need to be checked
///  for currently. This is because `git` and its associated keys, `path`, and
/// `version`  should all bee checked before this is called. `rename` is checked
/// for as it turns into `package`
fn check_invalid_ws_keys(toml_key: &str, arg: &DepOp) -> CargoResult<()> {
    fn err_msg(toml_key: &str, flag: &str, field: &str) -> String {
        format!(
            "cannot override workspace dependency with `{flag}`, \
            either change `workspace.dependencies.{toml_key}.{field}` \
            or define the dependency exclusively in the package's manifest"
        )
    }

    if arg.default_features.is_some() {
        anyhow::bail!(
            "{}",
            err_msg(toml_key, "--default-features", "default-features")
        )
    }
    if arg.registry.is_some() {
        anyhow::bail!("{}", err_msg(toml_key, "--registry", "registry"))
    }
    // rename is `package`
    if arg.rename.is_some() {
        anyhow::bail!("{}", err_msg(toml_key, "--rename", "package"))
    }
    Ok(())
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
    let Some((key, dep)) = possible.pop() else {
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
    spec: &Package,
    dependency: &Dependency,
    _flag_allow_prerelease: bool,
    honor_rust_version: bool,
    config: &Config,
    registry: &mut PackageRegistry<'_>,
) -> CargoResult<Dependency> {
    let query = dependency.query(config)?;
    match query {
        MaybeWorkspace::Workspace(_) => {
            unreachable!("registry dependencies required, found a workspace dependency");
        }
        MaybeWorkspace::Other(query) => {
            let mut possibilities = loop {
                match registry.query_vec(&query, QueryKind::Fuzzy) {
                    std::task::Poll::Ready(res) => {
                        break res?;
                    }
                    std::task::Poll::Pending => registry.block_until_ready()?,
                }
            };

            possibilities.sort_by_key(|s| {
                // Fallback to a pre-release if no official release is available by sorting them as
                // less.
                let stable = s.version().pre.is_empty();
                (stable, s.version().clone())
            });

            let mut latest = possibilities.last().ok_or_else(|| {
                anyhow::format_err!(
                    "the crate `{dependency}` could not be found in registry index."
                )
            })?;

            if config.cli_unstable().msrv_policy && honor_rust_version {
                fn parse_msrv(comp: &RustVersion) -> (u64, u64, u64) {
                    (comp.major, comp.minor.unwrap_or(0), comp.patch.unwrap_or(0))
                }

                if let Some(req_msrv) = spec.rust_version().map(parse_msrv) {
                    let msrvs = possibilities
                        .iter()
                        .map(|s| (s, s.rust_version().map(parse_msrv)))
                        .collect::<Vec<_>>();

                    // Find the latest version of the dep which has a compatible rust-version. To
                    // determine whether or not one rust-version is compatible with another, we
                    // compare the lowest possible versions they could represent, and treat
                    // candidates without a rust-version as compatible by default.
                    let (latest_msrv, _) = msrvs
                        .iter()
                        .filter(|(_, v)| v.map(|msrv| req_msrv >= msrv).unwrap_or(true))
                        .last()
                        .ok_or_else(|| {
                            // Failing that, try to find the highest version with the lowest
                            // rust-version to report to the user.
                            let lowest_candidate = msrvs
                                .iter()
                                .min_set_by_key(|(_, v)| v)
                                .iter()
                                .map(|(s, _)| s)
                                .max_by_key(|s| s.version());
                            rust_version_incompat_error(
                                &dependency.name,
                                spec.rust_version().unwrap(),
                                lowest_candidate.copied(),
                            )
                        })?;

                    if latest_msrv.version() < latest.version() {
                        config.shell().warn(format_args!(
                            "ignoring `{dependency}@{latest_version}` (which has a rust-version of \
                             {latest_rust_version}) to satisfy this package's rust-version of \
                             {rust_version} (use `--ignore-rust-version` to override)",
                            latest_version = latest.version(),
                            latest_rust_version = latest.rust_version().unwrap(),
                            rust_version = spec.rust_version().unwrap(),
                        ))?;

                        latest = latest_msrv;
                    }
                }
            }

            let mut dep = Dependency::from(latest);
            if let Some(reg_name) = dependency.registry.as_deref() {
                dep = dep.set_registry(reg_name);
            }
            Ok(dep)
        }
    }
}

fn rust_version_incompat_error(
    dep: &str,
    rust_version: &RustVersion,
    lowest_rust_version: Option<&Summary>,
) -> anyhow::Error {
    let mut error_msg = format!(
        "could not find version of crate `{dep}` that satisfies this package's rust-version of \
         {rust_version}\n\
         help: use `--ignore-rust-version` to override this behavior"
    );

    if let Some(lowest) = lowest_rust_version {
        // rust-version must be present for this candidate since it would have been selected as
        // compatible previously if it weren't.
        let version = lowest.version();
        let rust_version = lowest.rust_version().unwrap();
        error_msg.push_str(&format!(
            "\nnote: the lowest rust-version available for `{dep}` is {rust_version}, used in \
             version {version}"
        ));
    }

    anyhow::format_err!(error_msg)
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
                // Exact to avoid returning all for path/git
                match registry.query_vec(&query, QueryKind::Exact) {
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

fn infer_package_for_git_source(
    mut packages: Vec<Package>,
    src: &dyn std::fmt::Display,
) -> CargoResult<Package> {
    let package = match packages.len() {
        0 => unreachable!(
            "this function should only be called with packages from `GitSource::read_packages` \
            and that call should error for us when there are no packages"
        ),
        1 => packages.pop().expect("match ensured element is present"),
        _ => {
            let mut names: Vec<_> = packages
                .iter()
                .map(|p| p.name().as_str().to_owned())
                .collect();
            names.sort_unstable();
            anyhow::bail!(
                "multiple packages found at `{src}`:\n    {}\nTo disambiguate, run `cargo add --git {src} <package>`",
                names
                    .iter()
                    .map(|s| s.to_string())
                    .coalesce(|x, y| if x.len() + y.len() < 78 {
                        Ok(format!("{x}, {y}"))
                    } else {
                        Err((x, y))
                    })
                    .into_iter()
                    .format("\n    "),
            );
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

/// Track presentation-layer information with the editable representation of a `[dependencies]`
/// entry (Dependency)
pub struct DependencyUI {
    /// Editable representation of a `[depednencies]` entry
    dep: Dependency,
    /// The version of the crate that we pulled `available_features` from
    available_version: Option<semver::Version>,
    /// The widest set of features compatible with `Dependency`s version requirement
    available_features: BTreeMap<String, Vec<String>>,
}

impl DependencyUI {
    fn new(dep: Dependency) -> Self {
        Self {
            dep,
            available_version: None,
            available_features: Default::default(),
        }
    }

    fn apply_summary(&mut self, summary: &Summary) {
        self.available_version = Some(summary.version().clone());
        self.available_features = summary
            .features()
            .iter()
            .map(|(k, v)| {
                (
                    k.as_str().to_owned(),
                    v.iter()
                        .filter_map(|v| match v {
                            FeatureValue::Feature(f) => Some(f.as_str().to_owned()),
                            FeatureValue::Dep { .. } | FeatureValue::DepFeature { .. } => None,
                        })
                        .collect::<Vec<_>>(),
                )
            })
            .collect();
    }

    fn features(&self) -> (IndexSet<&str>, IndexSet<&str>) {
        let mut activated: IndexSet<_> =
            self.features.iter().flatten().map(|s| s.as_str()).collect();
        if self.default_features().unwrap_or(true) {
            activated.insert("default");
        }
        activated.extend(self.inherited_features.iter().flatten().map(|s| s.as_str()));
        let mut walk: VecDeque<_> = activated.iter().cloned().collect();
        while let Some(next) = walk.pop_front() {
            walk.extend(
                self.available_features
                    .get(next)
                    .into_iter()
                    .flatten()
                    .map(|s| s.as_str())
                    .filter(|s| !activated.contains(s)),
            );
            activated.extend(
                self.available_features
                    .get(next)
                    .into_iter()
                    .flatten()
                    .map(|s| s.as_str()),
            );
        }
        activated.remove("default");
        activated.sort();
        let mut deactivated = self
            .available_features
            .keys()
            .filter(|f| !activated.contains(f.as_str()) && *f != "default")
            .map(|f| f.as_str())
            .collect::<IndexSet<_>>();
        deactivated.sort();
        (activated, deactivated)
    }
}

impl<'s> From<&'s Summary> for DependencyUI {
    fn from(other: &'s Summary) -> Self {
        let dep = Dependency::from(other);
        let mut dep = Self::new(dep);
        dep.apply_summary(other);
        dep
    }
}

impl std::fmt::Display for DependencyUI {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.dep.fmt(f)
    }
}

impl std::ops::Deref for DependencyUI {
    type Target = Dependency;

    fn deref(&self) -> &Self::Target {
        &self.dep
    }
}

/// Lookup available features
fn populate_available_features(
    dependency: Dependency,
    query: &crate::core::dependency::Dependency,
    registry: &mut PackageRegistry<'_>,
) -> CargoResult<DependencyUI> {
    let mut dependency = DependencyUI::new(dependency);

    if !dependency.available_features.is_empty() {
        return Ok(dependency);
    }

    let possibilities = loop {
        match registry.query_vec(&query, QueryKind::Fuzzy) {
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
    dependency.apply_summary(&lowest_common_denominator);

    Ok(dependency)
}

fn print_action_msg(shell: &mut Shell, dep: &DependencyUI, section: &[String]) -> CargoResult<()> {
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
    shell.status("Adding", message)
}

fn print_dep_table_msg(shell: &mut Shell, dep: &DependencyUI) -> CargoResult<()> {
    if matches!(shell.verbosity(), crate::core::shell::Verbosity::Quiet) {
        return Ok(());
    }

    let (activated, deactivated) = dep.features();
    if !activated.is_empty() || !deactivated.is_empty() {
        let prefix = format!("{:>13}", " ");
        let suffix = format_features_version_suffix(&dep);

        shell.write_stderr(format_args!("{prefix}Features{suffix}:\n"), &style::NOP)?;

        const MAX_FEATURE_PRINTS: usize = 30;
        let total_activated = activated.len();
        let total_deactivated = deactivated.len();

        if total_activated <= MAX_FEATURE_PRINTS {
            for feat in activated {
                shell.write_stderr(&prefix, &style::NOP)?;
                shell.write_stderr('+', &style::GOOD)?;
                shell.write_stderr(format_args!(" {feat}\n"), &style::NOP)?;
            }
        } else {
            shell.write_stderr(
                format_args!("{prefix}{total_activated} activated features\n"),
                &style::NOP,
            )?;
        }

        if total_activated + total_deactivated <= MAX_FEATURE_PRINTS {
            for feat in deactivated {
                shell.write_stderr(&prefix, &style::NOP)?;
                shell.write_stderr('-', &style::ERROR)?;
                shell.write_stderr(format_args!(" {feat}\n"), &style::NOP)?;
            }
        } else {
            shell.write_stderr(
                format_args!("{prefix}{total_deactivated} deactivated features\n"),
                &style::NOP,
            )?;
        }
    }

    Ok(())
}

fn format_features_version_suffix(dep: &DependencyUI) -> String {
    if let Some(version) = &dep.available_version {
        let mut version = version.clone();
        version.build = Default::default();
        let version = version.to_string();
        // Avoid displaying the version if it will visually look like the version req that we
        // showed earlier
        let version_req = dep
            .version()
            .and_then(|v| semver::VersionReq::parse(v).ok())
            .and_then(|v| precise_version(&v));
        if version_req.as_deref() != Some(version.as_str()) {
            format!(" as of v{version}")
        } else {
            "".to_owned()
        }
    } else {
        "".to_owned()
    }
}

// Based on Iterator::is_sorted from nightly std; remove in favor of that when stabilized.
fn is_sorted(mut it: impl Iterator<Item = impl PartialOrd>) -> bool {
    let Some(mut last) = it.next() else {
        return true;
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

/// Convert a `semver::VersionReq` into a rendered `semver::Version` if all fields are fully
/// specified.
fn precise_version(version_req: &semver::VersionReq) -> Option<String> {
    version_req
        .comparators
        .iter()
        .filter(|c| {
            matches!(
                c.op,
                // Only ops we can determine a precise version from
                semver::Op::Exact
                    | semver::Op::GreaterEq
                    | semver::Op::LessEq
                    | semver::Op::Tilde
                    | semver::Op::Caret
                    | semver::Op::Wildcard
            )
        })
        .filter_map(|c| {
            // Only do it when full precision is specified
            c.minor.and_then(|minor| {
                c.patch.map(|patch| semver::Version {
                    major: c.major,
                    minor,
                    patch,
                    pre: c.pre.clone(),
                    build: Default::default(),
                })
            })
        })
        .max()
        .map(|v| v.to_string())
}
