use crate::core::compiler::artifact::match_artifacts_kind_with_targets;
use crate::core::compiler::{CompileKind, RustcTargetData};
use crate::core::dependency::DepKind;
use crate::core::package::SerializedPackage;
use crate::core::resolver::{features::CliFeatures, HasDevUnits, Resolve};
use crate::core::{Package, PackageId, Workspace};
use crate::ops::{self, Packages};
use crate::util::interning::InternedString;
use crate::util::CargoResult;
use cargo_platform::Platform;
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::PathBuf;

const VERSION: u32 = 1;

pub struct OutputMetadataOptions {
    pub cli_features: CliFeatures,
    pub no_deps: bool,
    pub version: u32,
    pub filter_platforms: Vec<String>,
}

/// Loads the manifest, resolves the dependencies of the package to the concrete
/// used versions - considering overrides - and writes all dependencies in a JSON
/// format to stdout.
pub fn output_metadata(ws: &Workspace<'_>, opt: &OutputMetadataOptions) -> CargoResult<ExportInfo> {
    if opt.version != VERSION {
        anyhow::bail!(
            "metadata version {} not supported, only {} is currently supported",
            opt.version,
            VERSION
        );
    }
    let (packages, resolve) = if opt.no_deps {
        let packages = ws.members().map(|pkg| pkg.serialized()).collect();
        (packages, None)
    } else {
        let (packages, resolve) = build_resolve_graph(ws, opt)?;
        (packages, Some(resolve))
    };

    Ok(ExportInfo {
        packages,
        workspace_members: ws.members().map(|pkg| pkg.package_id()).collect(),
        workspace_default_members: ws.default_members().map(|pkg| pkg.package_id()).collect(),
        resolve,
        target_directory: ws.target_dir().into_path_unlocked(),
        version: VERSION,
        workspace_root: ws.root().to_path_buf(),
        metadata: ws.custom_metadata().cloned(),
    })
}

/// This is the structure that is serialized and displayed to the user.
///
/// See cargo-metadata.adoc for detailed documentation of the format.
#[derive(Serialize)]
pub struct ExportInfo {
    packages: Vec<SerializedPackage>,
    workspace_members: Vec<PackageId>,
    workspace_default_members: Vec<PackageId>,
    resolve: Option<MetadataResolve>,
    target_directory: PathBuf,
    version: u32,
    workspace_root: PathBuf,
    metadata: Option<toml::Value>,
}

#[derive(Serialize)]
struct MetadataResolve {
    nodes: Vec<MetadataResolveNode>,
    root: Option<PackageId>,
}

#[derive(Serialize)]
struct MetadataResolveNode {
    id: PackageId,
    dependencies: Vec<PackageId>,
    deps: Vec<Dep>,
    features: Vec<InternedString>,
}

#[derive(Serialize)]
struct Dep {
    // TODO(bindeps): after -Zbindeps gets stabilized,
    // mark this field as deprecated in the help manual of cargo-metadata
    name: InternedString,
    pkg: PackageId,
    dep_kinds: Vec<DepKindInfo>,
}

#[derive(Serialize, PartialEq, Eq, PartialOrd, Ord)]
struct DepKindInfo {
    kind: DepKind,
    target: Option<Platform>,

    // vvvvv The fields below are introduced for `-Z bindeps`.
    /// What the manifest calls the crate.
    ///
    /// A renamed dependency will show the rename instead of original name.
    // TODO(bindeps): Remove `Option` after -Zbindeps get stabilized.
    #[serde(skip_serializing_if = "Option::is_none")]
    extern_name: Option<InternedString>,
    /// Artifact's crate type, e.g. staticlib, cdylib, bin...
    #[serde(skip_serializing_if = "Option::is_none")]
    artifact: Option<&'static str>,
    /// Equivalent to `{ target = "…" }` in an artifact dependency requirement.
    ///
    /// * If the target points to a custom target JSON file, the path will be absolute.
    /// * If the target is a build assumed target `{ target = "target" }`, it will show as `<target>`.
    #[serde(skip_serializing_if = "Option::is_none")]
    compile_target: Option<InternedString>,
    /// Executable name for an artifact binary dependency.
    #[serde(skip_serializing_if = "Option::is_none")]
    bin_name: Option<String>,
    // ^^^^^ The fields above are introduced for `-Z bindeps`.
}

/// Builds the resolve graph as it will be displayed to the user.
fn build_resolve_graph(
    ws: &Workspace<'_>,
    metadata_opts: &OutputMetadataOptions,
) -> CargoResult<(Vec<SerializedPackage>, MetadataResolve)> {
    // TODO: Without --filter-platform, features are being resolved for `host` only.
    // How should this work?
    let requested_kinds =
        CompileKind::from_requested_targets(ws.config(), &metadata_opts.filter_platforms)?;
    let mut target_data = RustcTargetData::new(ws, &requested_kinds)?;
    // Resolve entire workspace.
    let specs = Packages::All.to_package_id_specs(ws)?;
    let force_all = if metadata_opts.filter_platforms.is_empty() {
        crate::core::resolver::features::ForceAllTargets::Yes
    } else {
        crate::core::resolver::features::ForceAllTargets::No
    };

    let max_rust_version = ws.rust_version();

    // Note that even with --filter-platform we end up downloading host dependencies as well,
    // as that is the behavior of download_accessible.
    let ws_resolve = ops::resolve_ws_with_opts(
        ws,
        &mut target_data,
        &requested_kinds,
        &metadata_opts.cli_features,
        &specs,
        HasDevUnits::Yes,
        force_all,
        max_rust_version,
    )?;

    let package_map: BTreeMap<PackageId, Package> = ws_resolve
        .pkg_set
        .packages()
        // This is a little lazy, but serde doesn't handle Rc fields very well.
        .map(|pkg| (pkg.package_id(), Package::clone(pkg)))
        .collect();

    // Start from the workspace roots, and recurse through filling out the
    // map, filtering targets as necessary.
    let mut node_map = BTreeMap::new();
    for member_pkg in ws.members() {
        build_resolve_graph_r(
            &mut node_map,
            member_pkg.package_id(),
            &ws_resolve.targeted_resolve,
            &package_map,
            &target_data,
            &requested_kinds,
        )?;
    }
    // Get a Vec of Packages.
    let actual_packages = package_map
        .into_iter()
        .filter_map(|(pkg_id, pkg)| node_map.get(&pkg_id).map(|_| pkg))
        .map(|pkg| pkg.serialized())
        .collect();

    let mr = MetadataResolve {
        nodes: node_map.into_iter().map(|(_pkg_id, node)| node).collect(),
        root: ws.current_opt().map(|pkg| pkg.package_id()),
    };
    Ok((actual_packages, mr))
}

fn build_resolve_graph_r(
    node_map: &mut BTreeMap<PackageId, MetadataResolveNode>,
    pkg_id: PackageId,
    resolve: &Resolve,
    package_map: &BTreeMap<PackageId, Package>,
    target_data: &RustcTargetData<'_>,
    requested_kinds: &[CompileKind],
) -> CargoResult<()> {
    if node_map.contains_key(&pkg_id) {
        return Ok(());
    }
    // This normalizes the IDs so that they are consistent between the
    // `packages` array and the `resolve` map. This is a bit of a hack to
    // compensate for the fact that
    // SourceKind::Git(GitReference::Branch("master")) is the same as
    // SourceKind::Git(GitReference::DefaultBranch). We want IDs in the JSON
    // to be opaque, and compare with basic string equality, so this will
    // always prefer the style of ID in the Package instead of the resolver.
    // Cargo generally only exposes PackageIds from the Package struct, and
    // AFAIK this is the only place where the resolver variant is exposed.
    //
    // This diverges because the SourceIds created for Packages are built
    // based on the Dependency declaration, but the SourceIds in the resolver
    // are deserialized from Cargo.lock. Cargo.lock may have been generated by
    // an older (or newer!) version of Cargo which uses a different style.
    let normalize_id = |id| -> PackageId { *package_map.get_key_value(&id).unwrap().0 };
    let features = resolve.features(pkg_id).to_vec();

    let deps = {
        let mut dep_metadatas = Vec::new();
        let iter = resolve.deps(pkg_id).filter(|(_dep_id, deps)| {
            if requested_kinds == [CompileKind::Host] {
                true
            } else {
                requested_kinds.iter().any(|kind| {
                    deps.iter()
                        .any(|dep| target_data.dep_platform_activated(dep, *kind))
                })
            }
        });
        for (dep_id, deps) in iter {
            let mut dep_kinds = Vec::new();

            let targets = package_map[&dep_id].targets();

            // Try to get the extern name for lib, or crate name for bins.
            let extern_name = |target| {
                resolve
                    .extern_crate_name_and_dep_name(pkg_id, dep_id, target)
                    .map(|(ext_crate_name, _)| ext_crate_name)
            };

            let lib_target = targets.iter().find(|t| t.is_lib());

            for dep in deps.iter() {
                if let Some(target) = lib_target {
                    // When we do have a library target, include them in deps if...
                    let included = match dep.artifact() {
                        // it is not an artifact dep at all
                        None => true,
                        // it is also an artifact dep with `{ …, lib = true }`
                        Some(a) if a.is_lib() => true,
                        _ => false,
                    };
                    // TODO(bindeps): Cargo shouldn't have `extern_name` field
                    // if the user is not using -Zbindeps.
                    // Remove this condition ` after -Zbindeps gets stabilized.
                    let extern_name = if dep.artifact().is_some() {
                        Some(extern_name(target)?)
                    } else {
                        None
                    };
                    if included {
                        dep_kinds.push(DepKindInfo {
                            kind: dep.kind(),
                            target: dep.platform().cloned(),
                            extern_name,
                            artifact: None,
                            compile_target: None,
                            bin_name: None,
                        });
                    }
                }

                // No need to proceed if there is no artifact dependency.
                let Some(artifact_requirements) = dep.artifact() else {
                    continue;
                };

                let compile_target = match artifact_requirements.target() {
                    Some(t) => t
                        .to_compile_target()
                        .map(|t| t.rustc_target())
                        // Given that Cargo doesn't know which target it should resolve to,
                        // when an artifact dep is specified with { target = "target" },
                        // keep it with a special "<target>" string,
                        .or_else(|| Some(InternedString::new("<target>"))),
                    None => None,
                };

                let target_set =
                    match_artifacts_kind_with_targets(dep, targets, pkg_id.name().as_str())?;
                dep_kinds.reserve(target_set.len());
                for (kind, target) in target_set.into_iter() {
                    dep_kinds.push(DepKindInfo {
                        kind: dep.kind(),
                        target: dep.platform().cloned(),
                        extern_name: extern_name(target).ok(),
                        artifact: Some(kind.crate_type()),
                        compile_target,
                        bin_name: target.is_bin().then(|| target.name().to_string()),
                    })
                }
            }

            dep_kinds.sort();

            let pkg = normalize_id(dep_id);

            let dep = match (lib_target, dep_kinds.len()) {
                (Some(target), _) => Dep {
                    name: extern_name(target)?,
                    pkg,
                    dep_kinds,
                },
                // No lib target exists but contains artifact deps.
                (None, 1..) => Dep {
                    name: InternedString::new(""),
                    pkg,
                    dep_kinds,
                },
                // No lib or artifact dep exists.
                // Usually this mean parent depending on non-lib bin crate.
                (None, _) => continue,
            };

            dep_metadatas.push(dep)
        }
        dep_metadatas
    };

    let dumb_deps: Vec<PackageId> = deps.iter().map(|dep| dep.pkg).collect();
    let to_visit = dumb_deps.clone();
    let node = MetadataResolveNode {
        id: normalize_id(pkg_id),
        dependencies: dumb_deps,
        deps,
        features,
    };
    node_map.insert(pkg_id, node);
    for dep_id in to_visit {
        build_resolve_graph_r(
            node_map,
            dep_id,
            resolve,
            package_map,
            target_data,
            requested_kinds,
        )?;
    }

    Ok(())
}
