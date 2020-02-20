use crate::core::compiler::{CompileKind, CompileTarget, RustcTargetData};
use crate::core::dependency::DepKind;
use crate::core::resolver::{HasDevUnits, Resolve, ResolveOpts};
use crate::core::{Dependency, InternedString, Package, PackageId, Workspace};
use crate::ops::{self, Packages};
use crate::util::CargoResult;
use cargo_platform::Platform;
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;

const VERSION: u32 = 1;

pub struct OutputMetadataOptions {
    pub features: Vec<String>,
    pub no_default_features: bool,
    pub all_features: bool,
    pub no_deps: bool,
    pub version: u32,
    pub filter_platform: Option<String>,
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
        let packages = ws.members().cloned().collect();
        (packages, None)
    } else {
        let (packages, resolve) = build_resolve_graph(ws, opt)?;
        (packages, Some(resolve))
    };

    Ok(ExportInfo {
        packages,
        workspace_members: ws.members().map(|pkg| pkg.package_id()).collect(),
        resolve,
        target_directory: ws.target_dir().into_path_unlocked(),
        version: VERSION,
        workspace_root: ws.root().to_path_buf(),
    })
}

/// This is the structure that is serialized and displayed to the user.
///
/// See cargo-metadata.adoc for detailed documentation of the format.
#[derive(Serialize)]
pub struct ExportInfo {
    packages: Vec<Package>,
    workspace_members: Vec<PackageId>,
    resolve: Option<MetadataResolve>,
    target_directory: PathBuf,
    version: u32,
    workspace_root: PathBuf,
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
    name: String,
    pkg: PackageId,
    dep_kinds: Vec<DepKindInfo>,
}

#[derive(Serialize, PartialEq, Eq, PartialOrd, Ord)]
struct DepKindInfo {
    kind: DepKind,
    target: Option<Platform>,
}

impl From<&Dependency> for DepKindInfo {
    fn from(dep: &Dependency) -> DepKindInfo {
        DepKindInfo {
            kind: dep.kind(),
            target: dep.platform().cloned(),
        }
    }
}

/// Builds the resolve graph as it will be displayed to the user.
fn build_resolve_graph(
    ws: &Workspace<'_>,
    metadata_opts: &OutputMetadataOptions,
) -> CargoResult<(Vec<Package>, MetadataResolve)> {
    // TODO: Without --filter-platform, features are being resolved for `host` only.
    // How should this work?
    let requested_kind = match &metadata_opts.filter_platform {
        Some(t) => CompileKind::Target(CompileTarget::new(t)?),
        None => CompileKind::Host,
    };
    let target_data = RustcTargetData::new(ws, requested_kind)?;
    // Resolve entire workspace.
    let specs = Packages::All.to_package_id_specs(ws)?;
    let resolve_opts = ResolveOpts::new(
        /*dev_deps*/ true,
        &metadata_opts.features,
        metadata_opts.all_features,
        !metadata_opts.no_default_features,
    );
    let ws_resolve = ops::resolve_ws_with_opts(
        ws,
        &target_data,
        requested_kind,
        &resolve_opts,
        &specs,
        HasDevUnits::Yes,
    )?;
    // Download all Packages. This is needed to serialize the information
    // for every package. In theory this could honor target filtering,
    // but that would be somewhat complex.
    let mut package_map: HashMap<PackageId, Package> = ws_resolve
        .pkg_set
        .get_many(ws_resolve.pkg_set.package_ids())?
        .into_iter()
        .map(|pkg| (pkg.package_id(), pkg.clone()))
        .collect();

    // Start from the workspace roots, and recurse through filling out the
    // map, filtering targets as necessary.
    let mut node_map = HashMap::new();
    for member_pkg in ws.members() {
        build_resolve_graph_r(
            &mut node_map,
            member_pkg.package_id(),
            &ws_resolve.targeted_resolve,
            &package_map,
            &target_data,
            requested_kind,
        );
    }
    // Get a Vec of Packages.
    let actual_packages = package_map
        .drain()
        .filter_map(|(pkg_id, pkg)| node_map.get(&pkg_id).map(|_| pkg))
        .collect();
    let mr = MetadataResolve {
        nodes: node_map.drain().map(|(_pkg_id, node)| node).collect(),
        root: ws.current_opt().map(|pkg| pkg.package_id()),
    };
    Ok((actual_packages, mr))
}

fn build_resolve_graph_r(
    node_map: &mut HashMap<PackageId, MetadataResolveNode>,
    pkg_id: PackageId,
    resolve: &Resolve,
    package_map: &HashMap<PackageId, Package>,
    target_data: &RustcTargetData,
    requested_kind: CompileKind,
) {
    if node_map.contains_key(&pkg_id) {
        return;
    }
    let features = resolve.features(pkg_id).to_vec();

    let deps: Vec<Dep> = resolve
        .deps(pkg_id)
        .filter(|(_dep_id, deps)| match requested_kind {
            CompileKind::Target(_) => deps
                .iter()
                .any(|dep| target_data.dep_platform_activated(dep, requested_kind)),
            // No --filter-platform is interpreted as "all platforms".
            CompileKind::Host => true,
        })
        .filter_map(|(dep_id, deps)| {
            let mut dep_kinds: Vec<_> = deps.iter().map(DepKindInfo::from).collect();
            // Duplicates may appear if the same package is used by different
            // members of a workspace with different features selected.
            dep_kinds.sort_unstable();
            dep_kinds.dedup();
            package_map
                .get(&dep_id)
                .and_then(|pkg| pkg.targets().iter().find(|t| t.is_lib()))
                .and_then(|lib_target| resolve.extern_crate_name(pkg_id, dep_id, lib_target).ok())
                .map(|name| Dep {
                    name,
                    pkg: dep_id,
                    dep_kinds,
                })
        })
        .collect();
    let dumb_deps: Vec<PackageId> = deps.iter().map(|dep| dep.pkg).collect();
    let to_visit = dumb_deps.clone();
    let node = MetadataResolveNode {
        id: pkg_id,
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
            requested_kind,
        );
    }
}
