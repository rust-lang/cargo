use annotate_snippets::{AnnotationKind, Group, Level, Snippet};
use std::borrow::Cow;
use std::cell::OnceCell;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::str::{self, FromStr};

use crate::AlreadyPrintedError;
use crate::core::summary::MissingDependencyError;
use anyhow::{Context as _, anyhow, bail};
use cargo_platform::Platform;
use cargo_util::paths;
use cargo_util_schemas::manifest::{
    self, PackageName, PathBaseName, TomlDependency, TomlDetailedDependency, TomlManifest,
    TomlPackageBuild, TomlWorkspace,
};
use cargo_util_schemas::manifest::{RustVersion, StringOrBool};
use itertools::Itertools;
use pathdiff::diff_paths;
use url::Url;

use crate::core::compiler::{CompileKind, CompileTarget};
use crate::core::dependency::{Artifact, ArtifactTarget, DepKind};
use crate::core::manifest::{ManifestMetadata, TargetSourcePath};
use crate::core::resolver::ResolveBehavior;
use crate::core::{CliUnstable, FeatureValue, find_workspace_root, resolve_relative_path};
use crate::core::{Dependency, Manifest, Package, PackageId, Summary, Target};
use crate::core::{Edition, EitherManifest, Feature, Features, VirtualManifest, Workspace};
use crate::core::{GitReference, PackageIdSpec, SourceId, WorkspaceConfig, WorkspaceRootConfig};
use crate::sources::{CRATES_IO_INDEX, CRATES_IO_REGISTRY};
use crate::util::errors::{CargoResult, ManifestError};
use crate::util::interning::InternedString;
use crate::util::lints::{get_key_value_span, rel_cwd_manifest_path};
use crate::util::{
    self, GlobalContext, IntoUrl, OnceExt, OptVersionReq, context::ConfigRelativePath,
};

mod embedded;
mod targets;

use self::targets::to_targets;

/// See also `bin/cargo/commands/run.rs`s `is_manifest_command`
pub fn is_embedded(path: &Path) -> bool {
    let ext = path.extension();
    (ext == Some(OsStr::new("rs")) ||
        // Provide better errors by not considering directories to be embedded manifests
        ext.is_none())
        && path.is_file()
}

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
    let mut warnings = Default::default();
    let mut errors = Default::default();

    let is_embedded = is_embedded(path);
    let contents = read_toml_string(path, is_embedded, gctx)?;
    let document = parse_document(&contents)
        .map_err(|e| emit_toml_diagnostic(e.into(), &contents, path, gctx))?;
    let original_toml = deserialize_toml(&document)
        .map_err(|e| emit_toml_diagnostic(e.into(), &contents, path, gctx))?;

    let mut manifest = (|| {
        let empty = Vec::new();
        let cargo_features = original_toml.cargo_features.as_ref().unwrap_or(&empty);
        let features = Features::new(cargo_features, gctx, &mut warnings, source_id.is_path())?;
        let workspace_config =
            to_workspace_config(&original_toml, path, is_embedded, gctx, &mut warnings)?;
        if let WorkspaceConfig::Root(ws_root_config) = &workspace_config {
            let package_root = path.parent().unwrap();
            gctx.ws_roots()
                .insert(package_root.to_owned(), ws_root_config.clone());
        }
        let normalized_toml = normalize_toml(
            &original_toml,
            &features,
            &workspace_config,
            path,
            is_embedded,
            gctx,
            &mut warnings,
            &mut errors,
        )?;

        if normalized_toml.package().is_some() {
            to_real_manifest(
                contents,
                document,
                original_toml,
                normalized_toml,
                features,
                workspace_config,
                source_id,
                path,
                is_embedded,
                gctx,
                &mut warnings,
                &mut errors,
            )
            .map(EitherManifest::Real)
        } else if normalized_toml.workspace.is_some() {
            assert!(!is_embedded);
            to_virtual_manifest(
                contents,
                document,
                original_toml,
                normalized_toml,
                features,
                workspace_config,
                source_id,
                path,
                gctx,
                &mut warnings,
                &mut errors,
            )
            .map(EitherManifest::Virtual)
        } else {
            anyhow::bail!("manifest is missing either a `[package]` or a `[workspace]`")
        }
    })()
    .map_err(|err| {
        ManifestError::new(
            err.context(format!("failed to parse manifest at `{}`", path.display())),
            path.into(),
        )
    })?;

    for warning in warnings {
        manifest.warnings_mut().add_warning(warning);
    }
    for error in errors {
        manifest.warnings_mut().add_critical_warning(error);
    }

    Ok(manifest)
}

#[tracing::instrument(skip_all)]
fn read_toml_string(path: &Path, is_embedded: bool, gctx: &GlobalContext) -> CargoResult<String> {
    let mut contents = paths::read(path).map_err(|err| ManifestError::new(err, path.into()))?;
    if is_embedded {
        if !gctx.cli_unstable().script {
            anyhow::bail!("parsing `{}` requires `-Zscript`", path.display());
        }
        contents = embedded::expand_manifest(&contents)
            .map_err(|e| emit_frontmatter_diagnostic(e, &contents, path, gctx))?;
    }
    Ok(contents)
}

#[tracing::instrument(skip_all)]
fn parse_document(
    contents: &str,
) -> Result<toml::Spanned<toml::de::DeTable<'static>>, toml::de::Error> {
    let mut table = toml::de::DeTable::parse(contents)?;
    table.get_mut().make_owned();
    // SAFETY: `DeTable::make_owned` ensures no borrows remain and the lifetime does not affect
    // layout
    let table = unsafe {
        std::mem::transmute::<
            toml::Spanned<toml::de::DeTable<'_>>,
            toml::Spanned<toml::de::DeTable<'static>>,
        >(table)
    };
    Ok(table)
}

#[tracing::instrument(skip_all)]
fn deserialize_toml(
    document: &toml::Spanned<toml::de::DeTable<'static>>,
) -> Result<manifest::TomlManifest, toml::de::Error> {
    let mut unused = BTreeSet::new();
    let deserializer = toml::de::Deserializer::from(document.clone());
    let mut document: manifest::TomlManifest = serde_ignored::deserialize(deserializer, |path| {
        let mut key = String::new();
        stringify(&mut key, &path);
        unused.insert(key);
    })?;
    document._unused_keys = unused;
    Ok(document)
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

fn to_workspace_config(
    original_toml: &manifest::TomlManifest,
    manifest_file: &Path,
    is_embedded: bool,
    gctx: &GlobalContext,
    warnings: &mut Vec<String>,
) -> CargoResult<WorkspaceConfig> {
    if is_embedded {
        let ws_root_config = to_workspace_root_config(&TomlWorkspace::default(), manifest_file);
        return Ok(WorkspaceConfig::Root(ws_root_config));
    }
    let workspace_config = match (
        original_toml.workspace.as_ref(),
        original_toml.package().and_then(|p| p.workspace.as_ref()),
    ) {
        (Some(toml_config), None) => {
            verify_lints(toml_config.lints.as_ref(), gctx, warnings)?;
            if let Some(ws_deps) = &toml_config.dependencies {
                for (name, dep) in ws_deps {
                    if dep.is_optional() {
                        bail!("{name} is optional, but workspace dependencies cannot be optional",);
                    }
                    if dep.is_public() {
                        bail!("{name} is public, but workspace dependencies cannot be public",);
                    }
                }

                for (name, dep) in ws_deps {
                    unused_dep_keys(name, "workspace.dependencies", dep.unused_keys(), warnings);
                }
            }
            let ws_root_config = to_workspace_root_config(toml_config, manifest_file);
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
    Ok(workspace_config)
}

fn to_workspace_root_config(
    normalized_toml: &manifest::TomlWorkspace,
    manifest_file: &Path,
) -> WorkspaceRootConfig {
    let package_root = manifest_file.parent().unwrap();
    let inheritable = InheritableFields {
        package: normalized_toml.package.clone(),
        dependencies: normalized_toml.dependencies.clone(),
        lints: normalized_toml.lints.clone(),
        _ws_root: package_root.to_owned(),
    };
    let ws_root_config = WorkspaceRootConfig::new(
        package_root,
        &normalized_toml.members,
        &normalized_toml.default_members,
        &normalized_toml.exclude,
        &Some(inheritable),
        &normalized_toml.metadata,
    );
    ws_root_config
}

/// See [`Manifest::normalized_toml`] for more details
#[tracing::instrument(skip_all)]
fn normalize_toml(
    original_toml: &manifest::TomlManifest,
    features: &Features,
    workspace_config: &WorkspaceConfig,
    manifest_file: &Path,
    is_embedded: bool,
    gctx: &GlobalContext,
    warnings: &mut Vec<String>,
    errors: &mut Vec<String>,
) -> CargoResult<manifest::TomlManifest> {
    let package_root = manifest_file.parent().unwrap();

    let inherit_cell: OnceCell<InheritableFields> = OnceCell::new();
    let inherit = || {
        inherit_cell
            .try_borrow_with(|| load_inheritable_fields(gctx, manifest_file, &workspace_config))
    };
    let workspace_root = || inherit().map(|fields| fields.ws_root().as_path());

    let mut normalized_toml = manifest::TomlManifest {
        cargo_features: original_toml.cargo_features.clone(),
        package: None,
        project: None,
        badges: None,
        features: None,
        lib: None,
        bin: None,
        example: None,
        test: None,
        bench: None,
        dependencies: None,
        dev_dependencies: None,
        dev_dependencies2: None,
        build_dependencies: None,
        build_dependencies2: None,
        target: None,
        lints: None,
        hints: None,
        workspace: original_toml.workspace.clone().or_else(|| {
            // Prevent looking for a workspace by `read_manifest_from_str`
            is_embedded.then(manifest::TomlWorkspace::default)
        }),
        profile: original_toml.profile.clone(),
        patch: normalize_patch(
            gctx,
            original_toml.patch.as_ref(),
            &workspace_root,
            features,
        )?,
        replace: original_toml.replace.clone(),
        _unused_keys: Default::default(),
    };

    if let Some(original_package) = original_toml.package().map(Cow::Borrowed).or_else(|| {
        if is_embedded {
            Some(Cow::Owned(Box::new(manifest::TomlPackage::default())))
        } else {
            None
        }
    }) {
        let normalized_package = normalize_package_toml(
            &original_package,
            manifest_file,
            is_embedded,
            gctx,
            &inherit,
            features,
        )?;
        let package_name = &normalized_package
            .normalized_name()
            .expect("previously normalized")
            .clone();
        let edition = normalized_package
            .normalized_edition()
            .expect("previously normalized")
            .map_or(Edition::default(), |e| {
                Edition::from_str(&e).unwrap_or_default()
            });
        normalized_toml.package = Some(normalized_package);

        normalized_toml.features = normalize_features(original_toml.features.as_ref())?;

        let auto_embedded = is_embedded.then_some(false);
        normalized_toml.lib = targets::normalize_lib(
            original_toml.lib.as_ref(),
            package_root,
            package_name,
            edition,
            original_package.autolib.or(auto_embedded),
            warnings,
        )?;
        let original_toml_bin = if is_embedded {
            let name = package_name.as_ref().to_owned();
            let manifest_file_name = manifest_file
                .file_name()
                .expect("file name enforced previously");
            let path = PathBuf::from(manifest_file_name);
            Cow::Owned(Some(vec![manifest::TomlBinTarget {
                name: Some(name),
                crate_type: None,
                crate_type2: None,
                path: Some(manifest::PathValue(path)),
                filename: None,
                test: None,
                doctest: None,
                bench: None,
                doc: None,
                doc_scrape_examples: None,
                proc_macro: None,
                proc_macro2: None,
                harness: None,
                required_features: None,
                edition: None,
            }]))
        } else {
            Cow::Borrowed(&original_toml.bin)
        };
        normalized_toml.bin = Some(targets::normalize_bins(
            original_toml_bin.as_ref().as_ref(),
            package_root,
            package_name,
            edition,
            original_package.autobins.or(auto_embedded),
            warnings,
            errors,
            normalized_toml.lib.is_some(),
        )?);
        normalized_toml.example = Some(targets::normalize_examples(
            original_toml.example.as_ref(),
            package_root,
            edition,
            original_package.autoexamples.or(auto_embedded),
            warnings,
            errors,
        )?);
        normalized_toml.test = Some(targets::normalize_tests(
            original_toml.test.as_ref(),
            package_root,
            edition,
            original_package.autotests.or(auto_embedded),
            warnings,
            errors,
        )?);
        normalized_toml.bench = Some(targets::normalize_benches(
            original_toml.bench.as_ref(),
            package_root,
            edition,
            original_package.autobenches.or(auto_embedded),
            warnings,
            errors,
        )?);

        normalized_toml.dependencies = normalize_dependencies(
            gctx,
            edition,
            &features,
            original_toml.dependencies.as_ref(),
            DepKind::Normal,
            &inherit,
            &workspace_root,
            package_root,
            warnings,
        )?;
        deprecated_underscore(
            &original_toml.dev_dependencies2,
            &original_toml.dev_dependencies,
            "dev-dependencies",
            package_name,
            "package",
            edition,
            warnings,
        )?;
        normalized_toml.dev_dependencies = normalize_dependencies(
            gctx,
            edition,
            &features,
            original_toml.dev_dependencies(),
            DepKind::Development,
            &inherit,
            &workspace_root,
            package_root,
            warnings,
        )?;
        deprecated_underscore(
            &original_toml.build_dependencies2,
            &original_toml.build_dependencies,
            "build-dependencies",
            package_name,
            "package",
            edition,
            warnings,
        )?;
        normalized_toml.build_dependencies = normalize_dependencies(
            gctx,
            edition,
            &features,
            original_toml.build_dependencies(),
            DepKind::Build,
            &inherit,
            &workspace_root,
            package_root,
            warnings,
        )?;
        let mut normalized_target = BTreeMap::new();
        for (name, platform) in original_toml.target.iter().flatten() {
            let normalized_dependencies = normalize_dependencies(
                gctx,
                edition,
                &features,
                platform.dependencies.as_ref(),
                DepKind::Normal,
                &inherit,
                &workspace_root,
                package_root,
                warnings,
            )?;
            deprecated_underscore(
                &platform.dev_dependencies2,
                &platform.dev_dependencies,
                "dev-dependencies",
                name,
                "platform target",
                edition,
                warnings,
            )?;
            let normalized_dev_dependencies = normalize_dependencies(
                gctx,
                edition,
                &features,
                platform.dev_dependencies(),
                DepKind::Development,
                &inherit,
                &workspace_root,
                package_root,
                warnings,
            )?;
            deprecated_underscore(
                &platform.build_dependencies2,
                &platform.build_dependencies,
                "build-dependencies",
                name,
                "platform target",
                edition,
                warnings,
            )?;
            let normalized_build_dependencies = normalize_dependencies(
                gctx,
                edition,
                &features,
                platform.build_dependencies(),
                DepKind::Build,
                &inherit,
                &workspace_root,
                package_root,
                warnings,
            )?;
            normalized_target.insert(
                name.clone(),
                manifest::TomlPlatform {
                    dependencies: normalized_dependencies,
                    build_dependencies: normalized_build_dependencies,
                    build_dependencies2: None,
                    dev_dependencies: normalized_dev_dependencies,
                    dev_dependencies2: None,
                },
            );
        }
        normalized_toml.target = (!normalized_target.is_empty()).then_some(normalized_target);

        let normalized_lints = original_toml
            .lints
            .clone()
            .map(|value| lints_inherit_with(value, || inherit()?.lints()))
            .transpose()?;
        normalized_toml.lints = normalized_lints.map(|lints| manifest::InheritableLints {
            workspace: false,
            lints,
        });

        normalized_toml.hints = original_toml.hints.clone();

        normalized_toml.badges = original_toml.badges.clone();
    } else {
        if let Some(field) = original_toml.requires_package().next() {
            bail!("this virtual manifest specifies a `{field}` section, which is not allowed");
        }
    }

    Ok(normalized_toml)
}

fn normalize_patch<'a>(
    gctx: &GlobalContext,
    original_patch: Option<&BTreeMap<String, BTreeMap<PackageName, TomlDependency>>>,
    workspace_root: &dyn Fn() -> CargoResult<&'a Path>,
    features: &Features,
) -> CargoResult<Option<BTreeMap<String, BTreeMap<PackageName, TomlDependency>>>> {
    if let Some(patch) = original_patch {
        let mut normalized_patch = BTreeMap::new();
        for (name, packages) in patch {
            let mut normalized_packages = BTreeMap::new();
            for (pkg, dep) in packages {
                let dep = if let TomlDependency::Detailed(dep) = dep {
                    let mut dep = dep.clone();
                    normalize_path_dependency(gctx, &mut dep, workspace_root, features)
                        .with_context(|| {
                            format!("resolving path for patch of ({pkg}) for source ({name})")
                        })?;
                    TomlDependency::Detailed(dep)
                } else {
                    dep.clone()
                };
                normalized_packages.insert(pkg.clone(), dep);
            }
            normalized_patch.insert(name.clone(), normalized_packages);
        }
        Ok(Some(normalized_patch))
    } else {
        Ok(None)
    }
}

#[tracing::instrument(skip_all)]
fn normalize_package_toml<'a>(
    original_package: &manifest::TomlPackage,
    manifest_file: &Path,
    is_embedded: bool,
    gctx: &GlobalContext,
    inherit: &dyn Fn() -> CargoResult<&'a InheritableFields>,
    features: &Features,
) -> CargoResult<Box<manifest::TomlPackage>> {
    let package_root = manifest_file.parent().unwrap();

    let edition = original_package
        .edition
        .clone()
        .map(|value| field_inherit_with(value, "edition", || inherit()?.edition()))
        .transpose()?
        .map(manifest::InheritableField::Value)
        .or_else(|| {
            if is_embedded {
                const DEFAULT_EDITION: crate::core::features::Edition =
                    crate::core::features::Edition::LATEST_STABLE;
                let _ = gctx.shell().warn(format_args!(
                    "`package.edition` is unspecified, defaulting to `{}`",
                    DEFAULT_EDITION
                ));
                Some(manifest::InheritableField::Value(
                    DEFAULT_EDITION.to_string(),
                ))
            } else {
                None
            }
        });
    let rust_version = original_package
        .rust_version
        .clone()
        .map(|value| field_inherit_with(value, "rust-version", || inherit()?.rust_version()))
        .transpose()?
        .map(manifest::InheritableField::Value);
    let name = Some(
        original_package
            .name
            .clone()
            .or_else(|| {
                if is_embedded {
                    let file_stem = manifest_file
                        .file_stem()
                        .expect("file name enforced previously")
                        .to_string_lossy();
                    let name = embedded::sanitize_name(file_stem.as_ref());
                    let name =
                        manifest::PackageName::new(name).expect("sanitize made the name valid");
                    Some(name)
                } else {
                    None
                }
            })
            .ok_or_else(|| anyhow::format_err!("missing field `package.name`"))?,
    );
    let version = original_package
        .version
        .clone()
        .map(|value| field_inherit_with(value, "version", || inherit()?.version()))
        .transpose()?
        .map(manifest::InheritableField::Value);
    let authors = original_package
        .authors
        .clone()
        .map(|value| field_inherit_with(value, "authors", || inherit()?.authors()))
        .transpose()?
        .map(manifest::InheritableField::Value);
    let build = if is_embedded {
        Some(TomlPackageBuild::Auto(false))
    } else {
        if let Some(TomlPackageBuild::MultipleScript(_)) = original_package.build {
            features.require(Feature::multiple_build_scripts())?;
        }
        targets::normalize_build(original_package.build.as_ref(), package_root)?
    };
    let metabuild = original_package.metabuild.clone();
    let default_target = original_package.default_target.clone();
    let forced_target = original_package.forced_target.clone();
    let links = original_package.links.clone();
    let exclude = original_package
        .exclude
        .clone()
        .map(|value| field_inherit_with(value, "exclude", || inherit()?.exclude()))
        .transpose()?
        .map(manifest::InheritableField::Value);
    let include = original_package
        .include
        .clone()
        .map(|value| field_inherit_with(value, "include", || inherit()?.include()))
        .transpose()?
        .map(manifest::InheritableField::Value);
    let publish = original_package
        .publish
        .clone()
        .map(|value| field_inherit_with(value, "publish", || inherit()?.publish()))
        .transpose()?
        .map(manifest::InheritableField::Value);
    let workspace = original_package.workspace.clone();
    let im_a_teapot = original_package.im_a_teapot.clone();
    let autolib = Some(false);
    let autobins = Some(false);
    let autoexamples = Some(false);
    let autotests = Some(false);
    let autobenches = Some(false);
    let default_run = original_package.default_run.clone();
    let description = original_package
        .description
        .clone()
        .map(|value| field_inherit_with(value, "description", || inherit()?.description()))
        .transpose()?
        .map(manifest::InheritableField::Value);
    let homepage = original_package
        .homepage
        .clone()
        .map(|value| field_inherit_with(value, "homepage", || inherit()?.homepage()))
        .transpose()?
        .map(manifest::InheritableField::Value);
    let documentation = original_package
        .documentation
        .clone()
        .map(|value| field_inherit_with(value, "documentation", || inherit()?.documentation()))
        .transpose()?
        .map(manifest::InheritableField::Value);
    let readme = normalize_package_readme(
        package_root,
        original_package
            .readme
            .clone()
            .map(|value| field_inherit_with(value, "readme", || inherit()?.readme(package_root)))
            .transpose()?
            .as_ref(),
    )
    .map(|s| manifest::InheritableField::Value(StringOrBool::String(s)))
    .or(Some(manifest::InheritableField::Value(StringOrBool::Bool(
        false,
    ))));
    let keywords = original_package
        .keywords
        .clone()
        .map(|value| field_inherit_with(value, "keywords", || inherit()?.keywords()))
        .transpose()?
        .map(manifest::InheritableField::Value);
    let categories = original_package
        .categories
        .clone()
        .map(|value| field_inherit_with(value, "categories", || inherit()?.categories()))
        .transpose()?
        .map(manifest::InheritableField::Value);
    let license = original_package
        .license
        .clone()
        .map(|value| field_inherit_with(value, "license", || inherit()?.license()))
        .transpose()?
        .map(manifest::InheritableField::Value);
    let license_file = original_package
        .license_file
        .clone()
        .map(|value| {
            field_inherit_with(value, "license-file", || {
                inherit()?.license_file(package_root)
            })
        })
        .transpose()?
        .map(manifest::InheritableField::Value);
    let repository = original_package
        .repository
        .clone()
        .map(|value| field_inherit_with(value, "repository", || inherit()?.repository()))
        .transpose()?
        .map(manifest::InheritableField::Value);
    let resolver = original_package.resolver.clone();
    let metadata = original_package.metadata.clone();

    let normalized_package = manifest::TomlPackage {
        edition,
        rust_version,
        name,
        version,
        authors,
        build,
        metabuild,
        default_target,
        forced_target,
        links,
        exclude,
        include,
        publish,
        workspace,
        im_a_teapot,
        autolib,
        autobins,
        autoexamples,
        autotests,
        autobenches,
        default_run,
        description,
        homepage,
        documentation,
        readme,
        keywords,
        categories,
        license,
        license_file,
        repository,
        resolver,
        metadata,
        _invalid_cargo_features: Default::default(),
    };

    Ok(Box::new(normalized_package))
}

/// Returns the name of the README file for a [`manifest::TomlPackage`].
fn normalize_package_readme(
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

#[tracing::instrument(skip_all)]
fn normalize_features(
    original_features: Option<&BTreeMap<manifest::FeatureName, Vec<String>>>,
) -> CargoResult<Option<BTreeMap<manifest::FeatureName, Vec<String>>>> {
    let Some(normalized_features) = original_features.cloned() else {
        return Ok(None);
    };

    Ok(Some(normalized_features))
}

#[tracing::instrument(skip_all)]
fn normalize_dependencies<'a>(
    gctx: &GlobalContext,
    edition: Edition,
    features: &Features,
    orig_deps: Option<&BTreeMap<manifest::PackageName, manifest::InheritableDependency>>,
    kind: DepKind,
    inherit: &dyn Fn() -> CargoResult<&'a InheritableFields>,
    workspace_root: &dyn Fn() -> CargoResult<&'a Path>,
    package_root: &Path,
    warnings: &mut Vec<String>,
) -> CargoResult<Option<BTreeMap<manifest::PackageName, manifest::InheritableDependency>>> {
    let Some(dependencies) = orig_deps else {
        return Ok(None);
    };

    let mut deps = BTreeMap::new();
    for (name_in_toml, v) in dependencies.iter() {
        let mut resolved = dependency_inherit_with(
            v.clone(),
            name_in_toml,
            inherit,
            package_root,
            edition,
            warnings,
        )?;
        if let manifest::TomlDependency::Detailed(ref mut d) = resolved {
            deprecated_underscore(
                &d.default_features2,
                &d.default_features,
                "default-features",
                name_in_toml,
                "dependency",
                edition,
                warnings,
            )?;
            if d.public.is_some() {
                let with_public_feature = features.require(Feature::public_dependency()).is_ok();
                let with_z_public = gctx.cli_unstable().public_dependency;
                match kind {
                    DepKind::Normal => {
                        if !with_public_feature && !with_z_public {
                            d.public = None;
                            warnings.push(format!(
                                "ignoring `public` on dependency {name_in_toml}, pass `-Zpublic-dependency` to enable support for it"
                            ));
                        }
                    }
                    DepKind::Development | DepKind::Build => {
                        let kind_name = kind.kind_table();
                        let hint = format!(
                            "'public' specifier can only be used on regular dependencies, not {kind_name}",
                        );
                        if with_public_feature || with_z_public {
                            bail!(hint)
                        } else {
                            // If public feature isn't enabled in nightly, we instead warn that.
                            warnings.push(hint);
                            d.public = None;
                        }
                    }
                }
            }
            normalize_path_dependency(gctx, d, workspace_root, features)
                .with_context(|| format!("resolving path dependency {name_in_toml}"))?;
        }

        deps.insert(
            name_in_toml.clone(),
            manifest::InheritableDependency::Value(resolved.clone()),
        );
    }
    Ok(Some(deps))
}

fn normalize_path_dependency<'a>(
    gctx: &GlobalContext,
    detailed_dep: &mut TomlDetailedDependency,
    workspace_root: &dyn Fn() -> CargoResult<&'a Path>,
    features: &Features,
) -> CargoResult<()> {
    if let Some(base) = detailed_dep.base.take() {
        if let Some(path) = detailed_dep.path.as_mut() {
            let new_path = lookup_path_base(&base, gctx, workspace_root, features)?.join(&path);
            *path = new_path.to_str().unwrap().to_string();
        } else {
            bail!("`base` can only be used with path dependencies");
        }
    }
    Ok(())
}

fn load_inheritable_fields(
    gctx: &GlobalContext,
    normalized_path: &Path,
    workspace_config: &WorkspaceConfig,
) -> CargoResult<InheritableFields> {
    match workspace_config {
        WorkspaceConfig::Root(root) => Ok(root.inheritable().clone()),
        WorkspaceConfig::Member {
            root: Some(path_to_root),
        } => {
            let path = normalized_path
                .parent()
                .unwrap()
                .join(path_to_root)
                .join("Cargo.toml");
            let root_path = paths::normalize_path(&path);
            inheritable_from_path(gctx, root_path)
        }
        WorkspaceConfig::Member { root: None } => {
            match find_workspace_root(&normalized_path, gctx)? {
                Some(path_to_root) => inheritable_from_path(gctx, path_to_root),
                None => Err(anyhow!("failed to find a workspace root")),
            }
        }
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
    if let Some(ws_root) = gctx.ws_roots().get(workspace_path_root) {
        return Ok(ws_root.inheritable().clone());
    };

    let source_id = SourceId::for_manifest_path(&workspace_path)?;
    let man = read_manifest(&workspace_path, source_id, gctx)?;
    match man.workspace_config() {
        WorkspaceConfig::Root(root) => {
            gctx.ws_roots().insert(workspace_path, root.clone());
            Ok(root.inheritable().clone())
        }
        _ => bail!(
            "root of a workspace inferred but wasn't a root: {}",
            workspace_path.display()
        ),
    }
}

/// Defines simple getter methods for inheritable fields.
macro_rules! package_field_getter {
    ( $(($key:literal, $field:ident -> $ret:ty),)* ) => (
        $(
            #[doc = concat!("Gets the field `workspace.package.", $key, "`.")]
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
            if detailed.base.is_none() {
                // If this is a path dependency without a base, then update the path to be relative
                // to the workspace root instead.
                if let Some(rel_path) = &detailed.path {
                    detailed.path = Some(resolve_relative_path(
                        name,
                        self.ws_root(),
                        package_root,
                        rel_path,
                    )?);
                }
            }
        }
        Ok(dep)
    }

    /// Gets the field `workspace.lints`.
    pub fn lints(&self) -> CargoResult<manifest::TomlLints> {
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
        let Some(readme) = normalize_package_readme(
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
            anyhow::bail!(
                "cannot override `workspace.lints` in `lints`, either remove the overrides or `lints.workspace = true` and manually specify the lints"
            );
        }
        get_ws_inheritable().with_context(
            || "error inheriting `lints` from workspace root manifest's `workspace.lints`",
        )
    } else {
        Ok(lints.lints)
    }
}

fn dependency_inherit_with<'a>(
    dependency: manifest::InheritableDependency,
    name: &str,
    inherit: &dyn Fn() -> CargoResult<&'a InheritableFields>,
    package_root: &Path,
    edition: Edition,
    warnings: &mut Vec<String>,
) -> CargoResult<manifest::TomlDependency> {
    match dependency {
        manifest::InheritableDependency::Value(value) => Ok(value),
        manifest::InheritableDependency::Inherit(w) => {
            inner_dependency_inherit_with(w, name, inherit, package_root, edition, warnings).with_context(|| {
                format!(
                    "error inheriting `{name}` from workspace root manifest's `workspace.dependencies.{name}`",
                )
            })
        }
    }
}

fn inner_dependency_inherit_with<'a>(
    pkg_dep: manifest::TomlInheritedDependency,
    name: &str,
    inherit: &dyn Fn() -> CargoResult<&'a InheritableFields>,
    package_root: &Path,
    edition: Edition,
    warnings: &mut Vec<String>,
) -> CargoResult<manifest::TomlDependency> {
    let ws_dep = inherit()?.get_dependency(name, package_root)?;
    let mut merged_dep = match ws_dep {
        manifest::TomlDependency::Simple(ws_version) => manifest::TomlDetailedDependency {
            version: Some(ws_version),
            ..Default::default()
        },
        manifest::TomlDependency::Detailed(ws_dep) => ws_dep.clone(),
    };
    let manifest::TomlInheritedDependency {
        workspace: _,

        features,
        optional,
        default_features,
        default_features2,
        public,

        _unused_keys: _,
    } = &pkg_dep;
    let default_features = default_features.or(*default_features2);

    match (default_features, merged_dep.default_features()) {
        // member: default-features = true and
        // workspace: default-features = false should turn on
        // default-features
        (Some(true), Some(false)) => {
            merged_dep.default_features = Some(true);
        }
        // member: default-features = false and
        // workspace: default-features = true should ignore member
        // default-features
        (Some(false), Some(true)) => {
            deprecated_ws_default_features(name, Some(true), edition, warnings)?;
        }
        // member: default-features = false and
        // workspace: dep = "1.0" should ignore member default-features
        (Some(false), None) => {
            deprecated_ws_default_features(name, None, edition, warnings)?;
        }
        _ => {}
    }
    merged_dep.features = match (merged_dep.features.clone(), features.clone()) {
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
    merged_dep.optional = *optional;
    merged_dep.public = *public;
    Ok(manifest::TomlDependency::Detailed(merged_dep))
}

fn deprecated_ws_default_features(
    label: &str,
    ws_def_feat: Option<bool>,
    edition: Edition,
    warnings: &mut Vec<String>,
) -> CargoResult<()> {
    let ws_def_feat = match ws_def_feat {
        Some(true) => "true",
        Some(false) => "false",
        None => "not specified",
    };
    if Edition::Edition2024 <= edition {
        anyhow::bail!("`default-features = false` cannot override workspace's `default-features`");
    } else {
        warnings.push(format!(
            "`default-features` is ignored for {label}, since `default-features` was \
                {ws_def_feat} for `workspace.dependencies.{label}`, \
                this could become a hard error in the future"
        ));
    }
    Ok(())
}

#[tracing::instrument(skip_all)]
pub fn to_real_manifest(
    contents: String,
    document: toml::Spanned<toml::de::DeTable<'static>>,
    original_toml: manifest::TomlManifest,
    normalized_toml: manifest::TomlManifest,
    features: Features,
    workspace_config: WorkspaceConfig,
    source_id: SourceId,
    manifest_file: &Path,
    is_embedded: bool,
    gctx: &GlobalContext,
    warnings: &mut Vec<String>,
    _errors: &mut Vec<String>,
) -> CargoResult<Manifest> {
    let package_root = manifest_file.parent().unwrap();
    if !package_root.is_dir() {
        bail!(
            "package root '{}' is not a directory",
            package_root.display()
        );
    };

    let normalized_package = normalized_toml
        .package()
        .expect("previously verified to have a `[package]`");
    let package_name = normalized_package
        .normalized_name()
        .expect("previously normalized");
    if package_name.contains(':') {
        features.require(Feature::open_namespaces())?;
    }
    let rust_version = normalized_package
        .normalized_rust_version()
        .expect("previously normalized")
        .cloned();

    let edition = if let Some(edition) = normalized_package
        .normalized_edition()
        .expect("previously normalized")
    {
        let edition: Edition = edition
            .parse()
            .context("failed to parse the `edition` key")?;
        if let Some(pkg_msrv) = &rust_version {
            if let Some(edition_msrv) = edition.first_version() {
                let edition_msrv = RustVersion::try_from(edition_msrv).unwrap();
                if !edition_msrv.is_compatible_with(pkg_msrv.as_partial()) {
                    bail!(
                        "rust-version {} is imcompatible with the version ({}) required by \
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
        if msrv_edition != default_edition || rust_version.is_none() {
            let tip = if msrv_edition == latest_edition || rust_version.is_none() {
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
    if !edition.is_stable() {
        features.require(Feature::unstable_editions())?;
    }

    if original_toml.project.is_some() {
        if Edition::Edition2024 <= edition {
            anyhow::bail!(
                "`[project]` is not supported as of the 2024 Edition, please use `[package]`"
            );
        } else {
            warnings.push(format!("`[project]` is deprecated in favor of `[package]`"));
        }
    }

    if normalized_package.metabuild.is_some() {
        features.require(Feature::metabuild())?;
    }

    if is_embedded {
        let manifest::TomlManifest {
            cargo_features: _,
            package: _,
            project: _,
            badges: _,
            features: _,
            lib,
            bin,
            example,
            test,
            bench,
            dependencies: _,
            dev_dependencies: _,
            dev_dependencies2: _,
            build_dependencies,
            build_dependencies2,
            target: _,
            lints: _,
            hints: _,
            workspace,
            profile: _,
            patch: _,
            replace: _,
            _unused_keys: _,
        } = &original_toml;
        let mut invalid_fields = vec![
            ("`workspace`", workspace.is_some()),
            ("`lib`", lib.is_some()),
            ("`bin`", bin.is_some()),
            ("`example`", example.is_some()),
            ("`test`", test.is_some()),
            ("`bench`", bench.is_some()),
            ("`build-dependencies`", build_dependencies.is_some()),
            ("`build_dependencies`", build_dependencies2.is_some()),
        ];
        if let Some(package) = original_toml.package() {
            let manifest::TomlPackage {
                edition: _,
                rust_version: _,
                name: _,
                version: _,
                authors: _,
                build,
                metabuild,
                default_target: _,
                forced_target: _,
                links,
                exclude: _,
                include: _,
                publish: _,
                workspace,
                im_a_teapot: _,
                autolib,
                autobins,
                autoexamples,
                autotests,
                autobenches,
                default_run,
                description: _,
                homepage: _,
                documentation: _,
                readme: _,
                keywords: _,
                categories: _,
                license: _,
                license_file: _,
                repository: _,
                resolver: _,
                metadata: _,
                _invalid_cargo_features: _,
            } = package.as_ref();
            invalid_fields.extend([
                ("`package.workspace`", workspace.is_some()),
                ("`package.build`", build.is_some()),
                ("`package.metabuild`", metabuild.is_some()),
                ("`package.links`", links.is_some()),
                ("`package.autolib`", autolib.is_some()),
                ("`package.autobins`", autobins.is_some()),
                ("`package.autoexamples`", autoexamples.is_some()),
                ("`package.autotests`", autotests.is_some()),
                ("`package.autobenches`", autobenches.is_some()),
                ("`package.default-run`", default_run.is_some()),
            ]);
        }
        let invalid_fields = invalid_fields
            .into_iter()
            .filter_map(|(name, invalid)| invalid.then_some(name))
            .collect::<Vec<_>>();
        if !invalid_fields.is_empty() {
            let fields = invalid_fields.join(", ");
            let are = if invalid_fields.len() == 1 {
                "is"
            } else {
                "are"
            };
            anyhow::bail!("{fields} {are} not allowed in embedded manifests")
        }
    }

    let resolve_behavior = match (
        normalized_package.resolver.as_ref(),
        normalized_toml
            .workspace
            .as_ref()
            .and_then(|ws| ws.resolver.as_ref()),
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
    let targets = to_targets(
        &features,
        &original_toml,
        &normalized_toml,
        package_root,
        edition,
        &normalized_package.metabuild,
        warnings,
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
                    target_path.display(),
                    conflicts
                        .iter()
                        .map(|t| format!("  * `{}` target `{}`", t.kind().description(), t.name(),))
                        .join("\n")
                ));
            })
    }

    if let Some(links) = &normalized_package.links {
        if !targets.iter().any(|t| t.is_custom_build()) {
            bail!(
                "package specifies that it links to `{links}` but does not have a custom build script"
            )
        }
    }

    validate_dependencies(original_toml.dependencies.as_ref(), None, None, warnings)?;
    validate_dependencies(
        original_toml.dev_dependencies(),
        None,
        Some(DepKind::Development),
        warnings,
    )?;
    validate_dependencies(
        original_toml.build_dependencies(),
        None,
        Some(DepKind::Build),
        warnings,
    )?;
    for (name, platform) in original_toml.target.iter().flatten() {
        let platform_kind: Platform = name.parse()?;
        platform_kind.check_cfg_attributes(warnings);
        platform_kind.check_cfg_keywords(warnings, manifest_file);
        let platform_kind = Some(platform_kind);
        validate_dependencies(
            platform.dependencies.as_ref(),
            platform_kind.as_ref(),
            None,
            warnings,
        )?;
        validate_dependencies(
            platform.build_dependencies(),
            platform_kind.as_ref(),
            Some(DepKind::Build),
            warnings,
        )?;
        validate_dependencies(
            platform.dev_dependencies(),
            platform_kind.as_ref(),
            Some(DepKind::Development),
            warnings,
        )?;
    }

    // Collect the dependencies.
    let mut deps = Vec::new();
    let mut manifest_ctx = ManifestContext {
        deps: &mut deps,
        source_id,
        gctx,
        warnings,
        platform: None,
        root: package_root,
    };
    gather_dependencies(
        &mut manifest_ctx,
        normalized_toml.dependencies.as_ref(),
        None,
    )?;
    gather_dependencies(
        &mut manifest_ctx,
        normalized_toml.dev_dependencies(),
        Some(DepKind::Development),
    )?;
    gather_dependencies(
        &mut manifest_ctx,
        normalized_toml.build_dependencies(),
        Some(DepKind::Build),
    )?;
    for (name, platform) in normalized_toml.target.iter().flatten() {
        manifest_ctx.platform = Some(name.parse()?);
        gather_dependencies(&mut manifest_ctx, platform.dependencies.as_ref(), None)?;
        gather_dependencies(
            &mut manifest_ctx,
            platform.build_dependencies(),
            Some(DepKind::Build),
        )?;
        gather_dependencies(
            &mut manifest_ctx,
            platform.dev_dependencies(),
            Some(DepKind::Development),
        )?;
    }
    let replace = replace(&normalized_toml, &mut manifest_ctx)?;
    let patch = patch(&normalized_toml, &mut manifest_ctx)?;

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

    verify_lints(
        normalized_toml
            .normalized_lints()
            .expect("previously normalized"),
        gctx,
        warnings,
    )?;
    let default = manifest::TomlLints::default();
    let rustflags = lints_to_rustflags(
        normalized_toml
            .normalized_lints()
            .expect("previously normalized")
            .unwrap_or(&default),
    )?;

    let hints = normalized_toml.hints.clone();

    let metadata = ManifestMetadata {
        description: normalized_package
            .normalized_description()
            .expect("previously normalized")
            .cloned(),
        homepage: normalized_package
            .normalized_homepage()
            .expect("previously normalized")
            .cloned(),
        documentation: normalized_package
            .normalized_documentation()
            .expect("previously normalized")
            .cloned(),
        readme: normalized_package
            .normalized_readme()
            .expect("previously normalized")
            .cloned(),
        authors: normalized_package
            .normalized_authors()
            .expect("previously normalized")
            .cloned()
            .unwrap_or_default(),
        license: normalized_package
            .normalized_license()
            .expect("previously normalized")
            .cloned(),
        license_file: normalized_package
            .normalized_license_file()
            .expect("previously normalized")
            .cloned(),
        repository: normalized_package
            .normalized_repository()
            .expect("previously normalized")
            .cloned(),
        keywords: normalized_package
            .normalized_keywords()
            .expect("previously normalized")
            .cloned()
            .unwrap_or_default(),
        categories: normalized_package
            .normalized_categories()
            .expect("previously normalized")
            .cloned()
            .unwrap_or_default(),
        badges: normalized_toml.badges.clone().unwrap_or_default(),
        links: normalized_package.links.clone(),
        rust_version: rust_version.clone(),
    };

    if let Some(profiles) = &normalized_toml.profile {
        let cli_unstable = gctx.cli_unstable();
        validate_profiles(profiles, cli_unstable, &features, warnings)?;
    }

    let version = normalized_package
        .normalized_version()
        .expect("previously normalized");
    let publish = match normalized_package
        .normalized_publish()
        .expect("previously normalized")
    {
        Some(manifest::VecStringOrBool::VecString(vecstring)) => Some(vecstring.clone()),
        Some(manifest::VecStringOrBool::Bool(false)) => Some(vec![]),
        Some(manifest::VecStringOrBool::Bool(true)) => None,
        None => version.is_none().then_some(vec![]),
    };

    if version.is_none() && publish != Some(vec![]) {
        bail!("`package.publish` requires `package.version` be specified");
    }

    let pkgid = PackageId::new(
        package_name.as_str().into(),
        version
            .cloned()
            .unwrap_or_else(|| semver::Version::new(0, 0, 0)),
        source_id,
    );
    let summary = {
        let summary = Summary::new(
            pkgid,
            deps,
            &normalized_toml
                .features
                .as_ref()
                .unwrap_or(&Default::default())
                .iter()
                .map(|(k, v)| {
                    (
                        k.to_string().into(),
                        v.iter().map(InternedString::from).collect(),
                    )
                })
                .collect(),
            normalized_package.links.as_deref(),
            rust_version.clone(),
        );
        // edition2024 stops exposing implicit features, which will strip weak optional dependencies from `dependencies`,
        // need to check whether `dep_name` is stripped as unused dependency
        if let Err(ref err) = summary {
            if let Some(missing_dep) = err.downcast_ref::<MissingDependencyError>() {
                missing_dep_diagnostic(
                    missing_dep,
                    &original_toml,
                    &document,
                    &contents,
                    manifest_file,
                    gctx,
                )?;
            }
        }
        summary?
    };

    if summary.features().contains_key("default-features") {
        warnings.push(
            "`[features]` defines a feature named `default-features`
note: only a feature named `default` will be enabled by default"
                .to_string(),
        )
    }

    if let Some(run) = &normalized_package.default_run {
        if !targets
            .iter()
            .filter(|t| t.is_bin())
            .any(|t| t.name() == run)
        {
            let suggestion = util::closest_msg(
                run,
                targets.iter().filter(|t| t.is_bin()),
                |t| t.name(),
                "target",
            );
            bail!("default-run target `{}` not found{}", run, suggestion);
        }
    }

    let default_kind = normalized_package
        .default_target
        .as_ref()
        .map(|t| CompileTarget::new(&*t))
        .transpose()?
        .map(CompileKind::Target);
    let forced_kind = normalized_package
        .forced_target
        .as_ref()
        .map(|t| CompileTarget::new(&*t))
        .transpose()?
        .map(CompileKind::Target);
    let include = normalized_package
        .normalized_include()
        .expect("previously normalized")
        .cloned()
        .unwrap_or_default();
    let exclude = normalized_package
        .normalized_exclude()
        .expect("previously normalized")
        .cloned()
        .unwrap_or_default();
    let links = normalized_package.links.clone();
    let custom_metadata = normalized_package.metadata.clone();
    let im_a_teapot = normalized_package.im_a_teapot;
    let default_run = normalized_package.default_run.clone();
    let metabuild = normalized_package.metabuild.clone().map(|sov| sov.0);
    let manifest = Manifest::new(
        Rc::new(contents),
        Rc::new(document),
        Rc::new(original_toml),
        Rc::new(normalized_toml),
        summary,
        default_kind,
        forced_kind,
        targets,
        exclude,
        include,
        links,
        metadata,
        custom_metadata,
        publish,
        replace,
        patch,
        workspace_config,
        features,
        edition,
        rust_version,
        im_a_teapot,
        default_run,
        metabuild,
        resolve_behavior,
        rustflags,
        hints,
        is_embedded,
    );
    if manifest
        .normalized_toml()
        .package()
        .unwrap()
        .license_file
        .is_some()
        && manifest
            .normalized_toml()
            .package()
            .unwrap()
            .license
            .is_some()
    {
        warnings.push(
            "only one of `license` or `license-file` is necessary\n\
                 `license` should be used if the package license can be expressed \
                 with a standard SPDX expression.\n\
                 `license-file` should be used if the package uses a non-standard license.\n\
                 See https://doc.rust-lang.org/cargo/reference/manifest.html#the-license-and-license-file-fields \
                 for more information."
                .to_owned(),
        );
    }
    warn_on_unused(&manifest.original_toml()._unused_keys, warnings);

    manifest.feature_gate()?;

    Ok(manifest)
}

fn missing_dep_diagnostic(
    missing_dep: &MissingDependencyError,
    orig_toml: &TomlManifest,
    document: &toml::Spanned<toml::de::DeTable<'static>>,
    contents: &str,
    manifest_file: &Path,
    gctx: &GlobalContext,
) -> CargoResult<()> {
    let dep_name = missing_dep.dep_name;
    let manifest_path = rel_cwd_manifest_path(manifest_file, gctx);
    let feature_span =
        get_key_value_span(&document, &["features", missing_dep.feature.as_str()]).unwrap();

    let title = format!(
        "feature `{}` includes `{}`, but `{}` is not a dependency",
        missing_dep.feature, missing_dep.feature_value, &dep_name
    );
    let help = format!("enable the dependency with `dep:{dep_name}`");
    let info_label = format!(
        "`{}` is an unused optional dependency since no feature enables it",
        &dep_name
    );
    let group = Group::with_title(Level::ERROR.primary_title(&title));
    let snippet = Snippet::source(contents)
        .path(manifest_path)
        .annotation(AnnotationKind::Primary.span(feature_span.value));
    let group = if missing_dep.weak_optional {
        let mut orig_deps = vec![
            (
                orig_toml.dependencies.as_ref(),
                vec![DepKind::Normal.kind_table()],
            ),
            (
                orig_toml.build_dependencies.as_ref(),
                vec![DepKind::Build.kind_table()],
            ),
        ];
        for (name, platform) in orig_toml.target.iter().flatten() {
            orig_deps.push((
                platform.dependencies.as_ref(),
                vec!["target", name, DepKind::Normal.kind_table()],
            ));
            orig_deps.push((
                platform.build_dependencies.as_ref(),
                vec!["target", name, DepKind::Normal.kind_table()],
            ));
        }

        if let Some((_, toml_path)) = orig_deps.iter().find(|(deps, _)| {
            if let Some(deps) = deps {
                deps.keys().any(|p| *p.as_str() == *dep_name)
            } else {
                false
            }
        }) {
            let toml_path = toml_path
                .iter()
                .map(|s| *s)
                .chain(std::iter::once(dep_name.as_str()))
                .collect::<Vec<_>>();
            let dep_span = get_key_value_span(&document, &toml_path).unwrap();

            group
                .element(
                    snippet
                        .annotation(AnnotationKind::Context.span(dep_span.key).label(info_label)),
                )
                .element(Level::HELP.message(help))
        } else {
            group.element(snippet)
        }
    } else {
        group.element(snippet)
    };

    if let Err(err) = gctx.shell().print_report(&[group], true) {
        return Err(err.into());
    }
    Err(AlreadyPrintedError::new(anyhow!("").into()).into())
}

fn to_virtual_manifest(
    contents: String,
    document: toml::Spanned<toml::de::DeTable<'static>>,
    original_toml: manifest::TomlManifest,
    normalized_toml: manifest::TomlManifest,
    features: Features,
    workspace_config: WorkspaceConfig,
    source_id: SourceId,
    manifest_file: &Path,
    gctx: &GlobalContext,
    warnings: &mut Vec<String>,
    _errors: &mut Vec<String>,
) -> CargoResult<VirtualManifest> {
    let root = manifest_file.parent().unwrap();

    let mut deps = Vec::new();
    let (replace, patch) = {
        let mut manifest_ctx = ManifestContext {
            deps: &mut deps,
            source_id,
            gctx,
            warnings,
            platform: None,
            root,
        };
        (
            replace(&normalized_toml, &mut manifest_ctx)?,
            patch(&normalized_toml, &mut manifest_ctx)?,
        )
    };
    if let Some(profiles) = &normalized_toml.profile {
        validate_profiles(profiles, gctx.cli_unstable(), &features, warnings)?;
    }
    let resolve_behavior = normalized_toml
        .workspace
        .as_ref()
        .and_then(|ws| ws.resolver.as_deref())
        .map(|r| ResolveBehavior::from_manifest(r))
        .transpose()?;
    if let WorkspaceConfig::Member { .. } = &workspace_config {
        bail!("virtual manifests must be configured with [workspace]");
    }
    let manifest = VirtualManifest::new(
        Rc::new(contents),
        Rc::new(document),
        Rc::new(original_toml),
        Rc::new(normalized_toml),
        replace,
        patch,
        workspace_config,
        features,
        resolve_behavior,
    );

    warn_on_unused(&manifest.original_toml()._unused_keys, warnings);

    Ok(manifest)
}

#[tracing::instrument(skip_all)]
fn validate_dependencies(
    original_deps: Option<&BTreeMap<manifest::PackageName, manifest::InheritableDependency>>,
    platform: Option<&Platform>,
    kind: Option<DepKind>,
    warnings: &mut Vec<String>,
) -> CargoResult<()> {
    let Some(dependencies) = original_deps else {
        return Ok(());
    };

    for (name_in_toml, v) in dependencies.iter() {
        let kind_name = match kind {
            Some(k) => k.kind_table(),
            None => "dependencies",
        };
        let table_in_toml = if let Some(platform) = platform {
            format!("target.{platform}.{kind_name}")
        } else {
            kind_name.to_string()
        };
        unused_dep_keys(name_in_toml, &table_in_toml, v.unused_keys(), warnings);
    }
    Ok(())
}

struct ManifestContext<'a, 'b> {
    deps: &'a mut Vec<Dependency>,
    source_id: SourceId,
    gctx: &'b GlobalContext,
    warnings: &'a mut Vec<String>,
    platform: Option<Platform>,
    root: &'a Path,
}

#[tracing::instrument(skip_all)]
fn gather_dependencies(
    manifest_ctx: &mut ManifestContext<'_, '_>,
    normalized_deps: Option<&BTreeMap<manifest::PackageName, manifest::InheritableDependency>>,
    kind: Option<DepKind>,
) -> CargoResult<()> {
    let Some(dependencies) = normalized_deps else {
        return Ok(());
    };

    for (n, v) in dependencies.iter() {
        let resolved = v.normalized().expect("previously normalized");
        let dep = dep_to_dependency(&resolved, n, manifest_ctx, kind)?;
        manifest_ctx.deps.push(dep);
    }
    Ok(())
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
                        "[patch] entry `{}` should be a URL or registry name{}",
                        toml_url,
                        if toml_url == "crates" {
                            "\nFor crates.io, use [patch.crates-io] (with a dash)"
                        } else {
                            ""
                        }
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

pub(crate) fn to_dependency<P: ResolveToPath + Clone>(
    dep: &manifest::TomlDependency<P>,
    name: &str,
    source_id: SourceId,
    gctx: &GlobalContext,
    warnings: &mut Vec<String>,
    platform: Option<Platform>,
    root: &Path,
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
        anyhow::bail!(
            "dependency ({name_in_toml}) specified without \
                 providing a local path, Git repository, version, or \
                 workspace dependency to use"
        );
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

    let new_source_id = to_dependency_source_id(orig, name_in_toml, manifest_ctx)?;

    let (pkg_name, explicit_name_in_toml) = match orig.package {
        Some(ref s) => (&s[..], Some(name_in_toml)),
        None => (name_in_toml, None),
    };

    let version = orig.version.as_deref();
    let mut dep = Dependency::parse(pkg_name, version, new_source_id)?;
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
        dep.set_public(p);
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

fn to_dependency_source_id<P: ResolveToPath + Clone>(
    orig: &manifest::TomlDetailedDependency<P>,
    name_in_toml: &str,
    manifest_ctx: &mut ManifestContext<'_, '_>,
) -> CargoResult<SourceId> {
    match (
        orig.git.as_ref(),
        orig.path.as_ref(),
        orig.registry.as_deref(),
        orig.registry_index.as_ref(),
    ) {
        (Some(_git), _, Some(_registry), _) | (Some(_git), _, _, Some(_registry)) => bail!(
            "dependency ({name_in_toml}) specification is ambiguous. \
                 Only one of `git` or `registry` is allowed.",
        ),
        (_, _, Some(_registry), Some(_registry_index)) => bail!(
            "dependency ({name_in_toml}) specification is ambiguous. \
                 Only one of `registry` or `registry-index` is allowed.",
        ),
        (Some(_git), Some(_path), None, None) => {
            bail!(
                "dependency ({name_in_toml}) specification is ambiguous. \
                     Only one of `git` or `path` is allowed.",
            );
        }
        (Some(git), None, None, None) => {
            let n_details = [&orig.branch, &orig.tag, &orig.rev]
                .iter()
                .filter(|d| d.is_some())
                .count();

            if n_details > 1 {
                bail!(
                    "dependency ({name_in_toml}) specification is ambiguous. \
                         Only one of `branch`, `tag` or `rev` is allowed.",
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
                    "URL fragment `#{fragment}` in git URL is ignored for dependency ({name_in_toml}). \
                        If you were trying to specify a specific git revision, \
                        use `rev = \"{fragment}\"` in the dependency declaration.",
                );
                manifest_ctx.warnings.push(msg);
            }

            SourceId::for_git(&loc, reference)
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
                SourceId::for_path(&path)
            } else {
                Ok(manifest_ctx.source_id)
            }
        }
        (None, None, Some(registry), None) => SourceId::alt_registry(manifest_ctx.gctx, registry),
        (None, None, None, Some(registry_index)) => {
            let url = registry_index.into_url()?;
            SourceId::for_registry(&url)
        }
        (None, None, None, None) => SourceId::crates_io(manifest_ctx.gctx),
    }
}

pub(crate) fn lookup_path_base<'a>(
    base: &PathBaseName,
    gctx: &GlobalContext,
    workspace_root: &dyn Fn() -> CargoResult<&'a Path>,
    features: &Features,
) -> CargoResult<PathBuf> {
    features.require(Feature::path_bases())?;

    // HACK: The `base` string is user controlled, but building the path is safe from injection
    // attacks since the `PathBaseName` type restricts the characters that can be used to exclude `.`
    let base_key = format!("path-bases.{base}");

    // Look up the relevant base in the Config and use that as the root.
    if let Some(path_bases) = gctx.get::<Option<ConfigRelativePath>>(&base_key)? {
        Ok(path_bases.resolve_path(gctx))
    } else {
        // Otherwise, check the built-in bases.
        match base.as_str() {
            "workspace" => Ok(workspace_root()?.to_path_buf()),
            _ => bail!(
                "path base `{base}` is undefined. \
            You must add an entry for `{base}` in the Cargo configuration [path-bases] table."
            ),
        }
    }
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

/// Checks syntax validity and unstable feature gate for a given profile.
pub fn validate_profile(
    root: &manifest::TomlProfile,
    name: &str,
    cli_unstable: &CliUnstable,
    features: &Features,
    warnings: &mut Vec<String>,
) -> CargoResult<()> {
    validate_profile_layer(root, cli_unstable, features)?;
    if let Some(ref profile) = root.build_override {
        validate_profile_override(profile, "build-override")?;
        validate_profile_layer(profile, cli_unstable, features)?;
    }
    if let Some(ref packages) = root.package {
        for profile in packages.values() {
            validate_profile_override(profile, "package")?;
            validate_profile_layer(profile, cli_unstable, features)?;
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
        if panic != "unwind" && panic != "abort" && panic != "immediate-abort" {
            bail!(
                "`panic` setting of `{}` is not a valid setting, \
                     must be `unwind`, `abort`, or `immediate-abort`.",
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
    cli_unstable: &CliUnstable,
    features: &Features,
) -> CargoResult<()> {
    if profile.codegen_backend.is_some() {
        match (
            features.require(Feature::codegen_backend()),
            cli_unstable.codegen_backend,
        ) {
            (Err(e), false) => return Err(e),
            _ => {}
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
    if profile.panic.as_deref() == Some("immediate-abort") {
        match (
            features.require(Feature::panic_immediate_abort()),
            cli_unstable.panic_immediate_abort,
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

fn verify_lints(
    lints: Option<&manifest::TomlLints>,
    gctx: &GlobalContext,
    warnings: &mut Vec<String>,
) -> CargoResult<()> {
    let Some(lints) = lints else {
        return Ok(());
    };

    for (tool, lints) in lints {
        let supported = ["cargo", "clippy", "rust", "rustdoc"];
        if !supported.contains(&tool.as_str()) {
            let message = format!(
                "unrecognized lint tool `lints.{tool}`, specifying unrecognized tools may break in the future.
supported tools: {}",
                supported.join(", "),
            );
            warnings.push(message);
            continue;
        }
        if tool == "cargo" && !gctx.cli_unstable().cargo_lints {
            warn_for_cargo_lint_feature(gctx, warnings);
        }
        for (name, config) in lints {
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
            } else if let Some(config) = config.config() {
                for config_name in config.keys() {
                    // manually report unused manifest key warning since we collect all the "extra"
                    // keys and values inside the config table
                    //
                    // except for `rust.unexpected_cfgs.check-cfg` which is used by rustc/rustdoc
                    if !(tool == "rust" && name == "unexpected_cfgs" && config_name == "check-cfg")
                    {
                        let message =
                            format!("unused manifest key: `lints.{tool}.{name}.{config_name}`");
                        warnings.push(message);
                    }
                }
            }
        }
    }

    Ok(())
}

fn warn_for_cargo_lint_feature(gctx: &GlobalContext, warnings: &mut Vec<String>) {
    use std::fmt::Write as _;

    let key_name = "lints.cargo";
    let feature_name = "cargo-lints";

    let mut message = String::new();

    let _ = write!(
        message,
        "unused manifest key `{key_name}` (may be supported in a future version)"
    );
    if gctx.nightly_features_allowed {
        let _ = write!(
            message,
            "

consider passing `-Z{feature_name}` to enable this feature."
        );
    } else {
        let _ = write!(
            message,
            "

this Cargo does not support nightly features, but if you
switch to nightly channel you can pass
`-Z{feature_name}` to enable this feature.",
        );
    }
    warnings.push(message);
}

fn lints_to_rustflags(lints: &manifest::TomlLints) -> CargoResult<Vec<String>> {
    let mut rustflags = lints
        .iter()
        // We don't want to pass any of the `cargo` lints to `rustc`
        .filter(|(tool, _)| tool != &"cargo")
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

    let mut rustflags: Vec<_> = rustflags.into_iter().map(|(_, _, option)| option).collect();

    // Also include the custom arguments specified in `[lints.rust.unexpected_cfgs.check_cfg]`
    if let Some(rust_lints) = lints.get("rust") {
        if let Some(unexpected_cfgs) = rust_lints.get("unexpected_cfgs") {
            if let Some(config) = unexpected_cfgs.config() {
                if let Some(check_cfg) = config.get("check-cfg") {
                    if let Ok(check_cfgs) = toml::Value::try_into::<Vec<String>>(check_cfg.clone())
                    {
                        for check_cfg in check_cfgs {
                            rustflags.push("--check-cfg".to_string());
                            rustflags.push(check_cfg);
                        }
                    // error about `check-cfg` not being a list-of-string
                    } else {
                        bail!("`lints.rust.unexpected_cfgs.check-cfg` must be a list of string");
                    }
                }
            }
        }
    }

    Ok(rustflags)
}

fn emit_frontmatter_diagnostic(
    e: crate::util::frontmatter::FrontmatterError,
    contents: &str,
    manifest_file: &Path,
    gctx: &GlobalContext,
) -> anyhow::Error {
    let primary_span = e.primary_span();

    // Get the path to the manifest, relative to the cwd
    let manifest_path = diff_paths(manifest_file, gctx.cwd())
        .unwrap_or_else(|| manifest_file.to_path_buf())
        .display()
        .to_string();
    let group = Group::with_title(Level::ERROR.primary_title(e.message())).element(
        Snippet::source(contents)
            .path(manifest_path)
            .annotation(AnnotationKind::Primary.span(primary_span))
            .annotations(
                e.visible_spans()
                    .iter()
                    .map(|s| AnnotationKind::Visible.span(s.clone())),
            ),
    );

    if let Err(err) = gctx.shell().print_report(&[group], true) {
        return err.into();
    }
    return AlreadyPrintedError::new(e.into()).into();
}

fn emit_toml_diagnostic(
    e: toml::de::Error,
    contents: &str,
    manifest_file: &Path,
    gctx: &GlobalContext,
) -> anyhow::Error {
    let Some(span) = e.span() else {
        return e.into();
    };

    // Get the path to the manifest, relative to the cwd
    let manifest_path = diff_paths(manifest_file, gctx.cwd())
        .unwrap_or_else(|| manifest_file.to_path_buf())
        .display()
        .to_string();
    let group = Group::with_title(Level::ERROR.primary_title(e.message())).element(
        Snippet::source(contents)
            .path(manifest_path)
            .annotation(AnnotationKind::Primary.span(span)),
    );

    if let Err(err) = gctx.shell().print_report(&[group], true) {
        return err.into();
    }
    return AlreadyPrintedError::new(e.into()).into();
}

/// Warn about paths that have been deprecated and may conflict.
fn deprecated_underscore<T>(
    old: &Option<T>,
    new: &Option<T>,
    new_path: &str,
    name: &str,
    kind: &str,
    edition: Edition,
    warnings: &mut Vec<String>,
) -> CargoResult<()> {
    let old_path = new_path.replace("-", "_");
    if old.is_some() && Edition::Edition2024 <= edition {
        anyhow::bail!(
            "`{old_path}` is unsupported as of the 2024 edition; instead use `{new_path}`\n(in the `{name}` {kind})"
        );
    } else if old.is_some() && new.is_some() {
        warnings.push(format!(
            "`{old_path}` is redundant with `{new_path}`, preferring `{new_path}` in the `{name}` {kind}"
        ))
    } else if old.is_some() {
        warnings.push(format!(
            "`{old_path}` is deprecated in favor of `{new_path}` and will not work in the 2024 edition\n(in the `{name}` {kind})"
        ))
    }
    Ok(())
}

fn warn_on_unused(unused: &BTreeSet<String>, warnings: &mut Vec<String>) {
    for key in unused {
        warnings.push(format!("unused manifest key: {}", key));
        if key == "profiles.debug" {
            warnings.push("use `[profile.dev]` to configure debug builds".to_string());
        }
    }
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

/// Make the [`Package`] self-contained so its ready for packaging
pub fn prepare_for_publish(
    me: &Package,
    ws: &Workspace<'_>,
    packaged_files: Option<&[PathBuf]>,
) -> CargoResult<Package> {
    let contents = me.manifest().contents();
    let document = me.manifest().document();
    let original_toml = prepare_toml_for_publish(
        me.manifest().normalized_toml(),
        ws,
        me.root(),
        packaged_files,
    )?;
    let normalized_toml = original_toml.clone();
    let features = me.manifest().unstable_features().clone();
    let workspace_config = me.manifest().workspace_config().clone();
    let source_id = me.package_id().source_id();
    let mut warnings = Default::default();
    let mut errors = Default::default();
    let gctx = ws.gctx();
    let manifest = to_real_manifest(
        contents.to_owned(),
        document.clone(),
        original_toml,
        normalized_toml,
        features,
        workspace_config,
        source_id,
        me.manifest_path(),
        me.manifest().is_embedded(),
        gctx,
        &mut warnings,
        &mut errors,
    )?;
    let new_pkg = Package::new(manifest, me.manifest_path());
    Ok(new_pkg)
}

/// Prepares the manifest for publishing.
// - Path and git components of dependency specifications are removed.
// - License path is updated to point within the package.
fn prepare_toml_for_publish(
    me: &manifest::TomlManifest,
    ws: &Workspace<'_>,
    package_root: &Path,
    packaged_files: Option<&[PathBuf]>,
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
    // Validates if build script file is included in package. If not, warn and ignore.
    if let Some(custom_build_scripts) = package.normalized_build().expect("previously normalized") {
        let mut included_scripts = Vec::new();
        for script in custom_build_scripts {
            let path = Path::new(script).to_path_buf();
            let included = packaged_files.map(|i| i.contains(&path)).unwrap_or(true);
            if included {
                let path = path
                    .into_os_string()
                    .into_string()
                    .map_err(|_err| anyhow::format_err!("non-UTF8 `package.build`"))?;
                let path = normalize_path_string_sep(path);
                included_scripts.push(path);
            } else {
                ws.gctx().shell().warn(format!(
                    "ignoring `package.build` entry `{}` as it is not included in the published package",
                    path.display()
                ))?;
            }
        }

        package.build = Some(match included_scripts.len() {
            0 => TomlPackageBuild::Auto(false),
            1 => TomlPackageBuild::SingleScript(included_scripts[0].clone()),
            _ => TomlPackageBuild::MultipleScript(included_scripts),
        });
    }
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
        if let Ok(license_file) = abs_license_path.strip_prefix(package_root) {
            package.license_file = Some(manifest::InheritableField::Value(
                normalize_path_string_sep(
                    license_file
                        .to_str()
                        .ok_or_else(|| anyhow::format_err!("non-UTF8 `package.license-file`"))?
                        .to_owned(),
                ),
            ));
        } else {
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
                if let Ok(readme_path) = abs_readme_path.strip_prefix(package_root) {
                    package.readme = Some(manifest::InheritableField::Value(StringOrBool::String(
                        normalize_path_string_sep(
                            readme_path
                                .to_str()
                                .ok_or_else(|| {
                                    anyhow::format_err!("non-UTF8 `package.license-file`")
                                })?
                                .to_owned(),
                        ),
                    )));
                } else {
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

    let lib = if let Some(target) = &me.lib {
        prepare_target_for_publish(target, packaged_files, "library", ws.gctx())?
    } else {
        None
    };
    let bin = prepare_targets_for_publish(me.bin.as_ref(), packaged_files, "binary", ws.gctx())?;
    let example =
        prepare_targets_for_publish(me.example.as_ref(), packaged_files, "example", ws.gctx())?;
    let test = prepare_targets_for_publish(me.test.as_ref(), packaged_files, "test", ws.gctx())?;
    let bench =
        prepare_targets_for_publish(me.bench.as_ref(), packaged_files, "benchmark", ws.gctx())?;

    let all = |_d: &manifest::TomlDependency| true;
    let mut manifest = manifest::TomlManifest {
        cargo_features: me.cargo_features.clone(),
        package: Some(package),
        project: None,
        badges: me.badges.clone(),
        features: me.features.clone(),
        lib,
        bin,
        example,
        test,
        bench,
        dependencies: map_deps(gctx, me.dependencies.as_ref(), all)?,
        dev_dependencies: map_deps(
            gctx,
            me.dev_dependencies(),
            manifest::TomlDependency::is_version_specified,
        )?,
        dev_dependencies2: None,
        build_dependencies: map_deps(gctx, me.build_dependencies(), all)?,
        build_dependencies2: None,
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
        lints: me.lints.clone(),
        hints: me.hints.clone(),
        workspace: None,
        profile: me.profile.clone(),
        patch: None,
        replace: None,
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
                let feature_value = FeatureValue::new(feature_dep.into());
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
                d.base.take();
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

pub fn prepare_targets_for_publish(
    targets: Option<&Vec<manifest::TomlTarget>>,
    packaged_files: Option<&[PathBuf]>,
    context: &str,
    gctx: &GlobalContext,
) -> CargoResult<Option<Vec<manifest::TomlTarget>>> {
    let Some(targets) = targets else {
        return Ok(None);
    };

    let mut prepared = Vec::with_capacity(targets.len());
    for target in targets {
        let Some(target) = prepare_target_for_publish(target, packaged_files, context, gctx)?
        else {
            continue;
        };
        prepared.push(target);
    }

    if prepared.is_empty() {
        Ok(None)
    } else {
        Ok(Some(prepared))
    }
}

pub fn prepare_target_for_publish(
    target: &manifest::TomlTarget,
    packaged_files: Option<&[PathBuf]>,
    context: &str,
    gctx: &GlobalContext,
) -> CargoResult<Option<manifest::TomlTarget>> {
    let path = target.path.as_ref().expect("previously normalized");
    let path = &path.0;
    if let Some(packaged_files) = packaged_files {
        if !packaged_files.contains(&path) {
            let name = target.name.as_ref().expect("previously normalized");
            gctx.shell().warn(format!(
                "ignoring {context} `{name}` as `{}` is not included in the published package",
                path.display()
            ))?;
            return Ok(None);
        }
    }

    let mut target = target.clone();
    let path = normalize_path_sep(path.to_path_buf(), context)?;
    target.path = Some(manifest::PathValue(path.into()));

    Ok(Some(target))
}

fn normalize_path_sep(path: PathBuf, context: &str) -> CargoResult<PathBuf> {
    let path = path
        .into_os_string()
        .into_string()
        .map_err(|_err| anyhow::format_err!("non-UTF8 path for {context}"))?;
    let path = normalize_path_string_sep(path);
    Ok(path.into())
}

pub fn normalize_path_string_sep(path: String) -> String {
    if std::path::MAIN_SEPARATOR != '/' {
        path.replace(std::path::MAIN_SEPARATOR, "/")
    } else {
        path
    }
}
