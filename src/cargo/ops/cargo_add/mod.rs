//! Core of cargo-add command

mod crate_spec;

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::VecDeque;
use std::fmt::Write;
use std::path::Path;
use std::str::FromStr;

use anyhow::Context as _;
use cargo_util::paths;
use cargo_util_schemas::core::PartialVersion;
use cargo_util_schemas::manifest::PathBaseName;
use cargo_util_schemas::manifest::RustVersion;
use indexmap::IndexSet;
use itertools::Itertools;
use toml_edit::Item as TomlItem;

use crate::CargoResult;
use crate::GlobalContext;
use crate::core::Feature;
use crate::core::FeatureValue;
use crate::core::Features;
use crate::core::Package;
use crate::core::PackageId;
use crate::core::Registry;
use crate::core::Shell;
use crate::core::Summary;
use crate::core::Workspace;
use crate::core::dependency::DepKind;
use crate::core::registry::PackageRegistry;
use crate::ops::resolve_ws;
use crate::sources::source::QueryKind;
use crate::util::OptVersionReq;
use crate::util::cache_lock::CacheLockMode;
use crate::util::edit_distance;
use crate::util::style;
use crate::util::toml::lookup_path_base;
use crate::util::toml_mut::dependency::Dependency;
use crate::util::toml_mut::dependency::GitSource;
use crate::util::toml_mut::dependency::MaybeWorkspace;
use crate::util::toml_mut::dependency::PathSource;
use crate::util::toml_mut::dependency::RegistrySource;
use crate::util::toml_mut::dependency::Source;
use crate::util::toml_mut::dependency::WorkspaceSource;
use crate::util::toml_mut::manifest::DepTable;
use crate::util::toml_mut::manifest::LocalManifest;
use crate_spec::CrateSpec;

const MAX_FEATURE_PRINTS: usize = 30;

/// Information on what dependencies should be added
#[derive(Clone, Debug)]
pub struct AddOptions<'a> {
    /// Configuration information for cargo operations
    pub gctx: &'a GlobalContext,
    /// Package to add dependencies to
    pub spec: &'a Package,
    /// Dependencies to add or modify
    pub dependencies: Vec<DepOp>,
    /// Which dependency section to add these to
    pub section: DepTable,
    /// Act as if dependencies will be added
    pub dry_run: bool,
    /// Whether the minimum supported Rust version should be considered during resolution
    pub honor_rust_version: Option<bool>,
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

    let mut registry = workspace.package_registry()?;

    let deps = {
        let _lock = options
            .gctx
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
                    options.gctx,
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
                table
                    .get_values()
                    .iter_mut()
                    .map(|(key, _)| {
                        // get_values key paths always have at least one key.
                        key.remove(0)
                    })
                    .is_sorted()
            })
        });
    for dep in deps {
        print_action_msg(&mut options.gctx.shell(), &dep, &dep_table)?;
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
                "unrecognized feature{} for crate {}: {}",
                if unknown_features.len() == 1 { "" } else { "s" },
                dep.name,
                unknown_features.iter().format(", "),
            );
            if activated.is_empty() && deactivated.is_empty() {
                write!(message, "\n\nno features available for crate {}", dep.name)?;
            } else {
                let mut suggested = false;
                for unknown_feature in &unknown_features {
                    let suggestion = edit_distance::closest_msg(
                        unknown_feature,
                        deactivated.iter().chain(activated.iter()),
                        |dep| *dep,
                        "feature",
                    );
                    if !suggestion.is_empty() {
                        write!(message, "{suggestion}")?;
                        suggested = true;
                    }
                }
                if !deactivated.is_empty() && !suggested {
                    if deactivated.len() <= MAX_FEATURE_PRINTS {
                        write!(
                            message,
                            "\n\ndisabled features:\n    {}",
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
                        )?;
                    } else {
                        write!(
                            message,
                            "\n\n{} disabled features available",
                            deactivated.len()
                        )?;
                    }
                }
                if !activated.is_empty() && !suggested {
                    if deactivated.len() + activated.len() <= MAX_FEATURE_PRINTS {
                        writeln!(
                            message,
                            "\n\nenabled features:\n    {}",
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
                        )?;
                    } else {
                        writeln!(
                            message,
                            "\n\n{} enabled features available",
                            activated.len()
                        )?;
                    }
                }
            }
            anyhow::bail!(message.trim().to_owned());
        }

        print_dep_table_msg(&mut options.gctx.shell(), &dep)?;

        manifest.insert_into_table(
            &dep_table,
            &dep,
            workspace.gctx(),
            workspace.root(),
            options.spec.manifest().unstable_features(),
        )?;
        if dep.optional == Some(true) {
            let is_namespaced_features_supported =
                check_rust_version_for_optional_dependency(options.spec.rust_version())?;
            if is_namespaced_features_supported {
                let dep_key = dep.toml_key();
                if !manifest.is_explicit_dep_activation(dep_key) {
                    let table = manifest.get_table_mut(&[String::from("features")])?;
                    let dep_name = dep.rename.as_deref().unwrap_or(&dep.name);
                    let new_feature: toml_edit::Value =
                        [format!("dep:{dep_name}")].iter().collect();
                    table[dep_key] = toml_edit::value(new_feature);
                    options
                        .gctx
                        .shell()
                        .status("Adding", format!("feature `{dep_key}`"))?;
                }
            }
        }
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

    if let Some(locked_flag) = options.gctx.locked_flag() {
        let new_raw_manifest = manifest.to_string();
        if original_raw_manifest != new_raw_manifest {
            anyhow::bail!(
                "the manifest file {} needs to be updated but {locked_flag} was passed to prevent this",
                manifest.path.display()
            );
        }
    }

    if options.dry_run {
        options.gctx.shell().warn("aborting add due to dry run")?;
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
    /// Dependency key, overriding the package name in `crate_spec`
    pub rename: Option<String>,

    /// Feature flags to activate
    pub features: Option<IndexSet<String>>,
    /// Whether the default feature should be activated
    pub default_features: Option<bool>,

    /// Whether dependency is optional
    pub optional: Option<bool>,

    /// Whether dependency is public
    pub public: Option<bool>,

    /// Registry for looking up dependency version
    pub registry: Option<String>,

    /// File system path for dependency
    pub path: Option<String>,
    /// Specify a named base for a path dependency
    pub base: Option<String>,

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
    honor_rust_version: Option<bool>,
    gctx: &GlobalContext,
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
            let selected = select_package(&dependency, gctx, registry)?;
            if dependency.name != selected.name {
                gctx.shell().warn(format!(
                    "translating `{}` to `{}`",
                    dependency.name, selected.name,
                ))?;
            }
            selected
        } else {
            let mut source = crate::sources::GitSource::new(src.source_id()?, gctx)?;
            let packages = source.read_packages()?;
            let package = infer_package_for_git_source(packages, &src)?;
            Dependency::from(package.summary())
        };
        selected
    } else if let Some(raw_path) = &arg.path {
        let path = paths::normalize_path(&std::env::current_dir()?.join(raw_path));
        let mut src = PathSource::new(path);
        src.base = arg.base.clone();

        if let Some(base) = &arg.base {
            // Validate that the base is valid.
            let workspace_root = || Ok(ws.root_manifest().parent().unwrap());
            lookup_path_base(
                &PathBaseName::new(base.clone())?,
                &gctx,
                &workspace_root,
                spec.manifest().unstable_features(),
            )?;
        }

        let selected = if let Some(crate_spec) = &crate_spec {
            if let Some(v) = crate_spec.version_req() {
                // crate specifier includes a version (e.g. `docopt@0.8`)
                anyhow::bail!("cannot specify a path (`{raw_path}`) with a version (`{v}`).");
            }
            let dependency = crate_spec.to_dependency()?.set_source(src);
            let selected = select_package(&dependency, gctx, registry)?;
            if dependency.name != selected.name {
                gctx.shell().warn(format!(
                    "translating `{}` to `{}`",
                    dependency.name, selected.name,
                ))?;
            }
            selected
        } else {
            let mut source = crate::sources::PathSource::new(&src.path, src.source_id()?, gctx);
            let package = source.root_package()?;
            let mut selected = Dependency::from(package.summary());
            if let Some(Source::Path(selected_src)) = &mut selected.source {
                selected_src.base = src.base;
            }
            selected
        };
        selected
    } else if let Some(crate_spec) = &crate_spec {
        crate_spec.to_dependency()?
    } else {
        anyhow::bail!("dependency name is required");
    };
    selected_dep = populate_dependency(selected_dep, arg);

    let lookup = |dep_key: &_| {
        get_existing_dependency(
            ws,
            spec.manifest().unstable_features(),
            manifest,
            dep_key,
            section,
        )
    };
    let old_dep = fuzzy_lookup(&mut selected_dep, lookup, gctx)?;
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
        let lookup = |toml_key: &_| {
            Ok(find_workspace_dep(toml_key, ws, ws.root_manifest(), ws.unstable_features()).ok())
        };
        if let Some(_dep) = fuzzy_lookup(&mut dependency, lookup, gctx)? {
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
        } else if let Some((registry, public_source)) =
            get_public_dependency(spec, manifest, ws, section, gctx, &dependency)?
        {
            if let Some(registry) = registry {
                dependency = dependency.set_registry(registry);
            }
            dependency = dependency.set_source(public_source);
        } else {
            let latest =
                get_latest_dependency(spec, &dependency, honor_rust_version, gctx, registry)?;

            if dependency.name != latest.name {
                gctx.shell().warn(format!(
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

    let query = query_dependency(ws, gctx, &mut dependency)?;
    let dependency = populate_available_features(dependency, &query, registry)?;

    Ok(dependency)
}

fn get_public_dependency(
    spec: &Package,
    manifest: &LocalManifest,
    ws: &Workspace<'_>,
    section: &DepTable,
    gctx: &GlobalContext,
    dependency: &Dependency,
) -> CargoResult<Option<(Option<String>, Source)>> {
    if spec
        .manifest()
        .unstable_features()
        .require(Feature::public_dependency())
        .is_err()
    {
        return Ok(None);
    }

    let (package_set, resolve) = resolve_ws(ws, true)?;

    let mut latest: Option<(PackageId, OptVersionReq)> = None;

    for (_, path, dep) in manifest.get_dependencies(ws, ws.unstable_features()) {
        if path != *section {
            continue;
        }

        let Some(mut dep) = dep.ok() else {
            continue;
        };

        let dep = query_dependency(ws, gctx, &mut dep)?;
        let Some(dep_pkgid) = package_set
            .package_ids()
            .filter(|package_id| {
                package_id.name() == dep.package_name()
                    && dep.version_req().matches(package_id.version())
            })
            .max_by_key(|x| x.version())
        else {
            continue;
        };

        let mut pkg_ids_and_reqs = Vec::new();
        let mut pkg_id_queue = VecDeque::new();
        let mut examined = BTreeSet::new();
        pkg_id_queue.push_back(dep_pkgid);

        while let Some(dep_pkgid) = pkg_id_queue.pop_front() {
            let got_deps = resolve.deps(dep_pkgid).filter_map(|(id, deps)| {
                deps.iter()
                    .find(|dep| dep.is_public() && dep.kind() == DepKind::Normal)
                    .map(|dep| (id, dep))
            });

            for (pkg_id, got_dep) in got_deps {
                if got_dep.package_name() == dependency.name.as_str() {
                    pkg_ids_and_reqs.push((pkg_id, got_dep.version_req().clone()));
                }

                if examined.insert(pkg_id.clone()) {
                    pkg_id_queue.push_back(pkg_id)
                }
            }
        }

        for (pkg_id, req) in pkg_ids_and_reqs {
            if let Some((old_pkg_id, _)) = &latest
                && old_pkg_id.version() >= pkg_id.version()
            {
                continue;
            }
            latest = Some((pkg_id, req))
        }
    }

    let Some((pkg_id, version_req)) = latest else {
        return Ok(None);
    };

    let source = pkg_id.source_id();
    if source.is_git() {
        Ok(Some((
            Option::<String>::None,
            Source::Git(GitSource::new(source.as_encoded_url().to_string())),
        )))
    } else if let Some(path) = source.local_path() {
        Ok(Some((None, Source::Path(PathSource::new(path)))))
    } else {
        let toml_source = match version_req {
            crate::util::OptVersionReq::Any => {
                Source::Registry(RegistrySource::new(pkg_id.version().to_string()))
            }
            crate::util::OptVersionReq::Req(version_req)
            | crate::util::OptVersionReq::Locked(_, version_req)
            | crate::util::OptVersionReq::Precise(_, version_req) => {
                Source::Registry(RegistrySource::new(version_req.to_string()))
            }
        };
        Ok(Some((
            source
                .alt_registry_key()
                .map(|x| x.to_owned())
                .filter(|_| !source.is_crates_io()),
            toml_source,
        )))
    }
}

fn query_dependency(
    ws: &Workspace<'_>,
    gctx: &GlobalContext,
    dependency: &mut Dependency,
) -> CargoResult<crate::core::Dependency> {
    let query = dependency.query(gctx)?;
    let query = match query {
        MaybeWorkspace::Workspace(_workspace) => {
            let dep = find_workspace_dep(
                dependency.toml_key(),
                ws,
                ws.root_manifest(),
                ws.unstable_features(),
            )?;
            if let Some(features) = dep.features.clone() {
                *dependency = dependency.clone().set_inherited_features(features);
            }
            let query = dep.query(gctx)?;
            match query {
                MaybeWorkspace::Workspace(_) => {
                    anyhow::bail!(
                        "dependency ({}) specified without \
                        providing a local path, Git repository, or version",
                        dependency.toml_key()
                    );
                }
                MaybeWorkspace::Other(query) => query,
            }
        }
        MaybeWorkspace::Other(query) => query,
    };
    Ok(query)
}

fn fuzzy_lookup(
    dependency: &mut Dependency,
    lookup: impl Fn(&str) -> CargoResult<Option<Dependency>>,
    gctx: &GlobalContext,
) -> CargoResult<Option<Dependency>> {
    if let Some(rename) = dependency.rename() {
        // Manually implement `toml_key` to restrict fuzzy lookups to only package names to mirror `PackageRegistry::query()`
        return lookup(rename);
    }

    for name_permutation in [
        dependency.name.clone(),
        dependency.name.replace('-', "_"),
        dependency.name.replace('_', "-"),
    ] {
        let Some(dep) = lookup(&name_permutation)? else {
            continue;
        };

        if dependency.name != name_permutation {
            // Mirror the fuzzy matching policy of `PackageRegistry::query()`
            if !matches!(dep.source, Some(Source::Registry(_))) {
                continue;
            }
            gctx.shell().warn(format!(
                "translating `{}` to `{}`",
                dependency.name, &name_permutation,
            ))?;
            dependency.name = name_permutation;
        }
        return Ok(Some(dep));
    }

    Ok(None)
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

/// When the `--optional` option is added using `cargo add`, we need to
/// check the current rust-version. As the `dep:` syntax is only available
/// starting with Rust 1.60.0
///
/// `true` means that the rust-version is None or the rust-version is higher
/// than the version needed.
///
/// Note: Previous versions can only use the implicit feature name.
fn check_rust_version_for_optional_dependency(
    rust_version: Option<&RustVersion>,
) -> CargoResult<bool> {
    match rust_version {
        Some(version) => {
            let syntax_support_version = RustVersion::from_str("1.60.0")?;
            Ok(&syntax_support_version <= version)
        }
        None => Ok(true),
    }
}

/// Provide the existing dependency for the target table
///
/// If it doesn't exist but exists in another table, let's use that as most likely users
/// want to use the same version across all tables unless they are renaming.
fn get_existing_dependency(
    ws: &Workspace<'_>,
    unstable_features: &Features,
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
        .get_dependencies(ws, unstable_features)
        .filter_map(|(key, path, dep)| {
            if key.as_str() != dep_key {
                return None;
            }
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
            Some((key, dep))
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
    honor_rust_version: Option<bool>,
    gctx: &GlobalContext,
    registry: &mut PackageRegistry<'_>,
) -> CargoResult<Dependency> {
    let query = dependency.query(gctx)?;
    match query {
        MaybeWorkspace::Workspace(_) => {
            unreachable!("registry dependencies required, found a workspace dependency");
        }
        MaybeWorkspace::Other(query) => {
            let possibilities = loop {
                match registry.query_vec(&query, QueryKind::Normalized) {
                    std::task::Poll::Ready(res) => {
                        break res?;
                    }
                    std::task::Poll::Pending => registry.block_until_ready()?,
                }
            };

            let mut possibilities: Vec<_> = possibilities
                .into_iter()
                .map(|s| s.into_summary())
                .collect();

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

            if honor_rust_version.unwrap_or(true) {
                let (req_msrv, is_msrv) = spec
                    .rust_version()
                    .cloned()
                    .map(|msrv| CargoResult::Ok((msrv.clone().into_partial(), true)))
                    .unwrap_or_else(|| {
                        let rustc = gctx.load_global_rustc(None)?;

                        // Remove any pre-release identifiers for easier comparison
                        let rustc_version = rustc.version.clone().into();
                        Ok((rustc_version, false))
                    })?;

                let msrvs = possibilities
                    .iter()
                    .map(|s| (s, s.rust_version()))
                    .collect::<Vec<_>>();

                // Find the latest version of the dep which has a compatible rust-version. To
                // determine whether or not one rust-version is compatible with another, we
                // compare the lowest possible versions they could represent, and treat
                // candidates without a rust-version as compatible by default.
                let latest_msrv = latest_compatible(&msrvs, &req_msrv).ok_or_else(|| {
                        let name = spec.name();
                        let dep_name = &dependency.name;
                        let latest_version = latest.version();
                        let latest_msrv = latest
                            .rust_version()
                            .expect("as `None` are compatible, we can't be here");
                        if is_msrv {
                            anyhow::format_err!(
                                "\
no version of crate `{dep_name}` can maintain {name}'s rust-version of {req_msrv}
help: pass `--ignore-rust-version` to select {dep_name}@{latest_version} which requires rustc {latest_msrv}"
                            )
                        } else {
                            anyhow::format_err!(
                                "\
no version of crate `{dep_name}` is compatible with rustc {req_msrv}
help: pass `--ignore-rust-version` to select {dep_name}@{latest_version} which requires rustc {latest_msrv}"
                            )
                        }
                    })?;

                if latest_msrv.version() < latest.version() {
                    let latest_version = latest.version();
                    let latest_rust_version = latest.rust_version().unwrap();
                    let name = spec.name();
                    if is_msrv {
                        gctx.shell().warn(format_args!(
                            "\
ignoring {dependency}@{latest_version} (which requires rustc {latest_rust_version}) to maintain {name}'s rust-version of {req_msrv}",
                        ))?;
                    } else {
                        gctx.shell().warn(format_args!(
                            "\
ignoring {dependency}@{latest_version} (which requires rustc {latest_rust_version}) as it is incompatible with rustc {req_msrv}",
                        ))?;
                    }

                    latest = latest_msrv;
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

/// Of MSRV-compatible summaries, find the highest version
///
/// Assumptions:
/// - `msrvs` is sorted by version
fn latest_compatible<'s>(
    msrvs: &[(&'s Summary, Option<&RustVersion>)],
    pkg_msrv: &PartialVersion,
) -> Option<&'s Summary> {
    msrvs
        .iter()
        .filter(|(_, dep_msrv)| {
            dep_msrv
                .as_ref()
                .map(|dep_msrv| dep_msrv.is_compatible_with(pkg_msrv))
                .unwrap_or(true)
        })
        .map(|(s, _)| s)
        .next_back()
        .copied()
}

fn select_package(
    dependency: &Dependency,
    gctx: &GlobalContext,
    registry: &mut PackageRegistry<'_>,
) -> CargoResult<Dependency> {
    let query = dependency.query(gctx)?;
    match query {
        MaybeWorkspace::Workspace(_) => {
            unreachable!("path or git dependency expected, found workspace dependency");
        }
        MaybeWorkspace::Other(query) => {
            let possibilities = loop {
                // Exact to avoid returning all for path/git
                match registry.query_vec(&query, QueryKind::Normalized) {
                    std::task::Poll::Ready(res) => {
                        break res?;
                    }
                    std::task::Poll::Pending => registry.block_until_ready()?,
                }
            };

            let possibilities: Vec<_> = possibilities
                .into_iter()
                .map(|s| s.into_summary())
                .collect();

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
                    if let Some(Source::Path(PathSource { base, .. })) = dependency.source() {
                        if let Some(Source::Path(dep_src)) = &mut dep.source {
                            dep_src.base = base.clone();
                        }
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
    if let Some(value) = arg.public {
        if value {
            dependency.public = Some(true);
        } else {
            dependency.public = None;
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
    /// Editable representation of a `[dependencies]` entry
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
        activated.swap_remove("default");
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
        match registry.query_vec(&query, QueryKind::Normalized) {
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
        .map(|s| s.as_summary())
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
    if dep.public().unwrap_or(false) {
        write!(message, " public")?;
    }
    let section = if section.len() == 1 {
        section[0].clone()
    } else {
        format!("{} for target `{}`", &section[2], &section[1])
    };
    write!(message, " {section}")?;
    shell.status("Adding", message)
}

fn print_dep_table_msg(shell: &mut Shell, dep: &DependencyUI) -> CargoResult<()> {
    if matches!(shell.verbosity(), crate::core::shell::Verbosity::Quiet) {
        return Ok(());
    }

    let stderr = shell.err();
    let good = style::GOOD;
    let error = style::ERROR;

    let (activated, deactivated) = dep.features();
    if !activated.is_empty() || !deactivated.is_empty() {
        let prefix = format!("{:>13}", " ");
        let suffix = format_features_version_suffix(&dep);

        writeln!(stderr, "{prefix}Features{suffix}:")?;

        let total_activated = activated.len();
        let total_deactivated = deactivated.len();

        if total_activated <= MAX_FEATURE_PRINTS {
            for feat in activated {
                writeln!(stderr, "{prefix}{good}+{good:#} {feat}")?;
            }
        } else {
            writeln!(stderr, "{prefix}{total_activated} activated features")?;
        }

        if total_activated + total_deactivated <= MAX_FEATURE_PRINTS {
            for feat in deactivated {
                writeln!(stderr, "{prefix}{error}-{error:#} {feat}")?;
            }
        } else {
            writeln!(stderr, "{prefix}{total_deactivated} deactivated features")?;
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

fn find_workspace_dep(
    toml_key: &str,
    ws: &Workspace<'_>,
    root_manifest: &Path,
    unstable_features: &Features,
) -> CargoResult<Dependency> {
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
    let dep_item = dependencies
        .get(toml_key)
        .with_context(|| format!("could not find {toml_key} in `workspace.dependencies`"))?;
    Dependency::from_toml(
        ws.gctx(),
        ws.root(),
        root_manifest.parent().unwrap(),
        unstable_features,
        toml_key,
        dep_item,
    )
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
