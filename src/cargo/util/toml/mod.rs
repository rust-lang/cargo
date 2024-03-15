use annotate_snippets::{Annotation, AnnotationType, Renderer, Slice, Snippet, SourceAnnotation};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::str::{self, FromStr};

use crate::AlreadyPrintedError;
use anyhow::{anyhow, bail, Context as _};
use cargo_platform::Platform;
use cargo_util::paths;
use cargo_util_schemas::manifest::RustVersion;
use cargo_util_schemas::manifest::{self, TomlManifest};
use itertools::Itertools;
use lazycell::LazyCell;
use pathdiff::diff_paths;
use url::Url;

use crate::core::compiler::{CompileKind, CompileTarget};
use crate::core::dependency::{Artifact, ArtifactTarget, DepKind};
use crate::core::manifest::{ManifestMetadata, TargetSourcePath};
use crate::core::resolver::ResolveBehavior;
use crate::core::{find_workspace_root, resolve_relative_path, CliUnstable, FeatureValue};
use crate::core::{Dependency, Manifest, PackageId, Summary, Target};
use crate::core::{Edition, EitherManifest, Feature, Features, VirtualManifest, Workspace};
use crate::core::{GitReference, PackageIdSpec, SourceId, WorkspaceConfig, WorkspaceRootConfig};
use crate::sources::{CRATES_IO_INDEX, CRATES_IO_REGISTRY};
use crate::util::errors::{CargoResult, ManifestError};
use crate::util::interning::InternedString;
use crate::util::{self, context::ConfigRelativePath, GlobalContext, IntoUrl, OptVersionReq};

mod embedded;
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
#[tracing::instrument(skip(gctx))]
pub fn read_manifest(
    path: &Path,
    source_id: SourceId,
    gctx: &GlobalContext,
) -> CargoResult<EitherManifest> {
    let contents =
        read_toml_string(path, gctx).map_err(|err| ManifestError::new(err, path.into()))?;
    let document =
        parse_document(&contents).map_err(|e| emit_diagnostic(e.into(), &contents, path, gctx))?;
    let toml = deserialize_toml(&document)
        .map_err(|e| emit_diagnostic(e.into(), &contents, path, gctx))?;

    (|| {
        if toml.package().is_some() {
            to_real_manifest(contents, document, toml, source_id, path, gctx)
                .map(EitherManifest::Real)
        } else {
            to_virtual_manifest(toml, source_id, path, gctx).map(EitherManifest::Virtual)
        }
    })()
    .map_err(|err| {
        ManifestError::new(
            err.context(format!("failed to parse manifest at `{}`", path.display())),
            path.into(),
        )
        .into()
    })
}

#[tracing::instrument(skip_all)]
fn read_toml_string(path: &Path, gctx: &GlobalContext) -> CargoResult<String> {
    let mut contents = paths::read(path)?;
    if is_embedded(path) {
        if !gctx.cli_unstable().script {
            anyhow::bail!("parsing `{}` requires `-Zscript`", path.display());
        }
        contents = embedded::expand_manifest(&contents, path, gctx)?;
    }
    Ok(contents)
}

#[tracing::instrument(skip_all)]
fn parse_document(contents: &str) -> Result<toml_edit::ImDocument<String>, toml_edit::de::Error> {
    toml_edit::ImDocument::parse(contents.to_owned()).map_err(Into::into)
}

#[tracing::instrument(skip_all)]
fn deserialize_toml(
    document: &toml_edit::ImDocument<String>,
) -> Result<manifest::TomlManifest, toml_edit::de::Error> {
    let mut unused = BTreeSet::new();
    let deserializer = toml_edit::de::Deserializer::from(document.clone());
    let mut document: manifest::TomlManifest = serde_ignored::deserialize(deserializer, |path| {
        let mut key = String::new();
        stringify(&mut key, &path);
        unused.insert(key);
    })?;
    document._unused_keys = unused;
    Ok(document)
}

/// See also `bin/cargo/commands/run.rs`s `is_manifest_command`
pub fn is_embedded(path: &Path) -> bool {
    let ext = path.extension();
    ext == Some(OsStr::new("rs")) ||
        // Provide better errors by not considering directories to be embedded manifests
        (ext.is_none() && path.is_file())
}

fn emit_diagnostic(
    e: toml_edit::de::Error,
    contents: &str,
    manifest_file: &Path,
    gctx: &GlobalContext,
) -> anyhow::Error {
    let Some(span) = e.span() else {
        return e.into();
    };

    let (line_num, column) = translate_position(&contents, span.start);
    let source_start = contents[0..span.start]
        .rfind('\n')
        .map(|s| s + 1)
        .unwrap_or(0);
    let source_end = contents[span.end.saturating_sub(1)..]
        .find('\n')
        .map(|s| s + span.end)
        .unwrap_or(contents.len());
    let source = &contents[source_start..source_end];
    // Make sure we don't try to highlight past the end of the line,
    // but also make sure we are highlighting at least one character
    let highlight_end = (column + contents[span].chars().count())
        .min(source.len())
        .max(column + 1);
    // Get the path to the manifest, relative to the cwd
    let manifest_path = diff_paths(manifest_file, gctx.cwd())
        .unwrap_or_else(|| manifest_file.to_path_buf())
        .display()
        .to_string();
    let snippet = Snippet {
        title: Some(Annotation {
            id: None,
            label: Some(e.message()),
            annotation_type: AnnotationType::Error,
        }),
        footer: vec![],
        slices: vec![Slice {
            source: &source,
            line_start: line_num + 1,
            origin: Some(manifest_path.as_str()),
            annotations: vec![SourceAnnotation {
                range: (column, highlight_end),
                label: "",
                annotation_type: AnnotationType::Error,
            }],
            fold: false,
        }],
    };
    let renderer = Renderer::styled();
    if let Err(err) = writeln!(gctx.shell().err(), "{}", renderer.render(snippet)) {
        return err.into();
    }
    return AlreadyPrintedError::new(e.into()).into();
}

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

/// Warn about paths that have been deprecated and may conflict.
fn warn_on_deprecated(new_path: &str, name: &str, kind: &str, warnings: &mut Vec<String>) {
    let old_path = new_path.replace("-", "_");
    warnings.push(format!(
        "conflicting between `{new_path}` and `{old_path}` in the `{name}` {kind}.\n
        `{old_path}` is ignored and not recommended for use in the future"
    ))
}

fn warn_on_unused(unused: &BTreeSet<String>, warnings: &mut Vec<String>) {
    for key in unused {
        warnings.push(format!("unused manifest key: {}", key));
        if key == "profiles.debug" {
            warnings.push("use `[profile.dev]` to configure debug builds".to_string());
        }
    }
}

/// Prepares the manifest for publishing.
// - Path and git components of dependency specifications are removed.
// - License path is updated to point within the package.
pub fn prepare_for_publish(
    me: &manifest::TomlManifest,
    ws: &Workspace<'_>,
    package_root: &Path,
) -> CargoResult<manifest::TomlManifest> {
    let gctx = ws.gctx();

    if me
        .cargo_features
        .iter()
        .flat_map(|f| f.iter())
        .any(|f| f == "open-namespaces")
    {
        anyhow::bail!("cannot publish with `open-namespaces`")
    }

    let mut package = me.package().unwrap().clone();
    package.workspace = None;
    let current_resolver = package
        .resolver
        .as_ref()
        .map(|r| ResolveBehavior::from_manifest(r))
        .unwrap_or_else(|| {
            package
                .edition
                .as_ref()
                .and_then(|e| e.as_value())
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
            .as_value()
            .context("license file should have been resolved before `prepare_for_publish()`")?;
        let license_path = Path::new(&license_file);
        let abs_license_path = paths::normalize_path(&package_root.join(license_path));
        if abs_license_path.strip_prefix(package_root).is_err() {
            // This path points outside of the package root. `cargo package`
            // will copy it into the root, so adjust the path to this location.
            package.license_file = Some(manifest::InheritableField::Value(
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
            .as_value()
            .context("readme should have been resolved before `prepare_for_publish()`")?;
        match readme {
            manifest::StringOrBool::String(readme) => {
                let readme_path = Path::new(&readme);
                let abs_readme_path = paths::normalize_path(&package_root.join(readme_path));
                if abs_readme_path.strip_prefix(package_root).is_err() {
                    // This path points outside of the package root. `cargo package`
                    // will copy it into the root, so adjust the path to this location.
                    package.readme = Some(manifest::InheritableField::Value(
                        manifest::StringOrBool::String(
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
            manifest::StringOrBool::Bool(_) => {}
        }
    }
    let all = |_d: &manifest::TomlDependency| true;
    let mut manifest = manifest::TomlManifest {
        package: Some(package),
        project: None,
        profile: me.profile.clone(),
        lib: me.lib.clone(),
        bin: me.bin.clone(),
        example: me.example.clone(),
        test: me.test.clone(),
        bench: me.bench.clone(),
        dependencies: map_deps(gctx, me.dependencies.as_ref(), all)?,
        dev_dependencies: map_deps(
            gctx,
            me.dev_dependencies(),
            manifest::TomlDependency::is_version_specified,
        )?,
        dev_dependencies2: None,
        build_dependencies: map_deps(gctx, me.build_dependencies(), all)?,
        build_dependencies2: None,
        features: me.features.clone(),
        target: match me.target.as_ref().map(|target_map| {
            target_map
                .iter()
                .map(|(k, v)| {
                    Ok((
                        k.clone(),
                        manifest::TomlPlatform {
                            dependencies: map_deps(gctx, v.dependencies.as_ref(), all)?,
                            dev_dependencies: map_deps(
                                gctx,
                                v.dev_dependencies(),
                                manifest::TomlDependency::is_version_specified,
                            )?,
                            dev_dependencies2: None,
                            build_dependencies: map_deps(gctx, v.build_dependencies(), all)?,
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
        badges: me.badges.clone(),
        cargo_features: me.cargo_features.clone(),
        lints: me.lints.clone(),
        _unused_keys: Default::default(),
    };
    strip_features(&mut manifest);
    return Ok(manifest);

    fn strip_features(manifest: &mut TomlManifest) {
        fn insert_dep_name(
            dep_name_set: &mut BTreeSet<manifest::PackageName>,
            deps: Option<&BTreeMap<manifest::PackageName, manifest::InheritableDependency>>,
        ) {
            let Some(deps) = deps else {
                return;
            };
            deps.iter().for_each(|(k, _v)| {
                dep_name_set.insert(k.clone());
            });
        }
        let mut dep_name_set = BTreeSet::new();
        insert_dep_name(&mut dep_name_set, manifest.dependencies.as_ref());
        insert_dep_name(&mut dep_name_set, manifest.dev_dependencies());
        insert_dep_name(&mut dep_name_set, manifest.build_dependencies());
        if let Some(target_map) = manifest.target.as_ref() {
            target_map.iter().for_each(|(_k, v)| {
                insert_dep_name(&mut dep_name_set, v.dependencies.as_ref());
                insert_dep_name(&mut dep_name_set, v.dev_dependencies());
                insert_dep_name(&mut dep_name_set, v.build_dependencies());
            });
        }
        let features = manifest.features.as_mut();

        let Some(features) = features else {
            return;
        };

        features.values_mut().for_each(|feature_deps| {
            feature_deps.retain(|feature_dep| {
                let feature_value = FeatureValue::new(InternedString::new(feature_dep));
                match feature_value {
                    FeatureValue::Dep { dep_name } | FeatureValue::DepFeature { dep_name, .. } => {
                        let k = &manifest::PackageName::new(dep_name.to_string()).unwrap();
                        dep_name_set.contains(k)
                    }
                    _ => true,
                }
            });
        });
    }

    fn map_deps(
        gctx: &GlobalContext,
        deps: Option<&BTreeMap<manifest::PackageName, manifest::InheritableDependency>>,
        filter: impl Fn(&manifest::TomlDependency) -> bool,
    ) -> CargoResult<Option<BTreeMap<manifest::PackageName, manifest::InheritableDependency>>> {
        let Some(deps) = deps else {
            return Ok(None);
        };
        let deps = deps
            .iter()
            .filter(|(_k, v)| {
                if let manifest::InheritableDependency::Value(def) = v {
                    filter(def)
                } else {
                    false
                }
            })
            .map(|(k, v)| Ok((k.clone(), map_dependency(gctx, v)?)))
            .collect::<CargoResult<BTreeMap<_, _>>>()?;
        Ok(Some(deps))
    }

    fn map_dependency(
        gctx: &GlobalContext,
        dep: &manifest::InheritableDependency,
    ) -> CargoResult<manifest::InheritableDependency> {
        let dep = match dep {
            manifest::InheritableDependency::Value(manifest::TomlDependency::Detailed(d)) => {
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
                    d.registry_index = Some(gctx.get_registry_index(&registry)?.to_string());
                }
                Ok(d)
            }
            manifest::InheritableDependency::Value(manifest::TomlDependency::Simple(s)) => {
                Ok(manifest::TomlDetailedDependency {
                    version: Some(s.clone()),
                    ..Default::default()
                })
            }
            _ => unreachable!(),
        };
        dep.map(manifest::TomlDependency::Detailed)
            .map(manifest::InheritableDependency::Value)
    }
}

#[tracing::instrument(skip_all)]
pub fn to_real_manifest(
    contents: String,
    document: toml_edit::ImDocument<String>,
    me: manifest::TomlManifest,
    source_id: SourceId,
    manifest_file: &Path,
    gctx: &GlobalContext,
) -> CargoResult<Manifest> {
    fn get_ws(
        gctx: &GlobalContext,
        resolved_path: &Path,
        workspace_config: &WorkspaceConfig,
    ) -> CargoResult<InheritableFields> {
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
                inheritable_from_path(gctx, root_path)
            }
            WorkspaceConfig::Member { root: None } => {
                match find_workspace_root(&resolved_path, gctx)? {
                    Some(path_to_root) => inheritable_from_path(gctx, path_to_root),
                    None => Err(anyhow!("failed to find a workspace root")),
                }
            }
        }
    }

    let embedded = is_embedded(manifest_file);
    let package_root = manifest_file.parent().unwrap();
    if !package_root.is_dir() {
        bail!(
            "package root '{}' is not a directory",
            package_root.display()
        );
    };

    if let Some(deps) = me
        .workspace
        .as_ref()
        .and_then(|ws| ws.dependencies.as_ref())
    {
        for (name, dep) in deps {
            if dep.is_optional() {
                bail!("{name} is optional, but workspace dependencies cannot be optional",);
            }
            if dep.is_public() {
                bail!("{name} is public, but workspace dependencies cannot be public",);
            }
        }
    }

    let mut warnings = vec![];
    let mut errors = vec![];

    warn_on_unused(&me._unused_keys, &mut warnings);

    // Parse features first so they will be available when parsing other parts of the TOML.
    let empty = Vec::new();
    let cargo_features = me.cargo_features.as_ref().unwrap_or(&empty);
    let features = Features::new(cargo_features, gctx, &mut warnings, source_id.is_path())?;

    let mut package = match (&me.package, &me.project) {
        (Some(_), Some(project)) => {
            if source_id.is_path() {
                gctx.shell().warn(format!(
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
                gctx.shell().warn(format!(
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
            let lints = toml_config.lints.clone();
            let lints = verify_lints(lints)?;
            let inheritable = InheritableFields {
                package: toml_config.package.clone(),
                dependencies: toml_config.dependencies.clone(),
                lints,
                _ws_root: package_root.to_path_buf(),
            };
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
            gctx.ws_roots
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
    if package_name.contains(':') {
        features.require(Feature::open_namespaces())?;
    }

    let resolved_path = package_root.join("Cargo.toml");

    let inherit_cell: LazyCell<InheritableFields> = LazyCell::new();
    let inherit =
        || inherit_cell.try_borrow_with(|| get_ws(gctx, &resolved_path, &workspace_config));

    let version = package
        .version
        .clone()
        .map(|version| field_inherit_with(version, "version", || inherit()?.version()))
        .transpose()?;

    package.version = version.clone().map(manifest::InheritableField::Value);

    let pkgid = PackageId::new(
        package.name.as_str().into(),
        version
            .clone()
            .unwrap_or_else(|| semver::Version::new(0, 0, 0)),
        source_id,
    );

    let rust_version = if let Some(rust_version) = &package.rust_version {
        let rust_version = field_inherit_with(rust_version.clone(), "rust_version", || {
            inherit()?.rust_version()
        })?;
        Some(rust_version)
    } else {
        None
    };

    let edition = if let Some(edition) = package.edition.clone() {
        let edition: Edition = field_inherit_with(edition, "edition", || inherit()?.edition())?
            .parse()
            .with_context(|| "failed to parse the `edition` key")?;
        package.edition = Some(manifest::InheritableField::Value(edition.to_string()));
        if let Some(pkg_msrv) = &rust_version {
            if let Some(edition_msrv) = edition.first_version() {
                let edition_msrv = RustVersion::try_from(edition_msrv).unwrap();
                if !edition_msrv.is_compatible_with(pkg_msrv.as_partial()) {
                    bail!(
                        "rust-version {} is older than first version ({}) required by \
                            the specified edition ({})",
                        pkg_msrv,
                        edition_msrv,
                        edition,
                    )
                }
            }
        }
        edition
    } else {
        let msrv_edition = if let Some(pkg_msrv) = &rust_version {
            Edition::ALL
                .iter()
                .filter(|e| {
                    e.first_version()
                        .map(|e| {
                            let e = RustVersion::try_from(e).unwrap();
                            e.is_compatible_with(pkg_msrv.as_partial())
                        })
                        .unwrap_or_default()
                })
                .max()
                .copied()
        } else {
            None
        }
        .unwrap_or_default();
        let default_edition = Edition::default();
        let latest_edition = Edition::LATEST_STABLE;

        // We're trying to help the user who might assume they are using a new edition,
        // so if they can't use a new edition, don't bother to tell them to set it.
        // This also avoids having to worry about whether `package.edition` is compatible with
        // their MSRV.
        if msrv_edition != default_edition {
            let tip = if msrv_edition == latest_edition {
                format!(" while the latest is {latest_edition}")
            } else {
                format!(" while {msrv_edition} is compatible with `rust-version`")
            };
            warnings.push(format!(
                "no edition set: defaulting to the {default_edition} edition{tip}",
            ));
        }
        default_edition
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

    if targets.iter().all(|t| t.is_custom_build()) {
        bail!(
            "no targets specified in the manifest\n\
                 either src/lib.rs, src/main.rs, a [lib] section, or \
                 [[bin]] section must be present"
        )
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
                        .map(|t| format!("  * `{}` target `{}`", t.kind().description(), t.name(),))
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

    let mut manifest_ctx = ManifestContext {
        deps: &mut deps,
        source_id,
        gctx,
        warnings: &mut warnings,
        features: &features,
        platform: None,
        root: package_root,
    };

    #[tracing::instrument(skip(manifest_ctx, new_deps, workspace_config, inherit_cell))]
    fn process_dependencies(
        manifest_ctx: &mut ManifestContext<'_, '_>,
        new_deps: Option<&BTreeMap<manifest::PackageName, manifest::InheritableDependency>>,
        kind: Option<DepKind>,
        workspace_config: &WorkspaceConfig,
        inherit_cell: &LazyCell<InheritableFields>,
    ) -> CargoResult<Option<BTreeMap<manifest::PackageName, manifest::InheritableDependency>>> {
        let Some(dependencies) = new_deps else {
            return Ok(None);
        };

        let inheritable = || {
            inherit_cell.try_borrow_with(|| {
                get_ws(
                    manifest_ctx.gctx,
                    &manifest_ctx.root.join("Cargo.toml"),
                    &workspace_config,
                )
            })
        };

        let mut deps: BTreeMap<manifest::PackageName, manifest::InheritableDependency> =
            BTreeMap::new();
        for (n, v) in dependencies.iter() {
            let resolved = dependency_inherit_with(v.clone(), n, inheritable, manifest_ctx)?;
            let dep = dep_to_dependency(&resolved, n, manifest_ctx, kind)?;
            let name_in_toml = dep.name_in_toml().as_str();
            let kind_name = match kind {
                Some(k) => k.kind_table(),
                None => "dependencies",
            };
            let table_in_toml = if let Some(platform) = &manifest_ctx.platform {
                format!("target.{}.{kind_name}", platform.to_string())
            } else {
                kind_name.to_string()
            };
            unused_dep_keys(
                name_in_toml,
                &table_in_toml,
                v.unused_keys(),
                manifest_ctx.warnings,
            );
            let mut resolved = resolved;
            if let manifest::TomlDependency::Detailed(ref mut d) = resolved {
                if d.public.is_some() {
                    if matches!(dep.kind(), DepKind::Normal) {
                        if !manifest_ctx
                            .features
                            .require(Feature::public_dependency())
                            .is_ok()
                            && !manifest_ctx.gctx.cli_unstable().public_dependency
                        {
                            d.public = None;
                            manifest_ctx.warnings.push(format!(
                            "ignoring `public` on dependency {name}, pass `-Zpublic-dependency` to enable support for it", name = &dep.name_in_toml()
                        ))
                        }
                    } else {
                        d.public = None;
                    }
                }
            }

            manifest_ctx.deps.push(dep);
            deps.insert(
                n.clone(),
                manifest::InheritableDependency::Value(resolved.clone()),
            );
        }
        Ok(Some(deps))
    }

    // Collect the dependencies.
    let dependencies = process_dependencies(
        &mut manifest_ctx,
        me.dependencies.as_ref(),
        None,
        &workspace_config,
        &inherit_cell,
    )?;
    if me.dev_dependencies.is_some() && me.dev_dependencies2.is_some() {
        warn_on_deprecated(
            "dev-dependencies",
            package_name,
            "package",
            manifest_ctx.warnings,
        );
    }
    let dev_deps = me.dev_dependencies();
    let dev_deps = process_dependencies(
        &mut manifest_ctx,
        dev_deps,
        Some(DepKind::Development),
        &workspace_config,
        &inherit_cell,
    )?;
    if me.build_dependencies.is_some() && me.build_dependencies2.is_some() {
        warn_on_deprecated(
            "build-dependencies",
            package_name,
            "package",
            manifest_ctx.warnings,
        );
    }
    let build_deps = me.build_dependencies();
    let build_deps = process_dependencies(
        &mut manifest_ctx,
        build_deps,
        Some(DepKind::Build),
        &workspace_config,
        &inherit_cell,
    )?;

    let lints = me
        .lints
        .clone()
        .map(|mw| lints_inherit_with(mw, || inherit()?.lints()))
        .transpose()?;
    let lints = verify_lints(lints)?;
    let default = manifest::TomlLints::default();
    let rustflags = lints_to_rustflags(lints.as_ref().unwrap_or(&default));

    let mut target: BTreeMap<String, manifest::TomlPlatform> = BTreeMap::new();
    for (name, platform) in me.target.iter().flatten() {
        manifest_ctx.platform = {
            let platform: Platform = name.parse()?;
            platform.check_cfg_attributes(manifest_ctx.warnings);
            Some(platform)
        };
        let deps = process_dependencies(
            &mut manifest_ctx,
            platform.dependencies.as_ref(),
            None,
            &workspace_config,
            &inherit_cell,
        )?;
        if platform.build_dependencies.is_some() && platform.build_dependencies2.is_some() {
            warn_on_deprecated(
                "build-dependencies",
                name,
                "platform target",
                manifest_ctx.warnings,
            );
        }
        let build_deps = platform.build_dependencies();
        let build_deps = process_dependencies(
            &mut manifest_ctx,
            build_deps,
            Some(DepKind::Build),
            &workspace_config,
            &inherit_cell,
        )?;
        if platform.dev_dependencies.is_some() && platform.dev_dependencies2.is_some() {
            warn_on_deprecated(
                "dev-dependencies",
                name,
                "platform target",
                manifest_ctx.warnings,
            );
        }
        let dev_deps = platform.dev_dependencies();
        let dev_deps = process_dependencies(
            &mut manifest_ctx,
            dev_deps,
            Some(DepKind::Development),
            &workspace_config,
            &inherit_cell,
        )?;
        target.insert(
            name.clone(),
            manifest::TomlPlatform {
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
    let replace = replace(&me, &mut manifest_ctx)?;
    let patch = patch(&me, &mut manifest_ctx)?;

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
        .map(|mw| field_inherit_with(mw, "exclude", || inherit()?.exclude()))
        .transpose()?
        .unwrap_or_default();
    let include = package
        .include
        .clone()
        .map(|mw| field_inherit_with(mw, "include", || inherit()?.include()))
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
            .map(|mw| field_inherit_with(mw, "description", || inherit()?.description()))
            .transpose()?,
        homepage: package
            .homepage
            .clone()
            .map(|mw| field_inherit_with(mw, "homepage", || inherit()?.homepage()))
            .transpose()?,
        documentation: package
            .documentation
            .clone()
            .map(|mw| field_inherit_with(mw, "documentation", || inherit()?.documentation()))
            .transpose()?,
        readme: readme_for_package(
            package_root,
            package
                .readme
                .clone()
                .map(|mw| field_inherit_with(mw, "readme", || inherit()?.readme(package_root)))
                .transpose()?
                .as_ref(),
        ),
        authors: package
            .authors
            .clone()
            .map(|mw| field_inherit_with(mw, "authors", || inherit()?.authors()))
            .transpose()?
            .unwrap_or_default(),
        license: package
            .license
            .clone()
            .map(|mw| field_inherit_with(mw, "license", || inherit()?.license()))
            .transpose()?,
        license_file: package
            .license_file
            .clone()
            .map(|mw| field_inherit_with(mw, "license", || inherit()?.license_file(package_root)))
            .transpose()?,
        repository: package
            .repository
            .clone()
            .map(|mw| field_inherit_with(mw, "repository", || inherit()?.repository()))
            .transpose()?,
        keywords: package
            .keywords
            .clone()
            .map(|mw| field_inherit_with(mw, "keywords", || inherit()?.keywords()))
            .transpose()?
            .unwrap_or_default(),
        categories: package
            .categories
            .clone()
            .map(|mw| field_inherit_with(mw, "categories", || inherit()?.categories()))
            .transpose()?
            .unwrap_or_default(),
        badges: me
            .badges
            .clone()
            .map(|mw| field_inherit_with(mw, "badges", || inherit()?.badges()))
            .transpose()?
            .unwrap_or_default(),
        links: package.links.clone(),
        rust_version: package
            .rust_version
            .map(|mw| field_inherit_with(mw, "rust-version", || inherit()?.rust_version()))
            .transpose()?,
    };
    package.description = metadata
        .description
        .clone()
        .map(|description| manifest::InheritableField::Value(description));
    package.homepage = metadata
        .homepage
        .clone()
        .map(|homepage| manifest::InheritableField::Value(homepage));
    package.documentation = metadata
        .documentation
        .clone()
        .map(|documentation| manifest::InheritableField::Value(documentation));
    package.readme = metadata
        .readme
        .clone()
        .map(|readme| manifest::InheritableField::Value(manifest::StringOrBool::String(readme)));
    package.authors = package
        .authors
        .as_ref()
        .map(|_| manifest::InheritableField::Value(metadata.authors.clone()));
    package.license = metadata
        .license
        .clone()
        .map(|license| manifest::InheritableField::Value(license));
    package.license_file = metadata
        .license_file
        .clone()
        .map(|license_file| manifest::InheritableField::Value(license_file));
    package.repository = metadata
        .repository
        .clone()
        .map(|repository| manifest::InheritableField::Value(repository));
    package.keywords = package
        .keywords
        .as_ref()
        .map(|_| manifest::InheritableField::Value(metadata.keywords.clone()));
    package.categories = package
        .categories
        .as_ref()
        .map(|_| manifest::InheritableField::Value(metadata.categories.clone()));
    package.rust_version = rust_version
        .clone()
        .map(|rv| manifest::InheritableField::Value(rv));
    package.exclude = package
        .exclude
        .as_ref()
        .map(|_| manifest::InheritableField::Value(exclude.clone()));
    package.include = package
        .include
        .as_ref()
        .map(|_| manifest::InheritableField::Value(include.clone()));

    let profiles = me.profile.clone();
    if let Some(profiles) = &profiles {
        let cli_unstable = gctx.cli_unstable();
        validate_profiles(profiles, cli_unstable, &features, &mut warnings)?;
    }

    let publish = package
        .publish
        .clone()
        .map(|publish| field_inherit_with(publish, "publish", || inherit()?.publish()).unwrap());

    package.publish = publish
        .clone()
        .map(|p| manifest::InheritableField::Value(p));

    let publish = match publish {
        Some(manifest::VecStringOrBool::VecString(ref vecstring)) => Some(vecstring.clone()),
        Some(manifest::VecStringOrBool::Bool(false)) => Some(vec![]),
        Some(manifest::VecStringOrBool::Bool(true)) => None,
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
    let resolved_toml = manifest::TomlManifest {
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
            .map(|_| manifest::InheritableField::Value(metadata.badges.clone())),
        lints: lints.map(|lints| manifest::InheritableLints {
            workspace: false,
            lints,
        }),
        _unused_keys: Default::default(),
    };
    let mut manifest = Manifest::new(
        Rc::new(contents),
        Rc::new(document),
        Rc::new(resolved_toml),
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

    Ok(manifest)
}

fn to_virtual_manifest(
    me: manifest::TomlManifest,
    source_id: SourceId,
    manifest_file: &Path,
    gctx: &GlobalContext,
) -> CargoResult<VirtualManifest> {
    let root = manifest_file.parent().unwrap();

    if let Some(deps) = me
        .workspace
        .as_ref()
        .and_then(|ws| ws.dependencies.as_ref())
    {
        for (name, dep) in deps {
            if dep.is_optional() {
                bail!("{name} is optional, but workspace dependencies cannot be optional",);
            }
            if dep.is_public() {
                bail!("{name} is public, but workspace dependencies cannot be public",);
            }
        }
    }

    for field in me.requires_package() {
        bail!("this virtual manifest specifies a `{field}` section, which is not allowed");
    }

    let mut warnings = Vec::new();
    let mut deps = Vec::new();
    let empty = Vec::new();
    let cargo_features = me.cargo_features.as_ref().unwrap_or(&empty);
    let features = Features::new(cargo_features, gctx, &mut warnings, source_id.is_path())?;

    warn_on_unused(&me._unused_keys, &mut warnings);

    let (replace, patch) = {
        let mut manifest_ctx = ManifestContext {
            deps: &mut deps,
            source_id,
            gctx,
            warnings: &mut warnings,
            platform: None,
            features: &features,
            root,
        };
        (
            replace(&me, &mut manifest_ctx)?,
            patch(&me, &mut manifest_ctx)?,
        )
    };
    let profiles = me.profile.clone();
    if let Some(profiles) = &profiles {
        validate_profiles(profiles, gctx.cli_unstable(), &features, &mut warnings)?;
    }
    let resolve_behavior = me
        .workspace
        .as_ref()
        .and_then(|ws| ws.resolver.as_deref())
        .map(|r| ResolveBehavior::from_manifest(r))
        .transpose()?;
    let workspace_config = match me.workspace {
        Some(ref toml_config) => {
            let lints = toml_config.lints.clone();
            let lints = verify_lints(lints)?;
            let inheritable = InheritableFields {
                package: toml_config.package.clone(),
                dependencies: toml_config.dependencies.clone(),
                lints,
                _ws_root: root.to_path_buf(),
            };
            let ws_root_config = WorkspaceRootConfig::new(
                root,
                &toml_config.members,
                &toml_config.default_members,
                &toml_config.exclude,
                &Some(inheritable),
                &toml_config.metadata,
            );
            gctx.ws_roots
                .borrow_mut()
                .insert(root.to_path_buf(), ws_root_config.clone());
            WorkspaceConfig::Root(ws_root_config)
        }
        None => {
            bail!("virtual manifests must be configured with [workspace]");
        }
    };
    let mut manifest = VirtualManifest::new(
        replace,
        patch,
        workspace_config,
        profiles,
        features,
        resolve_behavior,
    );
    for warning in warnings {
        manifest.warnings_mut().add_warning(warning);
    }

    Ok(manifest)
}

fn replace(
    me: &manifest::TomlManifest,
    manifest_ctx: &mut ManifestContext<'_, '_>,
) -> CargoResult<Vec<(PackageIdSpec, Dependency)>> {
    if me.patch.is_some() && me.replace.is_some() {
        bail!("cannot specify both [replace] and [patch]");
    }
    let mut replace = Vec::new();
    for (spec, replacement) in me.replace.iter().flatten() {
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

        let mut dep = dep_to_dependency(replacement, spec.name(), manifest_ctx, None)?;
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
            &mut manifest_ctx.warnings,
        );
        dep.set_version_req(OptVersionReq::exact(&version));
        replace.push((spec, dep));
    }
    Ok(replace)
}

fn patch(
    me: &manifest::TomlManifest,
    manifest_ctx: &mut ManifestContext<'_, '_>,
) -> CargoResult<HashMap<Url, Vec<Dependency>>> {
    let mut patch = HashMap::new();
    for (toml_url, deps) in me.patch.iter().flatten() {
        let url = match &toml_url[..] {
            CRATES_IO_REGISTRY => CRATES_IO_INDEX.parse().unwrap(),
            _ => manifest_ctx
                .gctx
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
                        &mut manifest_ctx.warnings,
                    );
                    dep_to_dependency(dep, name, manifest_ctx, None)
                })
                .collect::<CargoResult<Vec<_>>>()?,
        );
    }
    Ok(patch)
}

struct ManifestContext<'a, 'b> {
    deps: &'a mut Vec<Dependency>,
    source_id: SourceId,
    gctx: &'b GlobalContext,
    warnings: &'a mut Vec<String>,
    platform: Option<Platform>,
    root: &'a Path,
    features: &'a Features,
}

fn verify_lints(lints: Option<manifest::TomlLints>) -> CargoResult<Option<manifest::TomlLints>> {
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

fn lints_to_rustflags(lints: &manifest::TomlLints) -> Vec<String> {
    let mut rustflags = lints
        .iter()
        .flat_map(|(tool, lints)| {
            lints.iter().map(move |(name, config)| {
                let flag = match config.level() {
                    manifest::TomlLintLevel::Forbid => "--forbid",
                    manifest::TomlLintLevel::Deny => "--deny",
                    manifest::TomlLintLevel::Warn => "--warn",
                    manifest::TomlLintLevel::Allow => "--allow",
                };

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
    gctx: &GlobalContext,
    workspace_path: PathBuf,
) -> CargoResult<InheritableFields> {
    // Workspace path should have Cargo.toml at the end
    let workspace_path_root = workspace_path.parent().unwrap();

    // Let the borrow exit scope so that it can be picked up if there is a need to
    // read a manifest
    if let Some(ws_root) = gctx.ws_roots.borrow().get(workspace_path_root) {
        return Ok(ws_root.inheritable().clone());
    };

    let source_id = SourceId::for_path(workspace_path_root)?;
    let man = read_manifest(&workspace_path, source_id, gctx)?;
    match man.workspace_config() {
        WorkspaceConfig::Root(root) => {
            gctx.ws_roots
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

/// Returns the name of the README file for a [`manifest::TomlPackage`].
fn readme_for_package(
    package_root: &Path,
    readme: Option<&manifest::StringOrBool>,
) -> Option<String> {
    match &readme {
        None => default_readme_from_package_root(package_root),
        Some(value) => match value {
            manifest::StringOrBool::Bool(false) => None,
            manifest::StringOrBool::Bool(true) => Some("README.md".to_string()),
            manifest::StringOrBool::String(v) => Some(v.clone()),
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
#[tracing::instrument(skip_all)]
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
macro_rules! package_field_getter {
    ( $(($key:literal, $field:ident -> $ret:ty),)* ) => (
        $(
            #[doc = concat!("Gets the field `workspace.package", $key, "`.")]
            fn $field(&self) -> CargoResult<$ret> {
                let Some(val) = self.package.as_ref().and_then(|p| p.$field.as_ref()) else  {
                    bail!("`workspace.package.{}` was not defined", $key);
                };
                Ok(val.clone())
            }
        )*
    )
}

/// A group of fields that are inheritable by members of the workspace
#[derive(Clone, Debug, Default)]
pub struct InheritableFields {
    package: Option<manifest::InheritablePackage>,
    dependencies: Option<BTreeMap<manifest::PackageName, manifest::TomlDependency>>,
    lints: Option<manifest::TomlLints>,

    // Bookkeeping to help when resolving values from above
    _ws_root: PathBuf,
}

impl InheritableFields {
    package_field_getter! {
        // Please keep this list lexicographically ordered.
        ("authors",       authors       -> Vec<String>),
        ("badges",        badges        -> BTreeMap<String, BTreeMap<String, String>>),
        ("categories",    categories    -> Vec<String>),
        ("description",   description   -> String),
        ("documentation", documentation -> String),
        ("edition",       edition       -> String),
        ("exclude",       exclude       -> Vec<String>),
        ("homepage",      homepage      -> String),
        ("include",       include       -> Vec<String>),
        ("keywords",      keywords      -> Vec<String>),
        ("license",       license       -> String),
        ("publish",       publish       -> manifest::VecStringOrBool),
        ("repository",    repository    -> String),
        ("rust-version",  rust_version  -> RustVersion),
        ("version",       version       -> semver::Version),
    }

    /// Gets a workspace dependency with the `name`.
    fn get_dependency(
        &self,
        name: &str,
        package_root: &Path,
    ) -> CargoResult<manifest::TomlDependency> {
        let Some(deps) = &self.dependencies else {
            bail!("`workspace.dependencies` was not defined");
        };
        let Some(dep) = deps.get(name) else {
            bail!("`dependency.{name}` was not found in `workspace.dependencies`");
        };
        let mut dep = dep.clone();
        if let manifest::TomlDependency::Detailed(detailed) = &mut dep {
            if let Some(rel_path) = &detailed.path {
                detailed.path = Some(resolve_relative_path(
                    name,
                    self.ws_root(),
                    package_root,
                    rel_path,
                )?);
            }
        }
        Ok(dep)
    }

    /// Gets the field `workspace.lint`.
    fn lints(&self) -> CargoResult<manifest::TomlLints> {
        let Some(val) = &self.lints else {
            bail!("`workspace.lints` was not defined");
        };
        Ok(val.clone())
    }

    /// Gets the field `workspace.package.license-file`.
    fn license_file(&self, package_root: &Path) -> CargoResult<String> {
        let Some(license_file) = self.package.as_ref().and_then(|p| p.license_file.as_ref()) else {
            bail!("`workspace.package.license-file` was not defined");
        };
        resolve_relative_path("license-file", &self._ws_root, package_root, license_file)
    }

    /// Gets the field `workspace.package.readme`.
    fn readme(&self, package_root: &Path) -> CargoResult<manifest::StringOrBool> {
        let Some(readme) = readme_for_package(
            self._ws_root.as_path(),
            self.package.as_ref().and_then(|p| p.readme.as_ref()),
        ) else {
            bail!("`workspace.package.readme` was not defined");
        };
        resolve_relative_path("readme", &self._ws_root, package_root, &readme)
            .map(manifest::StringOrBool::String)
    }

    fn ws_root(&self) -> &PathBuf {
        &self._ws_root
    }
}

fn field_inherit_with<'a, T>(
    field: manifest::InheritableField<T>,
    label: &str,
    get_ws_inheritable: impl FnOnce() -> CargoResult<T>,
) -> CargoResult<T> {
    match field {
        manifest::InheritableField::Value(value) => Ok(value),
        manifest::InheritableField::Inherit(_) => get_ws_inheritable().with_context(|| {
            format!(
                "error inheriting `{label}` from workspace root manifest's `workspace.package.{label}`",
            )
        }),
    }
}

fn lints_inherit_with(
    lints: manifest::InheritableLints,
    get_ws_inheritable: impl FnOnce() -> CargoResult<manifest::TomlLints>,
) -> CargoResult<manifest::TomlLints> {
    if lints.workspace {
        if !lints.lints.is_empty() {
            anyhow::bail!("cannot override `workspace.lints` in `lints`, either remove the overrides or `lints.workspace = true` and manually specify the lints");
        }
        get_ws_inheritable().with_context(|| {
            "error inheriting `lints` from workspace root manifest's `workspace.lints`"
        })
    } else {
        Ok(lints.lints)
    }
}

fn dependency_inherit_with<'a>(
    dependency: manifest::InheritableDependency,
    name: &str,
    inheritable: impl FnOnce() -> CargoResult<&'a InheritableFields>,
    manifest_ctx: &mut ManifestContext<'_, '_>,
) -> CargoResult<manifest::TomlDependency> {
    match dependency {
        manifest::InheritableDependency::Value(value) => Ok(value),
        manifest::InheritableDependency::Inherit(w) => {
            inner_dependency_inherit_with(w, name, inheritable, manifest_ctx).with_context(|| {
                format!(
                    "error inheriting `{name}` from workspace root manifest's `workspace.dependencies.{name}`",
                )
            })
        }
    }
}

fn inner_dependency_inherit_with<'a>(
    dependency: manifest::TomlInheritedDependency,
    name: &str,
    inheritable: impl FnOnce() -> CargoResult<&'a InheritableFields>,
    manifest_ctx: &mut ManifestContext<'_, '_>,
) -> CargoResult<manifest::TomlDependency> {
    fn default_features_msg(
        label: &str,
        ws_def_feat: Option<bool>,
        manifest_ctx: &mut ManifestContext<'_, '_>,
    ) {
        let ws_def_feat = match ws_def_feat {
            Some(true) => "true",
            Some(false) => "false",
            None => "not specified",
        };
        manifest_ctx.warnings.push(format!(
            "`default-features` is ignored for {label}, since `default-features` was \
                {ws_def_feat} for `workspace.dependencies.{label}`, \
                this could become a hard error in the future"
        ))
    }
    if dependency.default_features.is_some() && dependency.default_features2.is_some() {
        warn_on_deprecated(
            "default-features",
            name,
            "dependency",
            manifest_ctx.warnings,
        );
    }
    inheritable()?
        .get_dependency(name, manifest_ctx.root)
        .map(|d| {
            match d {
                manifest::TomlDependency::Simple(s) => {
                    if let Some(false) = dependency.default_features() {
                        default_features_msg(name, None, manifest_ctx);
                    }
                    if dependency.optional.is_some()
                        || dependency.features.is_some()
                        || dependency.public.is_some()
                    {
                        manifest::TomlDependency::Detailed(manifest::TomlDetailedDependency {
                            version: Some(s),
                            optional: dependency.optional,
                            features: dependency.features.clone(),
                            public: dependency.public,
                            ..Default::default()
                        })
                    } else {
                        manifest::TomlDependency::Simple(s)
                    }
                }
                manifest::TomlDependency::Detailed(d) => {
                    let mut d = d.clone();
                    match (dependency.default_features(), d.default_features()) {
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
                            default_features_msg(name, Some(true), manifest_ctx);
                        }
                        // member: default-features = false and
                        // workspace: dep = "1.0" should ignore member default-features
                        (Some(false), None) => {
                            default_features_msg(name, None, manifest_ctx);
                        }
                        _ => {}
                    }
                    d.features = match (d.features.clone(), dependency.features.clone()) {
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
                    d.optional = dependency.optional;
                    manifest::TomlDependency::Detailed(d)
                }
            }
        })
}

pub(crate) fn to_dependency<P: ResolveToPath + Clone>(
    dep: &manifest::TomlDependency<P>,
    name: &str,
    source_id: SourceId,
    gctx: &GlobalContext,
    warnings: &mut Vec<String>,
    platform: Option<Platform>,
    root: &Path,
    features: &Features,
    kind: Option<DepKind>,
) -> CargoResult<Dependency> {
    dep_to_dependency(
        dep,
        name,
        &mut ManifestContext {
            deps: &mut Vec::new(),
            source_id,
            gctx,
            warnings,
            platform,
            root,
            features,
        },
        kind,
    )
}

fn dep_to_dependency<P: ResolveToPath + Clone>(
    orig: &manifest::TomlDependency<P>,
    name: &str,
    manifest_ctx: &mut ManifestContext<'_, '_>,
    kind: Option<DepKind>,
) -> CargoResult<Dependency> {
    match *orig {
        manifest::TomlDependency::Simple(ref version) => detailed_dep_to_dependency(
            &manifest::TomlDetailedDependency::<P> {
                version: Some(version.clone()),
                ..Default::default()
            },
            name,
            manifest_ctx,
            kind,
        ),
        manifest::TomlDependency::Detailed(ref details) => {
            detailed_dep_to_dependency(details, name, manifest_ctx, kind)
        }
    }
}

fn detailed_dep_to_dependency<P: ResolveToPath + Clone>(
    orig: &manifest::TomlDetailedDependency<P>,
    name_in_toml: &str,
    manifest_ctx: &mut ManifestContext<'_, '_>,
    kind: Option<DepKind>,
) -> CargoResult<Dependency> {
    if orig.version.is_none() && orig.path.is_none() && orig.git.is_none() {
        let msg = format!(
            "dependency ({}) specified without \
                 providing a local path, Git repository, version, or \
                 workspace dependency to use. This will be considered an \
                 error in future versions",
            name_in_toml
        );
        manifest_ctx.warnings.push(msg);
    }

    if let Some(version) = &orig.version {
        if version.contains('+') {
            manifest_ctx.warnings.push(format!(
                "version requirement `{}` for dependency `{}` \
                     includes semver metadata which will be ignored, removing the \
                     metadata is recommended to avoid confusion",
                version, name_in_toml
            ));
        }
    }

    if orig.git.is_none() {
        let git_only_keys = [
            (&orig.branch, "branch"),
            (&orig.tag, "tag"),
            (&orig.rev, "rev"),
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
    if let Some(features) = &orig.features {
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
        orig.git.as_ref(),
        orig.path.as_ref(),
        orig.registry.as_ref(),
        orig.registry_index.as_ref(),
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

            let n_details = [&orig.branch, &orig.tag, &orig.rev]
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

            let reference = orig
                .branch
                .clone()
                .map(GitReference::Branch)
                .or_else(|| orig.tag.clone().map(GitReference::Tag))
                .or_else(|| orig.rev.clone().map(GitReference::Rev))
                .unwrap_or(GitReference::DefaultBranch);
            let loc = git.into_url()?;

            if let Some(fragment) = loc.fragment() {
                let msg = format!(
                    "URL fragment `#{}` in git URL is ignored for dependency ({}). \
                        If you were trying to specify a specific git revision, \
                        use `rev = \"{}\"` in the dependency declaration.",
                    fragment, name_in_toml, fragment
                );
                manifest_ctx.warnings.push(msg)
            }

            SourceId::for_git(&loc, reference)?
        }
        (None, Some(path), _, _) => {
            let path = path.resolve(manifest_ctx.gctx);
            // If the source ID for the package we're parsing is a path
            // source, then we normalize the path here to get rid of
            // components like `..`.
            //
            // The purpose of this is to get a canonical ID for the package
            // that we're depending on to ensure that builds of this package
            // always end up hashing to the same value no matter where it's
            // built from.
            if manifest_ctx.source_id.is_path() {
                let path = manifest_ctx.root.join(path);
                let path = paths::normalize_path(&path);
                SourceId::for_path(&path)?
            } else {
                manifest_ctx.source_id
            }
        }
        (None, None, Some(registry), None) => SourceId::alt_registry(manifest_ctx.gctx, registry)?,
        (None, None, None, Some(registry_index)) => {
            let url = registry_index.into_url()?;
            SourceId::for_registry(&url)?
        }
        (None, None, None, None) => SourceId::crates_io(manifest_ctx.gctx)?,
    };

    let (pkg_name, explicit_name_in_toml) = match orig.package {
        Some(ref s) => (&s[..], Some(name_in_toml)),
        None => (name_in_toml, None),
    };

    let version = orig.version.as_deref();
    let mut dep = Dependency::parse(pkg_name, version, new_source_id)?;
    if orig.default_features.is_some() && orig.default_features2.is_some() {
        warn_on_deprecated(
            "default-features",
            name_in_toml,
            "dependency",
            manifest_ctx.warnings,
        );
    }
    dep.set_features(orig.features.iter().flatten())
        .set_default_features(orig.default_features().unwrap_or(true))
        .set_optional(orig.optional.unwrap_or(false))
        .set_platform(manifest_ctx.platform.clone());
    if let Some(registry) = &orig.registry {
        let registry_id = SourceId::alt_registry(manifest_ctx.gctx, registry)?;
        dep.set_registry_id(registry_id);
    }
    if let Some(registry_index) = &orig.registry_index {
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

    if let Some(p) = orig.public {
        let public_feature = manifest_ctx.features.require(Feature::public_dependency());
        let with_z_public = manifest_ctx.gctx.cli_unstable().public_dependency;
        let with_public_feature = public_feature.is_ok();
        if !with_public_feature && (!with_z_public && !manifest_ctx.gctx.nightly_features_allowed) {
            public_feature?;
        }

        if dep.kind() != DepKind::Normal {
            let hint = format!(
                "'public' specifier can only be used on regular dependencies, not {}",
                dep.kind().kind_table(),
            );
            match (with_public_feature, with_z_public) {
                (true, _) | (_, true) => bail!(hint),
                // If public feature isn't enabled in nightly, we instead warn that.
                (false, false) => manifest_ctx.warnings.push(hint),
            }
        } else {
            dep.set_public(p);
        }
    }

    if let (Some(artifact), is_lib, target) = (
        orig.artifact.as_ref(),
        orig.lib.unwrap_or(false),
        orig.target.as_deref(),
    ) {
        if manifest_ctx.gctx.cli_unstable().bindeps {
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
    } else if orig.lib.is_some() || orig.target.is_some() {
        for (is_set, specifier) in [
            (orig.lib.is_some(), "lib"),
            (orig.target.is_some(), "target"),
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

/// Checks syntax validity and unstable feature gate for each profile.
///
/// It's a bit unfortunate both `-Z` flags and `cargo-features` are required,
/// because profiles can now be set in either `Cargo.toml` or `config.toml`.
fn validate_profiles(
    profiles: &manifest::TomlProfiles,
    cli_unstable: &CliUnstable,
    features: &Features,
    warnings: &mut Vec<String>,
) -> CargoResult<()> {
    for (name, profile) in &profiles.0 {
        validate_profile(profile, name, cli_unstable, features, warnings)?;
    }
    Ok(())
}

/// Checks stytax validity and unstable feature gate for a given profile.
pub fn validate_profile(
    root: &manifest::TomlProfile,
    name: &str,
    cli_unstable: &CliUnstable,
    features: &Features,
    warnings: &mut Vec<String>,
) -> CargoResult<()> {
    validate_profile_layer(root, name, cli_unstable, features)?;
    if let Some(ref profile) = root.build_override {
        validate_profile_override(profile, "build-override")?;
        validate_profile_layer(
            profile,
            &format!("{name}.build-override"),
            cli_unstable,
            features,
        )?;
    }
    if let Some(ref packages) = root.package {
        for (override_name, profile) in packages {
            validate_profile_override(profile, "package")?;
            validate_profile_layer(
                profile,
                &format!("{name}.package.{override_name}"),
                cli_unstable,
                features,
            )?;
        }
    }

    if let Some(dir_name) = &root.dir_name {
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
    if matches!(root.inherits.as_deref(), Some("debug")) {
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
            if root.panic.is_some() {
                warnings.push(format!("`panic` setting is ignored for `{}` profile", name))
            }
        }
        _ => {}
    }

    if let Some(panic) = &root.panic {
        if panic != "unwind" && panic != "abort" {
            bail!(
                "`panic` setting of `{}` is not a valid setting, \
                     must be `unwind` or `abort`",
                panic
            );
        }
    }

    if let Some(manifest::StringOrBool::String(arg)) = &root.lto {
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
fn validate_profile_layer(
    profile: &manifest::TomlProfile,
    name: &str,
    cli_unstable: &CliUnstable,
    features: &Features,
) -> CargoResult<()> {
    if let Some(codegen_backend) = &profile.codegen_backend {
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
    if profile.rustflags.is_some() {
        match (
            features.require(Feature::profile_rustflags()),
            cli_unstable.profile_rustflags,
        ) {
            (Err(e), false) => return Err(e),
            _ => {}
        }
    }
    if profile.trim_paths.is_some() {
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
fn validate_profile_override(profile: &manifest::TomlProfile, which: &str) -> CargoResult<()> {
    if profile.package.is_some() {
        bail!("package-specific profiles cannot be nested");
    }
    if profile.build_override.is_some() {
        bail!("build-override profiles cannot be nested");
    }
    if profile.panic.is_some() {
        bail!("`panic` may not be specified in a `{}` profile", which)
    }
    if profile.lto.is_some() {
        bail!("`lto` may not be specified in a `{}` profile", which)
    }
    if profile.rpath.is_some() {
        bail!("`rpath` may not be specified in a `{}` profile", which)
    }
    Ok(())
}

pub trait ResolveToPath {
    fn resolve(&self, gctx: &GlobalContext) -> PathBuf;
}

impl ResolveToPath for String {
    fn resolve(&self, _: &GlobalContext) -> PathBuf {
        self.into()
    }
}

impl ResolveToPath for ConfigRelativePath {
    fn resolve(&self, gctx: &GlobalContext) -> PathBuf {
        self.resolve_path(gctx)
    }
}

fn translate_position(input: &str, index: usize) -> (usize, usize) {
    if input.is_empty() {
        return (0, index);
    }

    let safe_index = index.min(input.len() - 1);
    let column_offset = index - safe_index;

    let nl = input[0..safe_index]
        .as_bytes()
        .iter()
        .rev()
        .enumerate()
        .find(|(_, b)| **b == b'\n')
        .map(|(nl, _)| safe_index - nl - 1);
    let line_start = match nl {
        Some(nl) => nl + 1,
        None => 0,
    };
    let line = input[0..line_start]
        .as_bytes()
        .iter()
        .filter(|c| **c == b'\n')
        .count();
    let column = input[line_start..=safe_index].chars().count() - 1;
    let column = column + column_offset;

    (line, column)
}
