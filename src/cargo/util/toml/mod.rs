use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::str::{self, FromStr};

use anyhow::{anyhow, bail, Context as _};
use cargo_platform::Platform;
use cargo_util::paths;
use itertools::Itertools;
use lazycell::LazyCell;
use tracing::{debug, trace};
use url::Url;

use crate::core::compiler::{CompileKind, CompileTarget};
use crate::core::dependency::{Artifact, ArtifactTarget, DepKind};
use crate::core::manifest::{ManifestMetadata, TargetSourcePath, Warnings};
use crate::core::resolver::ResolveBehavior;
use crate::core::{find_workspace_root, resolve_relative_path, CliUnstable};
use crate::core::{Dependency, Manifest, PackageId, Summary, Target};
use crate::core::{Edition, EitherManifest, Feature, Features, VirtualManifest, Workspace};
use crate::core::{GitReference, PackageIdSpec, SourceId, WorkspaceConfig, WorkspaceRootConfig};
use crate::sources::{CRATES_IO_INDEX, CRATES_IO_REGISTRY};
use crate::util::errors::{CargoResult, ManifestError};
use crate::util::interning::InternedString;
use crate::util::restricted_names;
use crate::util::{
    self, config::ConfigRelativePath, validate_package_name, Config, IntoUrl, OptVersionReq,
    RustVersion,
};

mod embedded;
pub mod schema;
mod targets;
use self::targets::targets;

/// Loads a `Cargo.toml` from a file on disk.
///
/// This could result in a real or virtual manifest being returned.
///
/// A list of nested paths is also returned, one for each path dependency
/// within the manifest. For virtual manifests, these paths can only
/// come from patched or replaced dependencies. These paths are not
/// canonicalized.
pub fn read_manifest(
    path: &Path,
    source_id: SourceId,
    config: &Config,
) -> Result<(EitherManifest, Vec<PathBuf>), ManifestError> {
    trace!(
        "read_manifest; path={}; source-id={}",
        path.display(),
        source_id
    );
    let mut contents = paths::read(path).map_err(|err| ManifestError::new(err, path.into()))?;
    let embedded = is_embedded(path);
    if embedded {
        if !config.cli_unstable().script {
            return Err(ManifestError::new(
                anyhow::anyhow!("parsing `{}` requires `-Zscript`", path.display()),
                path.into(),
            ));
        }
        contents = embedded::expand_manifest(&contents, path, config)
            .map_err(|err| ManifestError::new(err, path.into()))?;
    }

    read_manifest_from_str(&contents, path, embedded, source_id, config)
        .with_context(|| format!("failed to parse manifest at `{}`", path.display()))
        .map_err(|err| ManifestError::new(err, path.into()))
}

/// See also `bin/cargo/commands/run.rs`s `is_manifest_command`
pub fn is_embedded(path: &Path) -> bool {
    let ext = path.extension();
    ext == Some(OsStr::new("rs")) ||
        // Provide better errors by not considering directories to be embedded manifests
        (ext.is_none() && path.is_file())
}

/// Parse an already-loaded `Cargo.toml` as a Cargo manifest.
///
/// This could result in a real or virtual manifest being returned.
///
/// A list of nested paths is also returned, one for each path dependency
/// within the manifest. For virtual manifests, these paths can only
/// come from patched or replaced dependencies. These paths are not
/// canonicalized.
fn read_manifest_from_str(
    contents: &str,
    manifest_file: &Path,
    embedded: bool,
    source_id: SourceId,
    config: &Config,
) -> CargoResult<(EitherManifest, Vec<PathBuf>)> {
    let package_root = manifest_file.parent().unwrap();

    let mut unused = BTreeSet::new();
    let deserializer = toml::de::Deserializer::new(contents);
    let manifest: schema::TomlManifest = serde_ignored::deserialize(deserializer, |path| {
        let mut key = String::new();
        stringify(&mut key, &path);
        unused.insert(key);
    })?;
    let add_unused = |warnings: &mut Warnings| {
        for key in unused {
            warnings.add_warning(format!("unused manifest key: {}", key));
            if key == "profiles.debug" {
                warnings.add_warning("use `[profile.dev]` to configure debug builds".to_string());
            }
        }
    };

    if let Some(deps) = manifest
        .workspace
        .as_ref()
        .and_then(|ws| ws.dependencies.as_ref())
    {
        for (name, dep) in deps {
            if dep.is_optional() {
                bail!(
                    "{} is optional, but workspace dependencies cannot be optional",
                    name
                );
            }
        }
    }
    return if manifest.project.is_some() || manifest.package.is_some() {
        let (mut manifest, paths) = schema::TomlManifest::to_real_manifest(
            manifest,
            embedded,
            source_id,
            package_root,
            config,
        )?;
        add_unused(manifest.warnings_mut());
        if manifest.targets().iter().all(|t| t.is_custom_build()) {
            bail!(
                "no targets specified in the manifest\n\
                 either src/lib.rs, src/main.rs, a [lib] section, or \
                 [[bin]] section must be present"
            )
        }
        Ok((EitherManifest::Real(manifest), paths))
    } else {
        let (mut m, paths) =
            schema::TomlManifest::to_virtual_manifest(manifest, source_id, package_root, config)?;
        add_unused(m.warnings_mut());
        Ok((EitherManifest::Virtual(m), paths))
    };

    fn stringify(dst: &mut String, path: &serde_ignored::Path<'_>) {
        use serde_ignored::Path;

        match *path {
            Path::Root => {}
            Path::Seq { parent, index } => {
                stringify(dst, parent);
                if !dst.is_empty() {
                    dst.push('.');
                }
                dst.push_str(&index.to_string());
            }
            Path::Map { parent, ref key } => {
                stringify(dst, parent);
                if !dst.is_empty() {
                    dst.push('.');
                }
                dst.push_str(key);
            }
            Path::Some { parent }
            | Path::NewtypeVariant { parent }
            | Path::NewtypeStruct { parent } => stringify(dst, parent),
        }
    }
}

/// Warn about paths that have been deprecated and may conflict.
fn warn_on_deprecated(new_path: &str, name: &str, kind: &str, warnings: &mut Vec<String>) {
    let old_path = new_path.replace("-", "_");
    warnings.push(format!(
        "conflicting between `{new_path}` and `{old_path}` in the `{name}` {kind}.\n
        `{old_path}` is ignored and not recommended for use in the future"
    ))
}

impl schema::TomlManifest {
    /// Prepares the manifest for publishing.
    // - Path and git components of dependency specifications are removed.
    // - License path is updated to point within the package.
    pub fn prepare_for_publish(
        &self,
        ws: &Workspace<'_>,
        package_root: &Path,
    ) -> CargoResult<schema::TomlManifest> {
        let config = ws.config();
        let mut package = self
            .package
            .as_ref()
            .or_else(|| self.project.as_ref())
            .unwrap()
            .clone();
        package.workspace = None;
        let current_resolver = package
            .resolver
            .as_ref()
            .map(|r| ResolveBehavior::from_manifest(r))
            .unwrap_or_else(|| {
                package
                    .edition
                    .as_ref()
                    .and_then(|e| e.as_defined())
                    .map(|e| Edition::from_str(e))
                    .unwrap_or(Ok(Edition::Edition2015))
                    .map(|e| e.default_resolve_behavior())
            })?;
        if ws.resolve_behavior() != current_resolver {
            // This ensures the published crate if built as a root (e.g. `cargo install`) will
            // use the same resolver behavior it was tested with in the workspace.
            // To avoid forcing a higher MSRV we don't explicitly set this if it would implicitly
            // result in the same thing.
            package.resolver = Some(ws.resolve_behavior().to_manifest());
        }
        if let Some(license_file) = &package.license_file {
            let license_file = license_file
                .as_defined()
                .context("license file should have been resolved before `prepare_for_publish()`")?;
            let license_path = Path::new(&license_file);
            let abs_license_path = paths::normalize_path(&package_root.join(license_path));
            if abs_license_path.strip_prefix(package_root).is_err() {
                // This path points outside of the package root. `cargo package`
                // will copy it into the root, so adjust the path to this location.
                package.license_file = Some(schema::MaybeWorkspace::Defined(
                    license_path
                        .file_name()
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .to_string(),
                ));
            }
        }

        if let Some(readme) = &package.readme {
            let readme = readme
                .as_defined()
                .context("readme should have been resolved before `prepare_for_publish()`")?;
            match readme {
                schema::StringOrBool::String(readme) => {
                    let readme_path = Path::new(&readme);
                    let abs_readme_path = paths::normalize_path(&package_root.join(readme_path));
                    if abs_readme_path.strip_prefix(package_root).is_err() {
                        // This path points outside of the package root. `cargo package`
                        // will copy it into the root, so adjust the path to this location.
                        package.readme = Some(schema::MaybeWorkspace::Defined(
                            schema::StringOrBool::String(
                                readme_path
                                    .file_name()
                                    .unwrap()
                                    .to_str()
                                    .unwrap()
                                    .to_string(),
                            ),
                        ));
                    }
                }
                schema::StringOrBool::Bool(_) => {}
            }
        }
        let all = |_d: &schema::TomlDependency| true;
        return Ok(schema::TomlManifest {
            package: Some(package),
            project: None,
            profile: self.profile.clone(),
            lib: self.lib.clone(),
            bin: self.bin.clone(),
            example: self.example.clone(),
            test: self.test.clone(),
            bench: self.bench.clone(),
            dependencies: map_deps(config, self.dependencies.as_ref(), all)?,
            dev_dependencies: map_deps(
                config,
                self.dev_dependencies(),
                schema::TomlDependency::is_version_specified,
            )?,
            dev_dependencies2: None,
            build_dependencies: map_deps(config, self.build_dependencies(), all)?,
            build_dependencies2: None,
            features: self.features.clone(),
            target: match self.target.as_ref().map(|target_map| {
                target_map
                    .iter()
                    .map(|(k, v)| {
                        Ok((
                            k.clone(),
                            schema::TomlPlatform {
                                dependencies: map_deps(config, v.dependencies.as_ref(), all)?,
                                dev_dependencies: map_deps(
                                    config,
                                    v.dev_dependencies(),
                                    schema::TomlDependency::is_version_specified,
                                )?,
                                dev_dependencies2: None,
                                build_dependencies: map_deps(config, v.build_dependencies(), all)?,
                                build_dependencies2: None,
                            },
                        ))
                    })
                    .collect()
            }) {
                Some(Ok(v)) => Some(v),
                Some(Err(e)) => return Err(e),
                None => None,
            },
            replace: None,
            patch: None,
            workspace: None,
            badges: self.badges.clone(),
            cargo_features: self.cargo_features.clone(),
            lints: self.lints.clone(),
        });

        fn map_deps(
            config: &Config,
            deps: Option<&BTreeMap<String, schema::MaybeWorkspaceDependency>>,
            filter: impl Fn(&schema::TomlDependency) -> bool,
        ) -> CargoResult<Option<BTreeMap<String, schema::MaybeWorkspaceDependency>>> {
            let Some(deps) = deps else { return Ok(None) };
            let deps = deps
                .iter()
                .filter(|(_k, v)| {
                    if let schema::MaybeWorkspace::Defined(def) = v {
                        filter(def)
                    } else {
                        false
                    }
                })
                .map(|(k, v)| Ok((k.clone(), map_dependency(config, v)?)))
                .collect::<CargoResult<BTreeMap<_, _>>>()?;
            Ok(Some(deps))
        }

        fn map_dependency(
            config: &Config,
            dep: &schema::MaybeWorkspaceDependency,
        ) -> CargoResult<schema::MaybeWorkspaceDependency> {
            let dep = match dep {
                schema::MaybeWorkspace::Defined(schema::TomlDependency::Detailed(d)) => {
                    let mut d = d.clone();
                    // Path dependencies become crates.io deps.
                    d.path.take();
                    // Same with git dependencies.
                    d.git.take();
                    d.branch.take();
                    d.tag.take();
                    d.rev.take();
                    // registry specifications are elaborated to the index URL
                    if let Some(registry) = d.registry.take() {
                        d.registry_index = Some(config.get_registry_index(&registry)?.to_string());
                    }
                    Ok(d)
                }
                schema::MaybeWorkspace::Defined(schema::TomlDependency::Simple(s)) => {
                    Ok(schema::DetailedTomlDependency {
                        version: Some(s.clone()),
                        ..Default::default()
                    })
                }
                _ => unreachable!(),
            };
            dep.map(schema::TomlDependency::Detailed)
                .map(schema::MaybeWorkspace::Defined)
        }
    }

    pub fn to_real_manifest(
        me: schema::TomlManifest,
        embedded: bool,
        source_id: SourceId,
        package_root: &Path,
        config: &Config,
    ) -> CargoResult<(Manifest, Vec<PathBuf>)> {
        fn get_ws(
            config: &Config,
            resolved_path: &Path,
            workspace_config: &WorkspaceConfig,
        ) -> CargoResult<schema::InheritableFields> {
            match workspace_config {
                WorkspaceConfig::Root(root) => Ok(root.inheritable().clone()),
                WorkspaceConfig::Member {
                    root: Some(ref path_to_root),
                } => {
                    let path = resolved_path
                        .parent()
                        .unwrap()
                        .join(path_to_root)
                        .join("Cargo.toml");
                    let root_path = paths::normalize_path(&path);
                    inheritable_from_path(config, root_path)
                }
                WorkspaceConfig::Member { root: None } => {
                    match find_workspace_root(&resolved_path, config)? {
                        Some(path_to_root) => inheritable_from_path(config, path_to_root),
                        None => Err(anyhow!("failed to find a workspace root")),
                    }
                }
            }
        }

        if !package_root.is_dir() {
            bail!(
                "package root '{}' is not a directory",
                package_root.display()
            );
        };

        let mut nested_paths = vec![];
        let mut warnings = vec![];
        let mut errors = vec![];

        // Parse features first so they will be available when parsing other parts of the TOML.
        let empty = Vec::new();
        let cargo_features = me.cargo_features.as_ref().unwrap_or(&empty);
        let features = Features::new(cargo_features, config, &mut warnings, source_id.is_path())?;

        let mut package = match (&me.package, &me.project) {
            (Some(_), Some(project)) => {
                if source_id.is_path() {
                    config.shell().warn(format!(
                        "manifest at `{}` contains both `project` and `package`, \
                    this could become a hard error in the future",
                        package_root.display()
                    ))?;
                }
                project.clone()
            }
            (Some(package), None) => package.clone(),
            (None, Some(project)) => {
                if source_id.is_path() {
                    config.shell().warn(format!(
                        "manifest at `{}` contains `[project]` instead of `[package]`, \
                                this could become a hard error in the future",
                        package_root.display()
                    ))?;
                }
                project.clone()
            }
            (None, None) => bail!("no `package` section found"),
        };

        let workspace_config = match (me.workspace.as_ref(), package.workspace.as_ref()) {
            (Some(toml_config), None) => {
                let mut inheritable = toml_config.package.clone().unwrap_or_default();
                inheritable.update_ws_path(package_root.to_path_buf());
                inheritable.update_deps(toml_config.dependencies.clone());
                let lints = toml_config.lints.clone();
                let lints = verify_lints(lints)?;
                inheritable.update_lints(lints);
                if let Some(ws_deps) = &inheritable.dependencies {
                    for (name, dep) in ws_deps {
                        unused_dep_keys(
                            name,
                            "workspace.dependencies",
                            dep.unused_keys(),
                            &mut warnings,
                        );
                    }
                }
                let ws_root_config = WorkspaceRootConfig::new(
                    package_root,
                    &toml_config.members,
                    &toml_config.default_members,
                    &toml_config.exclude,
                    &Some(inheritable),
                    &toml_config.metadata,
                );
                config
                    .ws_roots
                    .borrow_mut()
                    .insert(package_root.to_path_buf(), ws_root_config.clone());
                WorkspaceConfig::Root(ws_root_config)
            }
            (None, root) => WorkspaceConfig::Member {
                root: root.cloned(),
            },
            (Some(..), Some(..)) => bail!(
                "cannot configure both `package.workspace` and \
                 `[workspace]`, only one can be specified"
            ),
        };

        let package_name = package.name.trim();
        if package_name.is_empty() {
            bail!("package name cannot be an empty string")
        }

        validate_package_name(package_name, "package name", "")?;

        let resolved_path = package_root.join("Cargo.toml");

        let inherit_cell: LazyCell<schema::InheritableFields> = LazyCell::new();
        let inherit =
            || inherit_cell.try_borrow_with(|| get_ws(config, &resolved_path, &workspace_config));

        let version = package
            .version
            .clone()
            .map(|version| version.resolve("version", || inherit()?.version()))
            .transpose()?;

        package.version = version.clone().map(schema::MaybeWorkspace::Defined);

        let pkgid = package.to_package_id(
            source_id,
            version
                .clone()
                .unwrap_or_else(|| semver::Version::new(0, 0, 0)),
        );

        let edition = if let Some(edition) = package.edition.clone() {
            let edition: Edition = edition
                .resolve("edition", || inherit()?.edition())?
                .parse()
                .with_context(|| "failed to parse the `edition` key")?;
            package.edition = Some(schema::MaybeWorkspace::Defined(edition.to_string()));
            edition
        } else {
            Edition::Edition2015
        };
        // Add these lines if start a new unstable edition.
        // ```
        // if edition == Edition::Edition20xx {
        //     features.require(Feature::edition20xx())?;
        // }
        // ```
        if edition == Edition::Edition2024 {
            features.require(Feature::edition2024())?;
        } else if !edition.is_stable() {
            // Guard in case someone forgets to add .require()
            return Err(util::errors::internal(format!(
                "edition {} should be gated",
                edition
            )));
        }

        let rust_version = if let Some(rust_version) = &package.rust_version {
            let rust_version = rust_version
                .clone()
                .resolve("rust_version", || inherit()?.rust_version())?;
            let req = rust_version.to_caret_req();
            if let Some(first_version) = edition.first_version() {
                let unsupported =
                    semver::Version::new(first_version.major, first_version.minor - 1, 9999);
                if req.matches(&unsupported) {
                    bail!(
                        "rust-version {} is older than first version ({}) required by \
                            the specified edition ({})",
                        rust_version,
                        first_version,
                        edition,
                    )
                }
            }
            Some(rust_version)
        } else {
            None
        };

        if package.metabuild.is_some() {
            features.require(Feature::metabuild())?;
        }

        let resolve_behavior = match (
            package.resolver.as_ref(),
            me.workspace.as_ref().and_then(|ws| ws.resolver.as_ref()),
        ) {
            (None, None) => None,
            (Some(s), None) | (None, Some(s)) => Some(ResolveBehavior::from_manifest(s)?),
            (Some(_), Some(_)) => {
                bail!("cannot specify `resolver` field in both `[workspace]` and `[package]`")
            }
        };

        // If we have no lib at all, use the inferred lib, if available.
        // If we have a lib with a path, we're done.
        // If we have a lib with no path, use the inferred lib or else the package name.
        let targets = targets(
            &features,
            &me,
            package_name,
            package_root,
            edition,
            &package.build,
            &package.metabuild,
            &mut warnings,
            &mut errors,
        )?;

        if targets.is_empty() {
            debug!("manifest has no build targets");
        }

        if let Err(conflict_targets) = unique_build_targets(&targets, package_root) {
            conflict_targets
                .iter()
                .for_each(|(target_path, conflicts)| {
                    warnings.push(format!(
                        "file `{}` found to be present in multiple \
                 build targets:\n{}",
                        target_path.display().to_string(),
                        conflicts
                            .iter()
                            .map(|t| format!(
                                "  * `{}` target `{}`",
                                t.kind().description(),
                                t.name(),
                            ))
                            .join("\n")
                    ));
                })
        }

        if let Some(links) = &package.links {
            if !targets.iter().any(|t| t.is_custom_build()) {
                bail!(
                    "package `{}` specifies that it links to `{}` but does not \
                     have a custom build script",
                    pkgid,
                    links
                )
            }
        }

        let mut deps = Vec::new();

        let mut cx = Context {
            deps: &mut deps,
            source_id,
            nested_paths: &mut nested_paths,
            config,
            warnings: &mut warnings,
            features: &features,
            platform: None,
            root: package_root,
        };

        fn process_dependencies(
            cx: &mut Context<'_, '_>,
            new_deps: Option<&BTreeMap<String, schema::MaybeWorkspaceDependency>>,
            kind: Option<DepKind>,
            workspace_config: &WorkspaceConfig,
            inherit_cell: &LazyCell<schema::InheritableFields>,
        ) -> CargoResult<Option<BTreeMap<String, schema::MaybeWorkspaceDependency>>> {
            let Some(dependencies) = new_deps else {
                return Ok(None);
            };

            let inheritable = || {
                inherit_cell.try_borrow_with(|| {
                    get_ws(cx.config, &cx.root.join("Cargo.toml"), &workspace_config)
                })
            };

            let mut deps: BTreeMap<String, schema::MaybeWorkspaceDependency> = BTreeMap::new();
            for (n, v) in dependencies.iter() {
                let resolved = v
                    .clone()
                    .resolve_with_self(n, |dep| dep.resolve(n, inheritable, cx))?;
                let dep = resolved.to_dependency(n, cx, kind)?;
                let name_in_toml = dep.name_in_toml().as_str();
                validate_package_name(name_in_toml, "dependency name", "")?;
                let kind_name = match kind {
                    Some(k) => k.kind_table(),
                    None => "dependencies",
                };
                let table_in_toml = if let Some(platform) = &cx.platform {
                    format!("target.{}.{kind_name}", platform.to_string())
                } else {
                    kind_name.to_string()
                };
                unused_dep_keys(name_in_toml, &table_in_toml, v.unused_keys(), cx.warnings);
                cx.deps.push(dep);
                deps.insert(
                    n.to_string(),
                    schema::MaybeWorkspace::Defined(resolved.clone()),
                );
            }
            Ok(Some(deps))
        }

        // Collect the dependencies.
        let dependencies = process_dependencies(
            &mut cx,
            me.dependencies.as_ref(),
            None,
            &workspace_config,
            &inherit_cell,
        )?;
        if me.dev_dependencies.is_some() && me.dev_dependencies2.is_some() {
            warn_on_deprecated("dev-dependencies", package_name, "package", cx.warnings);
        }
        let dev_deps = me.dev_dependencies();
        let dev_deps = process_dependencies(
            &mut cx,
            dev_deps,
            Some(DepKind::Development),
            &workspace_config,
            &inherit_cell,
        )?;
        if me.build_dependencies.is_some() && me.build_dependencies2.is_some() {
            warn_on_deprecated("build-dependencies", package_name, "package", cx.warnings);
        }
        let build_deps = me.build_dependencies();
        let build_deps = process_dependencies(
            &mut cx,
            build_deps,
            Some(DepKind::Build),
            &workspace_config,
            &inherit_cell,
        )?;

        let lints = me
            .lints
            .clone()
            .map(|mw| mw.resolve(|| inherit()?.lints()))
            .transpose()?;
        let lints = verify_lints(lints)?;
        let default = schema::TomlLints::default();
        let rustflags = lints_to_rustflags(lints.as_ref().unwrap_or(&default));

        let mut target: BTreeMap<String, schema::TomlPlatform> = BTreeMap::new();
        for (name, platform) in me.target.iter().flatten() {
            cx.platform = {
                let platform: Platform = name.parse()?;
                platform.check_cfg_attributes(cx.warnings);
                Some(platform)
            };
            let deps = process_dependencies(
                &mut cx,
                platform.dependencies.as_ref(),
                None,
                &workspace_config,
                &inherit_cell,
            )?;
            if platform.build_dependencies.is_some() && platform.build_dependencies2.is_some() {
                warn_on_deprecated("build-dependencies", name, "platform target", cx.warnings);
            }
            let build_deps = platform.build_dependencies();
            let build_deps = process_dependencies(
                &mut cx,
                build_deps,
                Some(DepKind::Build),
                &workspace_config,
                &inherit_cell,
            )?;
            if platform.dev_dependencies.is_some() && platform.dev_dependencies2.is_some() {
                warn_on_deprecated("dev-dependencies", name, "platform target", cx.warnings);
            }
            let dev_deps = platform.dev_dependencies();
            let dev_deps = process_dependencies(
                &mut cx,
                dev_deps,
                Some(DepKind::Development),
                &workspace_config,
                &inherit_cell,
            )?;
            target.insert(
                name.clone(),
                schema::TomlPlatform {
                    dependencies: deps,
                    build_dependencies: build_deps,
                    build_dependencies2: None,
                    dev_dependencies: dev_deps,
                    dev_dependencies2: None,
                },
            );
        }

        let target = if target.is_empty() {
            None
        } else {
            Some(target)
        };
        let replace = me.replace(&mut cx)?;
        let patch = me.patch(&mut cx)?;

        {
            let mut names_sources = BTreeMap::new();
            for dep in &deps {
                let name = dep.name_in_toml();
                let prev = names_sources.insert(name, dep.source_id());
                if prev.is_some() && prev != Some(dep.source_id()) {
                    bail!(
                        "Dependency '{}' has different source paths depending on the build \
                         target. Each dependency must have a single canonical source path \
                         irrespective of build target.",
                        name
                    );
                }
            }
        }

        let exclude = package
            .exclude
            .clone()
            .map(|mw| mw.resolve("exclude", || inherit()?.exclude()))
            .transpose()?
            .unwrap_or_default();
        let include = package
            .include
            .clone()
            .map(|mw| mw.resolve("include", || inherit()?.include()))
            .transpose()?
            .unwrap_or_default();
        let empty_features = BTreeMap::new();

        let summary = Summary::new(
            pkgid,
            deps,
            &me.features
                .as_ref()
                .unwrap_or(&empty_features)
                .iter()
                .map(|(k, v)| {
                    (
                        InternedString::new(k),
                        v.iter().map(InternedString::from).collect(),
                    )
                })
                .collect(),
            package.links.as_deref(),
            rust_version.clone(),
        )?;

        let metadata = ManifestMetadata {
            description: package
                .description
                .clone()
                .map(|mw| mw.resolve("description", || inherit()?.description()))
                .transpose()?,
            homepage: package
                .homepage
                .clone()
                .map(|mw| mw.resolve("homepage", || inherit()?.homepage()))
                .transpose()?,
            documentation: package
                .documentation
                .clone()
                .map(|mw| mw.resolve("documentation", || inherit()?.documentation()))
                .transpose()?,
            readme: readme_for_package(
                package_root,
                package
                    .readme
                    .clone()
                    .map(|mw| mw.resolve("readme", || inherit()?.readme(package_root)))
                    .transpose()?
                    .as_ref(),
            ),
            authors: package
                .authors
                .clone()
                .map(|mw| mw.resolve("authors", || inherit()?.authors()))
                .transpose()?
                .unwrap_or_default(),
            license: package
                .license
                .clone()
                .map(|mw| mw.resolve("license", || inherit()?.license()))
                .transpose()?,
            license_file: package
                .license_file
                .clone()
                .map(|mw| mw.resolve("license", || inherit()?.license_file(package_root)))
                .transpose()?,
            repository: package
                .repository
                .clone()
                .map(|mw| mw.resolve("repository", || inherit()?.repository()))
                .transpose()?,
            keywords: package
                .keywords
                .clone()
                .map(|mw| mw.resolve("keywords", || inherit()?.keywords()))
                .transpose()?
                .unwrap_or_default(),
            categories: package
                .categories
                .clone()
                .map(|mw| mw.resolve("categories", || inherit()?.categories()))
                .transpose()?
                .unwrap_or_default(),
            badges: me
                .badges
                .clone()
                .map(|mw| mw.resolve("badges", || inherit()?.badges()))
                .transpose()?
                .unwrap_or_default(),
            links: package.links.clone(),
            rust_version: package
                .rust_version
                .map(|mw| mw.resolve("rust-version", || inherit()?.rust_version()))
                .transpose()?,
        };
        package.description = metadata
            .description
            .clone()
            .map(|description| schema::MaybeWorkspace::Defined(description));
        package.homepage = metadata
            .homepage
            .clone()
            .map(|homepage| schema::MaybeWorkspace::Defined(homepage));
        package.documentation = metadata
            .documentation
            .clone()
            .map(|documentation| schema::MaybeWorkspace::Defined(documentation));
        package.readme = metadata
            .readme
            .clone()
            .map(|readme| schema::MaybeWorkspace::Defined(schema::StringOrBool::String(readme)));
        package.authors = package
            .authors
            .as_ref()
            .map(|_| schema::MaybeWorkspace::Defined(metadata.authors.clone()));
        package.license = metadata
            .license
            .clone()
            .map(|license| schema::MaybeWorkspace::Defined(license));
        package.license_file = metadata
            .license_file
            .clone()
            .map(|license_file| schema::MaybeWorkspace::Defined(license_file));
        package.repository = metadata
            .repository
            .clone()
            .map(|repository| schema::MaybeWorkspace::Defined(repository));
        package.keywords = package
            .keywords
            .as_ref()
            .map(|_| schema::MaybeWorkspace::Defined(metadata.keywords.clone()));
        package.categories = package
            .categories
            .as_ref()
            .map(|_| schema::MaybeWorkspace::Defined(metadata.categories.clone()));
        package.rust_version = rust_version
            .clone()
            .map(|rv| schema::MaybeWorkspace::Defined(rv));
        package.exclude = package
            .exclude
            .as_ref()
            .map(|_| schema::MaybeWorkspace::Defined(exclude.clone()));
        package.include = package
            .include
            .as_ref()
            .map(|_| schema::MaybeWorkspace::Defined(include.clone()));

        let profiles = me.profile.clone();
        if let Some(profiles) = &profiles {
            let cli_unstable = config.cli_unstable();
            profiles.validate(cli_unstable, &features, &mut warnings)?;
        }

        let publish = package
            .publish
            .clone()
            .map(|publish| publish.resolve("publish", || inherit()?.publish()).unwrap());

        package.publish = publish.clone().map(|p| schema::MaybeWorkspace::Defined(p));

        let publish = match publish {
            Some(schema::VecStringOrBool::VecString(ref vecstring)) => Some(vecstring.clone()),
            Some(schema::VecStringOrBool::Bool(false)) => Some(vec![]),
            Some(schema::VecStringOrBool::Bool(true)) => None,
            None => version.is_none().then_some(vec![]),
        };

        if version.is_none() && publish != Some(vec![]) {
            bail!("`package.publish` requires `package.version` be specified");
        }

        if summary.features().contains_key("default-features") {
            warnings.push(
                "`default-features = [\"..\"]` was found in [features]. \
                 Did you mean to use `default = [\"..\"]`?"
                    .to_string(),
            )
        }

        if let Some(run) = &package.default_run {
            if !targets
                .iter()
                .filter(|t| t.is_bin())
                .any(|t| t.name() == run)
            {
                let suggestion =
                    util::closest_msg(run, targets.iter().filter(|t| t.is_bin()), |t| t.name());
                bail!("default-run target `{}` not found{}", run, suggestion);
            }
        }

        let default_kind = package
            .default_target
            .as_ref()
            .map(|t| CompileTarget::new(&*t))
            .transpose()?
            .map(CompileKind::Target);
        let forced_kind = package
            .forced_target
            .as_ref()
            .map(|t| CompileTarget::new(&*t))
            .transpose()?
            .map(CompileKind::Target);
        let custom_metadata = package.metadata.clone();
        let resolved_toml = schema::TomlManifest {
            cargo_features: me.cargo_features.clone(),
            package: Some(package.clone()),
            project: None,
            profile: me.profile.clone(),
            lib: me.lib.clone(),
            bin: me.bin.clone(),
            example: me.example.clone(),
            test: me.test.clone(),
            bench: me.bench.clone(),
            dependencies,
            dev_dependencies: dev_deps,
            dev_dependencies2: None,
            build_dependencies: build_deps,
            build_dependencies2: None,
            features: me.features.clone(),
            target,
            replace: me.replace.clone(),
            patch: me.patch.clone(),
            workspace: me.workspace.clone(),
            badges: me
                .badges
                .as_ref()
                .map(|_| schema::MaybeWorkspace::Defined(metadata.badges.clone())),
            lints: lints.map(|lints| schema::MaybeWorkspaceLints {
                workspace: false,
                lints,
            }),
        };
        let mut manifest = Manifest::new(
            summary,
            default_kind,
            forced_kind,
            targets,
            exclude,
            include,
            package.links.clone(),
            metadata,
            custom_metadata,
            profiles,
            publish,
            replace,
            patch,
            workspace_config,
            features,
            edition,
            rust_version,
            package.im_a_teapot,
            package.default_run.clone(),
            Rc::new(resolved_toml),
            package.metabuild.clone().map(|sov| sov.0),
            resolve_behavior,
            rustflags,
            embedded,
        );
        if package.license_file.is_some() && package.license.is_some() {
            manifest.warnings_mut().add_warning(
                "only one of `license` or `license-file` is necessary\n\
                 `license` should be used if the package license can be expressed \
                 with a standard SPDX expression.\n\
                 `license-file` should be used if the package uses a non-standard license.\n\
                 See https://doc.rust-lang.org/cargo/reference/manifest.html#the-license-and-license-file-fields \
                 for more information."
                    .to_string(),
            );
        }
        for warning in warnings {
            manifest.warnings_mut().add_warning(warning);
        }
        for error in errors {
            manifest.warnings_mut().add_critical_warning(error);
        }

        manifest.feature_gate()?;

        Ok((manifest, nested_paths))
    }

    fn to_virtual_manifest(
        me: schema::TomlManifest,
        source_id: SourceId,
        root: &Path,
        config: &Config,
    ) -> CargoResult<(VirtualManifest, Vec<PathBuf>)> {
        if me.project.is_some() {
            bail!("this virtual manifest specifies a [project] section, which is not allowed");
        }
        if me.package.is_some() {
            bail!("this virtual manifest specifies a [package] section, which is not allowed");
        }
        if me.lib.is_some() {
            bail!("this virtual manifest specifies a [lib] section, which is not allowed");
        }
        if me.bin.is_some() {
            bail!("this virtual manifest specifies a [[bin]] section, which is not allowed");
        }
        if me.example.is_some() {
            bail!("this virtual manifest specifies a [[example]] section, which is not allowed");
        }
        if me.test.is_some() {
            bail!("this virtual manifest specifies a [[test]] section, which is not allowed");
        }
        if me.bench.is_some() {
            bail!("this virtual manifest specifies a [[bench]] section, which is not allowed");
        }
        if me.dependencies.is_some() {
            bail!("this virtual manifest specifies a [dependencies] section, which is not allowed");
        }
        if me.dev_dependencies().is_some() {
            bail!("this virtual manifest specifies a [dev-dependencies] section, which is not allowed");
        }
        if me.build_dependencies().is_some() {
            bail!("this virtual manifest specifies a [build-dependencies] section, which is not allowed");
        }
        if me.features.is_some() {
            bail!("this virtual manifest specifies a [features] section, which is not allowed");
        }
        if me.target.is_some() {
            bail!("this virtual manifest specifies a [target] section, which is not allowed");
        }
        if me.badges.is_some() {
            bail!("this virtual manifest specifies a [badges] section, which is not allowed");
        }

        let mut nested_paths = Vec::new();
        let mut warnings = Vec::new();
        let mut deps = Vec::new();
        let empty = Vec::new();
        let cargo_features = me.cargo_features.as_ref().unwrap_or(&empty);
        let features = Features::new(cargo_features, config, &mut warnings, source_id.is_path())?;

        let (replace, patch) = {
            let mut cx = Context {
                deps: &mut deps,
                source_id,
                nested_paths: &mut nested_paths,
                config,
                warnings: &mut warnings,
                platform: None,
                features: &features,
                root,
            };
            (me.replace(&mut cx)?, me.patch(&mut cx)?)
        };
        let profiles = me.profile.clone();
        if let Some(profiles) = &profiles {
            profiles.validate(config.cli_unstable(), &features, &mut warnings)?;
        }
        let resolve_behavior = me
            .workspace
            .as_ref()
            .and_then(|ws| ws.resolver.as_deref())
            .map(|r| ResolveBehavior::from_manifest(r))
            .transpose()?;
        let workspace_config = match me.workspace {
            Some(ref toml_config) => {
                let mut inheritable = toml_config.package.clone().unwrap_or_default();
                inheritable.update_ws_path(root.to_path_buf());
                inheritable.update_deps(toml_config.dependencies.clone());
                let lints = toml_config.lints.clone();
                let lints = verify_lints(lints)?;
                inheritable.update_lints(lints);
                let ws_root_config = WorkspaceRootConfig::new(
                    root,
                    &toml_config.members,
                    &toml_config.default_members,
                    &toml_config.exclude,
                    &Some(inheritable),
                    &toml_config.metadata,
                );
                config
                    .ws_roots
                    .borrow_mut()
                    .insert(root.to_path_buf(), ws_root_config.clone());
                WorkspaceConfig::Root(ws_root_config)
            }
            None => {
                bail!("virtual manifests must be configured with [workspace]");
            }
        };
        Ok((
            VirtualManifest::new(
                replace,
                patch,
                workspace_config,
                profiles,
                features,
                resolve_behavior,
            ),
            nested_paths,
        ))
    }

    fn replace(&self, cx: &mut Context<'_, '_>) -> CargoResult<Vec<(PackageIdSpec, Dependency)>> {
        if self.patch.is_some() && self.replace.is_some() {
            bail!("cannot specify both [replace] and [patch]");
        }
        let mut replace = Vec::new();
        for (spec, replacement) in self.replace.iter().flatten() {
            let mut spec = PackageIdSpec::parse(spec).with_context(|| {
                format!(
                    "replacements must specify a valid semver \
                     version to replace, but `{}` does not",
                    spec
                )
            })?;
            if spec.url().is_none() {
                spec.set_url(CRATES_IO_INDEX.parse().unwrap());
            }

            if replacement.is_version_specified() {
                bail!(
                    "replacements cannot specify a version \
                     requirement, but found one for `{}`",
                    spec
                );
            }

            let mut dep = replacement.to_dependency(spec.name(), cx, None)?;
            let version = spec.version().ok_or_else(|| {
                anyhow!(
                    "replacements must specify a version \
                     to replace, but `{}` does not",
                    spec
                )
            })?;
            unused_dep_keys(
                dep.name_in_toml().as_str(),
                "replace",
                replacement.unused_keys(),
                &mut cx.warnings,
            );
            dep.set_version_req(OptVersionReq::exact(&version));
            replace.push((spec, dep));
        }
        Ok(replace)
    }

    fn patch(&self, cx: &mut Context<'_, '_>) -> CargoResult<HashMap<Url, Vec<Dependency>>> {
        let mut patch = HashMap::new();
        for (toml_url, deps) in self.patch.iter().flatten() {
            let url = match &toml_url[..] {
                CRATES_IO_REGISTRY => CRATES_IO_INDEX.parse().unwrap(),
                _ => cx
                    .config
                    .get_registry_index(toml_url)
                    .or_else(|_| toml_url.into_url())
                    .with_context(|| {
                        format!(
                            "[patch] entry `{}` should be a URL or registry name",
                            toml_url
                        )
                    })?,
            };
            patch.insert(
                url,
                deps.iter()
                    .map(|(name, dep)| {
                        unused_dep_keys(
                            name,
                            &format!("patch.{toml_url}",),
                            dep.unused_keys(),
                            &mut cx.warnings,
                        );
                        dep.to_dependency(name, cx, None)
                    })
                    .collect::<CargoResult<Vec<_>>>()?,
            );
        }
        Ok(patch)
    }
}

struct Context<'a, 'b> {
    deps: &'a mut Vec<Dependency>,
    source_id: SourceId,
    nested_paths: &'a mut Vec<PathBuf>,
    config: &'b Config,
    warnings: &'a mut Vec<String>,
    platform: Option<Platform>,
    root: &'a Path,
    features: &'a Features,
}

fn verify_lints(lints: Option<schema::TomlLints>) -> CargoResult<Option<schema::TomlLints>> {
    let Some(lints) = lints else {
        return Ok(None);
    };

    for (tool, lints) in &lints {
        let supported = ["rust", "clippy", "rustdoc"];
        if !supported.contains(&tool.as_str()) {
            let supported = supported.join(", ");
            anyhow::bail!("unsupported `{tool}` in `[lints]`, must be one of {supported}")
        }
        for name in lints.keys() {
            if let Some((prefix, suffix)) = name.split_once("::") {
                if tool == prefix {
                    anyhow::bail!(
                        "`lints.{tool}.{name}` is not valid lint name; try `lints.{prefix}.{suffix}`"
                    )
                } else if tool == "rust" && supported.contains(&prefix) {
                    anyhow::bail!(
                        "`lints.{tool}.{name}` is not valid lint name; try `lints.{prefix}.{suffix}`"
                    )
                } else {
                    anyhow::bail!("`lints.{tool}.{name}` is not a valid lint name")
                }
            }
        }
    }

    Ok(Some(lints))
}

fn lints_to_rustflags(lints: &schema::TomlLints) -> Vec<String> {
    let mut rustflags = lints
        .iter()
        .flat_map(|(tool, lints)| {
            lints.iter().map(move |(name, config)| {
                let flag = config.level().flag();
                let option = if tool == "rust" {
                    format!("{flag}={name}")
                } else {
                    format!("{flag}={tool}::{name}")
                };
                (
                    config.priority(),
                    // Since the most common group will be `all`, put it last so people are more
                    // likely to notice that they need to use `priority`.
                    std::cmp::Reverse(name),
                    option,
                )
            })
        })
        .collect::<Vec<_>>();
    rustflags.sort();
    rustflags.into_iter().map(|(_, _, option)| option).collect()
}

fn unused_dep_keys(
    dep_name: &str,
    kind: &str,
    unused_keys: Vec<String>,
    warnings: &mut Vec<String>,
) {
    for unused in unused_keys {
        let key = format!("unused manifest key: {kind}.{dep_name}.{unused}");
        warnings.push(key);
    }
}

fn inheritable_from_path(
    config: &Config,
    workspace_path: PathBuf,
) -> CargoResult<schema::InheritableFields> {
    // Workspace path should have Cargo.toml at the end
    let workspace_path_root = workspace_path.parent().unwrap();

    // Let the borrow exit scope so that it can be picked up if there is a need to
    // read a manifest
    if let Some(ws_root) = config.ws_roots.borrow().get(workspace_path_root) {
        return Ok(ws_root.inheritable().clone());
    };

    let source_id = SourceId::for_path(workspace_path_root)?;
    let (man, _) = read_manifest(&workspace_path, source_id, config)?;
    match man.workspace_config() {
        WorkspaceConfig::Root(root) => {
            config
                .ws_roots
                .borrow_mut()
                .insert(workspace_path, root.clone());
            Ok(root.inheritable().clone())
        }
        _ => bail!(
            "root of a workspace inferred but wasn't a root: {}",
            workspace_path.display()
        ),
    }
}

/// Returns the name of the README file for a [`schema::TomlPackage`].
fn readme_for_package(
    package_root: &Path,
    readme: Option<&schema::StringOrBool>,
) -> Option<String> {
    match &readme {
        None => default_readme_from_package_root(package_root),
        Some(value) => match value {
            schema::StringOrBool::Bool(false) => None,
            schema::StringOrBool::Bool(true) => Some("README.md".to_string()),
            schema::StringOrBool::String(v) => Some(v.clone()),
        },
    }
}

const DEFAULT_README_FILES: [&str; 3] = ["README.md", "README.txt", "README"];

/// Checks if a file with any of the default README file names exists in the package root.
/// If so, returns a `String` representing that name.
fn default_readme_from_package_root(package_root: &Path) -> Option<String> {
    for &readme_filename in DEFAULT_README_FILES.iter() {
        if package_root.join(readme_filename).is_file() {
            return Some(readme_filename.to_string());
        }
    }

    None
}

/// Checks a list of build targets, and ensures the target names are unique within a vector.
/// If not, the name of the offending build target is returned.
fn unique_build_targets(
    targets: &[Target],
    package_root: &Path,
) -> Result<(), HashMap<PathBuf, Vec<Target>>> {
    let mut source_targets = HashMap::<_, Vec<_>>::new();
    for target in targets {
        if let TargetSourcePath::Path(path) = target.src_path() {
            let full = package_root.join(path);
            source_targets.entry(full).or_default().push(target.clone());
        }
    }

    let conflict_targets = source_targets
        .into_iter()
        .filter(|(_, targets)| targets.len() > 1)
        .collect::<HashMap<_, _>>();

    if !conflict_targets.is_empty() {
        return Err(conflict_targets);
    }

    Ok(())
}

/// Defines simple getter methods for inheritable fields.
macro_rules! inheritable_field_getter {
    ( $(($key:literal, $field:ident -> $ret:ty),)* ) => (
        $(
            #[doc = concat!("Gets the field `workspace.", $key, "`.")]
            fn $field(&self) -> CargoResult<$ret> {
                let Some(val) = &self.$field else  {
                    bail!("`workspace.{}` was not defined", $key);
                };
                Ok(val.clone())
            }
        )*
    )
}

impl schema::InheritableFields {
    inheritable_field_getter! {
        // Please keep this list lexicographically ordered.
        ("lints",                 lints         -> schema::TomlLints),
        ("package.authors",       authors       -> Vec<String>),
        ("package.badges",        badges        -> BTreeMap<String, BTreeMap<String, String>>),
        ("package.categories",    categories    -> Vec<String>),
        ("package.description",   description   -> String),
        ("package.documentation", documentation -> String),
        ("package.edition",       edition       -> String),
        ("package.exclude",       exclude       -> Vec<String>),
        ("package.homepage",      homepage      -> String),
        ("package.include",       include       -> Vec<String>),
        ("package.keywords",      keywords      -> Vec<String>),
        ("package.license",       license       -> String),
        ("package.publish",       publish       -> schema::VecStringOrBool),
        ("package.repository",    repository    -> String),
        ("package.rust-version",  rust_version  -> RustVersion),
        ("package.version",       version       -> semver::Version),
    }

    /// Gets a workspace dependency with the `name`.
    fn get_dependency(
        &self,
        name: &str,
        package_root: &Path,
    ) -> CargoResult<schema::TomlDependency> {
        let Some(deps) = &self.dependencies else {
            bail!("`workspace.dependencies` was not defined");
        };
        let Some(dep) = deps.get(name) else {
            bail!("`dependency.{name}` was not found in `workspace.dependencies`");
        };
        let mut dep = dep.clone();
        if let schema::TomlDependency::Detailed(detailed) = &mut dep {
            detailed.resolve_path(name, self.ws_root(), package_root)?;
        }
        Ok(dep)
    }

    /// Gets the field `workspace.package.license-file`.
    fn license_file(&self, package_root: &Path) -> CargoResult<String> {
        let Some(license_file) = &self.license_file else {
            bail!("`workspace.package.license-file` was not defined");
        };
        resolve_relative_path("license-file", &self.ws_root, package_root, license_file)
    }

    /// Gets the field `workspace.package.readme`.
    fn readme(&self, package_root: &Path) -> CargoResult<schema::StringOrBool> {
        let Some(readme) = readme_for_package(self.ws_root.as_path(), self.readme.as_ref()) else {
            bail!("`workspace.package.readme` was not defined");
        };
        resolve_relative_path("readme", &self.ws_root, package_root, &readme)
            .map(schema::StringOrBool::String)
    }

    fn ws_root(&self) -> &PathBuf {
        &self.ws_root
    }

    fn update_deps(&mut self, deps: Option<BTreeMap<String, schema::TomlDependency>>) {
        self.dependencies = deps;
    }

    fn update_lints(&mut self, lints: Option<schema::TomlLints>) {
        self.lints = lints;
    }

    fn update_ws_path(&mut self, ws_root: PathBuf) {
        self.ws_root = ws_root;
    }
}

impl schema::TomlPackage {
    fn to_package_id(&self, source_id: SourceId, version: semver::Version) -> PackageId {
        PackageId::pure(self.name.as_str().into(), version, source_id)
    }
}

/// This Trait exists to make [`schema::MaybeWorkspace::Workspace`] generic. It makes deserialization of
/// [`schema::MaybeWorkspace`] much easier, as well as making error messages for
/// [`schema::MaybeWorkspace::resolve`] much nicer
///
/// Implementors should have a field `workspace` with the type of `bool`. It is used to ensure
/// `workspace` is not `false` in a `Cargo.toml`
pub trait WorkspaceInherit {
    /// This is the workspace table that is being inherited from.
    /// For example `[workspace.dependencies]` would be the table "dependencies"
    fn inherit_toml_table(&self) -> &str;

    /// This is used to output the value of the implementors `workspace` field
    fn workspace(&self) -> bool;
}

impl<T, W: WorkspaceInherit> schema::MaybeWorkspace<T, W> {
    fn resolve<'a>(
        self,
        label: &str,
        get_ws_inheritable: impl FnOnce() -> CargoResult<T>,
    ) -> CargoResult<T> {
        match self {
            schema::MaybeWorkspace::Defined(value) => Ok(value),
            schema::MaybeWorkspace::Workspace(w) => get_ws_inheritable().with_context(|| {
                format!(
                "error inheriting `{label}` from workspace root manifest's `workspace.{}.{label}`",
                w.inherit_toml_table(),
            )
            }),
        }
    }

    fn resolve_with_self<'a>(
        self,
        label: &str,
        get_ws_inheritable: impl FnOnce(&W) -> CargoResult<T>,
    ) -> CargoResult<T> {
        match self {
            schema::MaybeWorkspace::Defined(value) => Ok(value),
            schema::MaybeWorkspace::Workspace(w) => get_ws_inheritable(&w).with_context(|| {
                format!(
                "error inheriting `{label}` from workspace root manifest's `workspace.{}.{label}`",
                w.inherit_toml_table(),
            )
            }),
        }
    }

    fn as_defined(&self) -> Option<&T> {
        match self {
            schema::MaybeWorkspace::Workspace(_) => None,
            schema::MaybeWorkspace::Defined(defined) => Some(defined),
        }
    }
}

impl WorkspaceInherit for schema::TomlWorkspaceField {
    fn inherit_toml_table(&self) -> &str {
        "package"
    }

    fn workspace(&self) -> bool {
        self.workspace
    }
}

impl schema::TomlWorkspaceDependency {
    fn resolve<'a>(
        &self,
        name: &str,
        inheritable: impl FnOnce() -> CargoResult<&'a schema::InheritableFields>,
        cx: &mut Context<'_, '_>,
    ) -> CargoResult<schema::TomlDependency> {
        fn default_features_msg(label: &str, ws_def_feat: Option<bool>, cx: &mut Context<'_, '_>) {
            let ws_def_feat = match ws_def_feat {
                Some(true) => "true",
                Some(false) => "false",
                None => "not specified",
            };
            cx.warnings.push(format!(
                "`default-features` is ignored for {label}, since `default-features` was \
                {ws_def_feat} for `workspace.dependencies.{label}`, \
                this could become a hard error in the future"
            ))
        }
        if self.default_features.is_some() && self.default_features2.is_some() {
            warn_on_deprecated("default-features", name, "dependency", cx.warnings);
        }
        inheritable()?.get_dependency(name, cx.root).map(|d| {
            match d {
                schema::TomlDependency::Simple(s) => {
                    if let Some(false) = self.default_features() {
                        default_features_msg(name, None, cx);
                    }
                    if self.optional.is_some() || self.features.is_some() || self.public.is_some() {
                        schema::TomlDependency::Detailed(schema::DetailedTomlDependency {
                            version: Some(s),
                            optional: self.optional,
                            features: self.features.clone(),
                            public: self.public,
                            ..Default::default()
                        })
                    } else {
                        schema::TomlDependency::Simple(s)
                    }
                }
                schema::TomlDependency::Detailed(d) => {
                    let mut d = d.clone();
                    match (self.default_features(), d.default_features()) {
                        // member: default-features = true and
                        // workspace: default-features = false should turn on
                        // default-features
                        (Some(true), Some(false)) => {
                            d.default_features = Some(true);
                        }
                        // member: default-features = false and
                        // workspace: default-features = true should ignore member
                        // default-features
                        (Some(false), Some(true)) => {
                            default_features_msg(name, Some(true), cx);
                        }
                        // member: default-features = false and
                        // workspace: dep = "1.0" should ignore member default-features
                        (Some(false), None) => {
                            default_features_msg(name, None, cx);
                        }
                        _ => {}
                    }
                    // Inherit the workspace configuration for `public` unless
                    // it's explicitly specified for this dependency.
                    if let Some(public) = self.public {
                        d.public = Some(public);
                    }
                    d.add_features(self.features.clone());
                    d.update_optional(self.optional);
                    schema::TomlDependency::Detailed(d)
                }
            }
        })
    }
}

impl WorkspaceInherit for schema::TomlWorkspaceDependency {
    fn inherit_toml_table(&self) -> &str {
        "dependencies"
    }

    fn workspace(&self) -> bool {
        self.workspace
    }
}

impl<P: ResolveToPath + Clone> schema::TomlDependency<P> {
    pub(crate) fn to_dependency_split(
        &self,
        name: &str,
        source_id: SourceId,
        nested_paths: &mut Vec<PathBuf>,
        config: &Config,
        warnings: &mut Vec<String>,
        platform: Option<Platform>,
        root: &Path,
        features: &Features,
        kind: Option<DepKind>,
    ) -> CargoResult<Dependency> {
        self.to_dependency(
            name,
            &mut Context {
                deps: &mut Vec::new(),
                source_id,
                nested_paths,
                config,
                warnings,
                platform,
                root,
                features,
            },
            kind,
        )
    }

    fn to_dependency(
        &self,
        name: &str,
        cx: &mut Context<'_, '_>,
        kind: Option<DepKind>,
    ) -> CargoResult<Dependency> {
        match *self {
            schema::TomlDependency::Simple(ref version) => schema::DetailedTomlDependency::<P> {
                version: Some(version.clone()),
                ..Default::default()
            }
            .to_dependency(name, cx, kind),
            schema::TomlDependency::Detailed(ref details) => details.to_dependency(name, cx, kind),
        }
    }
}

impl schema::DetailedTomlDependency {
    fn add_features(&mut self, features: Option<Vec<String>>) {
        self.features = match (self.features.clone(), features.clone()) {
            (Some(dep_feat), Some(inherit_feat)) => Some(
                dep_feat
                    .into_iter()
                    .chain(inherit_feat)
                    .collect::<Vec<String>>(),
            ),
            (Some(dep_fet), None) => Some(dep_fet),
            (None, Some(inherit_feat)) => Some(inherit_feat),
            (None, None) => None,
        };
    }

    fn update_optional(&mut self, optional: Option<bool>) {
        self.optional = optional;
    }

    fn resolve_path(
        &mut self,
        name: &str,
        root_path: &Path,
        package_root: &Path,
    ) -> CargoResult<()> {
        if let Some(rel_path) = &self.path {
            self.path = Some(resolve_relative_path(
                name,
                root_path,
                package_root,
                rel_path,
            )?)
        }
        Ok(())
    }
}

impl<P: ResolveToPath + Clone> schema::DetailedTomlDependency<P> {
    fn to_dependency(
        &self,
        name_in_toml: &str,
        cx: &mut Context<'_, '_>,
        kind: Option<DepKind>,
    ) -> CargoResult<Dependency> {
        if self.version.is_none() && self.path.is_none() && self.git.is_none() {
            let msg = format!(
                "dependency ({}) specified without \
                 providing a local path, Git repository, version, or \
                 workspace dependency to use. This will be considered an \
                 error in future versions",
                name_in_toml
            );
            cx.warnings.push(msg);
        }

        if let Some(version) = &self.version {
            if version.contains('+') {
                cx.warnings.push(format!(
                    "version requirement `{}` for dependency `{}` \
                     includes semver metadata which will be ignored, removing the \
                     metadata is recommended to avoid confusion",
                    version, name_in_toml
                ));
            }
        }

        if self.git.is_none() {
            let git_only_keys = [
                (&self.branch, "branch"),
                (&self.tag, "tag"),
                (&self.rev, "rev"),
            ];

            for &(key, key_name) in &git_only_keys {
                if key.is_some() {
                    bail!(
                        "key `{}` is ignored for dependency ({}).",
                        key_name,
                        name_in_toml
                    );
                }
            }
        }

        // Early detection of potentially misused feature syntax
        // instead of generating a "feature not found" error.
        if let Some(features) = &self.features {
            for feature in features {
                if feature.contains('/') {
                    bail!(
                        "feature `{}` in dependency `{}` is not allowed to contain slashes\n\
                         If you want to enable features of a transitive dependency, \
                         the direct dependency needs to re-export those features from \
                         the `[features]` table.",
                        feature,
                        name_in_toml
                    );
                }
                if feature.starts_with("dep:") {
                    bail!(
                        "feature `{}` in dependency `{}` is not allowed to use explicit \
                        `dep:` syntax\n\
                         If you want to enable an optional dependency, specify the name \
                         of the optional dependency without the `dep:` prefix, or specify \
                         a feature from the dependency's `[features]` table that enables \
                         the optional dependency.",
                        feature,
                        name_in_toml
                    );
                }
            }
        }

        let new_source_id = match (
            self.git.as_ref(),
            self.path.as_ref(),
            self.registry.as_ref(),
            self.registry_index.as_ref(),
        ) {
            (Some(_), _, Some(_), _) | (Some(_), _, _, Some(_)) => bail!(
                "dependency ({}) specification is ambiguous. \
                 Only one of `git` or `registry` is allowed.",
                name_in_toml
            ),
            (_, _, Some(_), Some(_)) => bail!(
                "dependency ({}) specification is ambiguous. \
                 Only one of `registry` or `registry-index` is allowed.",
                name_in_toml
            ),
            (Some(git), maybe_path, _, _) => {
                if maybe_path.is_some() {
                    bail!(
                        "dependency ({}) specification is ambiguous. \
                         Only one of `git` or `path` is allowed.",
                        name_in_toml
                    );
                }

                let n_details = [&self.branch, &self.tag, &self.rev]
                    .iter()
                    .filter(|d| d.is_some())
                    .count();

                if n_details > 1 {
                    bail!(
                        "dependency ({}) specification is ambiguous. \
                         Only one of `branch`, `tag` or `rev` is allowed.",
                        name_in_toml
                    );
                }

                let reference = self
                    .branch
                    .clone()
                    .map(GitReference::Branch)
                    .or_else(|| self.tag.clone().map(GitReference::Tag))
                    .or_else(|| self.rev.clone().map(GitReference::Rev))
                    .unwrap_or(GitReference::DefaultBranch);
                let loc = git.into_url()?;

                if let Some(fragment) = loc.fragment() {
                    let msg = format!(
                        "URL fragment `#{}` in git URL is ignored for dependency ({}). \
                        If you were trying to specify a specific git revision, \
                        use `rev = \"{}\"` in the dependency declaration.",
                        fragment, name_in_toml, fragment
                    );
                    cx.warnings.push(msg)
                }

                SourceId::for_git(&loc, reference)?
            }
            (None, Some(path), _, _) => {
                let path = path.resolve(cx.config);
                cx.nested_paths.push(path.clone());
                // If the source ID for the package we're parsing is a path
                // source, then we normalize the path here to get rid of
                // components like `..`.
                //
                // The purpose of this is to get a canonical ID for the package
                // that we're depending on to ensure that builds of this package
                // always end up hashing to the same value no matter where it's
                // built from.
                if cx.source_id.is_path() {
                    let path = cx.root.join(path);
                    let path = paths::normalize_path(&path);
                    SourceId::for_path(&path)?
                } else {
                    cx.source_id
                }
            }
            (None, None, Some(registry), None) => SourceId::alt_registry(cx.config, registry)?,
            (None, None, None, Some(registry_index)) => {
                let url = registry_index.into_url()?;
                SourceId::for_registry(&url)?
            }
            (None, None, None, None) => SourceId::crates_io(cx.config)?,
        };

        let (pkg_name, explicit_name_in_toml) = match self.package {
            Some(ref s) => (&s[..], Some(name_in_toml)),
            None => (name_in_toml, None),
        };

        let version = self.version.as_deref();
        let mut dep = Dependency::parse(pkg_name, version, new_source_id)?;
        if self.default_features.is_some() && self.default_features2.is_some() {
            warn_on_deprecated("default-features", name_in_toml, "dependency", cx.warnings);
        }
        dep.set_features(self.features.iter().flatten())
            .set_default_features(self.default_features().unwrap_or(true))
            .set_optional(self.optional.unwrap_or(false))
            .set_platform(cx.platform.clone());
        if let Some(registry) = &self.registry {
            let registry_id = SourceId::alt_registry(cx.config, registry)?;
            dep.set_registry_id(registry_id);
        }
        if let Some(registry_index) = &self.registry_index {
            let url = registry_index.into_url()?;
            let registry_id = SourceId::for_registry(&url)?;
            dep.set_registry_id(registry_id);
        }

        if let Some(kind) = kind {
            dep.set_kind(kind);
        }
        if let Some(name_in_toml) = explicit_name_in_toml {
            dep.set_explicit_name_in_toml(name_in_toml);
        }

        if let Some(p) = self.public {
            cx.features.require(Feature::public_dependency())?;

            if dep.kind() != DepKind::Normal {
                bail!("'public' specifier can only be used on regular dependencies, not {:?} dependencies", dep.kind());
            }

            dep.set_public(p);
        }

        if let (Some(artifact), is_lib, target) = (
            self.artifact.as_ref(),
            self.lib.unwrap_or(false),
            self.target.as_deref(),
        ) {
            if cx.config.cli_unstable().bindeps {
                let artifact = Artifact::parse(&artifact.0, is_lib, target)?;
                if dep.kind() != DepKind::Build
                    && artifact.target() == Some(ArtifactTarget::BuildDependencyAssumeTarget)
                {
                    bail!(
                        r#"`target = "target"` in normal- or dev-dependencies has no effect ({})"#,
                        name_in_toml
                    );
                }
                dep.set_artifact(artifact)
            } else {
                bail!("`artifact = …` requires `-Z bindeps` ({})", name_in_toml);
            }
        } else if self.lib.is_some() || self.target.is_some() {
            for (is_set, specifier) in [
                (self.lib.is_some(), "lib"),
                (self.target.is_some(), "target"),
            ] {
                if !is_set {
                    continue;
                }
                bail!(
                    "'{}' specifier cannot be used without an 'artifact = …' value ({})",
                    specifier,
                    name_in_toml
                )
            }
        }
        Ok(dep)
    }
}

impl schema::TomlProfiles {
    /// Checks syntax validity and unstable feature gate for each profile.
    ///
    /// It's a bit unfortunate both `-Z` flags and `cargo-features` are required,
    /// because profiles can now be set in either `Cargo.toml` or `config.toml`.
    fn validate(
        &self,
        cli_unstable: &CliUnstable,
        features: &Features,
        warnings: &mut Vec<String>,
    ) -> CargoResult<()> {
        for (name, profile) in &self.0 {
            profile.validate(name, cli_unstable, features, warnings)?;
        }
        Ok(())
    }
}

impl schema::TomlProfile {
    /// Checks stytax validity and unstable feature gate for a given profile.
    pub fn validate(
        &self,
        name: &str,
        cli_unstable: &CliUnstable,
        features: &Features,
        warnings: &mut Vec<String>,
    ) -> CargoResult<()> {
        self.validate_profile(name, cli_unstable, features)?;
        if let Some(ref profile) = self.build_override {
            profile.validate_override("build-override")?;
            profile.validate_profile(&format!("{name}.build-override"), cli_unstable, features)?;
        }
        if let Some(ref packages) = self.package {
            for (override_name, profile) in packages {
                profile.validate_override("package")?;
                profile.validate_profile(
                    &format!("{name}.package.{override_name}"),
                    cli_unstable,
                    features,
                )?;
            }
        }

        // Profile name validation
        restricted_names::validate_profile_name(name)?;

        if let Some(dir_name) = &self.dir_name {
            // This is disabled for now, as we would like to stabilize named
            // profiles without this, and then decide in the future if it is
            // needed. This helps simplify the UI a little.
            bail!(
                "dir-name=\"{}\" in profile `{}` is not currently allowed, \
                 directory names are tied to the profile name for custom profiles",
                dir_name,
                name
            );
        }

        // `inherits` validation
        if matches!(self.inherits.as_deref(), Some("debug")) {
            bail!(
                "profile.{}.inherits=\"debug\" should be profile.{}.inherits=\"dev\"",
                name,
                name
            );
        }

        match name {
            "doc" => {
                warnings.push("profile `doc` is deprecated and has no effect".to_string());
            }
            "test" | "bench" => {
                if self.panic.is_some() {
                    warnings.push(format!("`panic` setting is ignored for `{}` profile", name))
                }
            }
            _ => {}
        }

        if let Some(panic) = &self.panic {
            if panic != "unwind" && panic != "abort" {
                bail!(
                    "`panic` setting of `{}` is not a valid setting, \
                     must be `unwind` or `abort`",
                    panic
                );
            }
        }

        if let Some(schema::StringOrBool::String(arg)) = &self.lto {
            if arg == "true" || arg == "false" {
                bail!(
                    "`lto` setting of string `\"{arg}\"` for `{name}` profile is not \
                     a valid setting, must be a boolean (`true`/`false`) or a string \
                    (`\"thin\"`/`\"fat\"`/`\"off\"`) or omitted.",
                );
            }
        }

        Ok(())
    }

    /// Validates a profile.
    ///
    /// This is a shallow check, which is reused for the profile itself and any overrides.
    fn validate_profile(
        &self,
        name: &str,
        cli_unstable: &CliUnstable,
        features: &Features,
    ) -> CargoResult<()> {
        if let Some(codegen_backend) = &self.codegen_backend {
            match (
                features.require(Feature::codegen_backend()),
                cli_unstable.codegen_backend,
            ) {
                (Err(e), false) => return Err(e),
                _ => {}
            }

            if codegen_backend.contains(|c: char| !c.is_ascii_alphanumeric() && c != '_') {
                bail!(
                    "`profile.{}.codegen-backend` setting of `{}` is not a valid backend name.",
                    name,
                    codegen_backend,
                );
            }
        }
        if self.rustflags.is_some() {
            match (
                features.require(Feature::profile_rustflags()),
                cli_unstable.profile_rustflags,
            ) {
                (Err(e), false) => return Err(e),
                _ => {}
            }
        }
        if self.trim_paths.is_some() {
            match (
                features.require(Feature::trim_paths()),
                cli_unstable.trim_paths,
            ) {
                (Err(e), false) => return Err(e),
                _ => {}
            }
        }
        Ok(())
    }

    /// Validation that is specific to an override.
    fn validate_override(&self, which: &str) -> CargoResult<()> {
        if self.package.is_some() {
            bail!("package-specific profiles cannot be nested");
        }
        if self.build_override.is_some() {
            bail!("build-override profiles cannot be nested");
        }
        if self.panic.is_some() {
            bail!("`panic` may not be specified in a `{}` profile", which)
        }
        if self.lto.is_some() {
            bail!("`lto` may not be specified in a `{}` profile", which)
        }
        if self.rpath.is_some() {
            bail!("`rpath` may not be specified in a `{}` profile", which)
        }
        Ok(())
    }

    /// Overwrite self's values with the given profile.
    pub fn merge(&mut self, profile: &schema::TomlProfile) {
        if let Some(v) = &profile.opt_level {
            self.opt_level = Some(v.clone());
        }

        if let Some(v) = &profile.lto {
            self.lto = Some(v.clone());
        }

        if let Some(v) = &profile.codegen_backend {
            self.codegen_backend = Some(v.clone());
        }

        if let Some(v) = profile.codegen_units {
            self.codegen_units = Some(v);
        }

        if let Some(v) = profile.debug {
            self.debug = Some(v);
        }

        if let Some(v) = profile.debug_assertions {
            self.debug_assertions = Some(v);
        }

        if let Some(v) = &profile.split_debuginfo {
            self.split_debuginfo = Some(v.clone());
        }

        if let Some(v) = profile.rpath {
            self.rpath = Some(v);
        }

        if let Some(v) = &profile.panic {
            self.panic = Some(v.clone());
        }

        if let Some(v) = profile.overflow_checks {
            self.overflow_checks = Some(v);
        }

        if let Some(v) = profile.incremental {
            self.incremental = Some(v);
        }

        if let Some(v) = &profile.rustflags {
            self.rustflags = Some(v.clone());
        }

        if let Some(other_package) = &profile.package {
            match &mut self.package {
                Some(self_package) => {
                    for (spec, other_pkg_profile) in other_package {
                        match self_package.get_mut(spec) {
                            Some(p) => p.merge(other_pkg_profile),
                            None => {
                                self_package.insert(spec.clone(), other_pkg_profile.clone());
                            }
                        }
                    }
                }
                None => self.package = Some(other_package.clone()),
            }
        }

        if let Some(other_bo) = &profile.build_override {
            match &mut self.build_override {
                Some(self_bo) => self_bo.merge(other_bo),
                None => self.build_override = Some(other_bo.clone()),
            }
        }

        if let Some(v) = &profile.inherits {
            self.inherits = Some(v.clone());
        }

        if let Some(v) = &profile.dir_name {
            self.dir_name = Some(v.clone());
        }

        if let Some(v) = &profile.strip {
            self.strip = Some(v.clone());
        }

        if let Some(v) = &profile.trim_paths {
            self.trim_paths = Some(v.clone())
        }
    }
}

impl schema::MaybeWorkspaceLints {
    fn resolve<'a>(
        self,
        get_ws_inheritable: impl FnOnce() -> CargoResult<schema::TomlLints>,
    ) -> CargoResult<schema::TomlLints> {
        if self.workspace {
            if !self.lints.is_empty() {
                anyhow::bail!("cannot override `workspace.lints` in `lints`, either remove the overrides or `lints.workspace = true` and manually specify the lints");
            }
            get_ws_inheritable().with_context(|| {
                "error inheriting `lints` from workspace root manifest's `workspace.lints`"
            })
        } else {
            Ok(self.lints)
        }
    }
}

impl schema::TomlLintLevel {
    fn flag(&self) -> &'static str {
        match self {
            Self::Forbid => "--forbid",
            Self::Deny => "--deny",
            Self::Warn => "--warn",
            Self::Allow => "--allow",
        }
    }
}

pub trait ResolveToPath {
    fn resolve(&self, config: &Config) -> PathBuf;
}

impl ResolveToPath for String {
    fn resolve(&self, _: &Config) -> PathBuf {
        self.into()
    }
}

impl ResolveToPath for ConfigRelativePath {
    fn resolve(&self, c: &Config) -> PathBuf {
        self.resolve_path(c)
    }
}
