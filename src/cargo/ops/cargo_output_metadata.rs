use crate::core::compiler::{CompileKind, CompileTarget, TargetInfo};
use crate::core::resolver::{Resolve, ResolveOpts};
use crate::core::{dependency, Dependency, Package, PackageId, Workspace};
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
        failure::bail!(
            "metadata version {} not supported, only {} is currently supported",
            opt.version,
            VERSION
        );
    }
    let (packages, resolve) = if opt.no_deps {
        let packages = ws.members().cloned().collect();
        (packages, None)
    } else {
        let resolve_opts = ResolveOpts::new(
            /*dev_deps*/ true,
            &opt.features,
            opt.all_features,
            !opt.no_default_features,
        );
        let (packages, resolve) = build_resolve_graph(ws, resolve_opts, &opt.filter_platform)?;
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
    features: Vec<String>,
}

#[derive(Serialize)]
struct Dep {
    name: String,
    pkg: PackageId,
    dep_kinds: Vec<DepKindInfo>,
}

#[derive(Serialize)]
struct DepKindInfo {
    kind: dependency::Kind,
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
    resolve_opts: ResolveOpts,
    target: &Option<String>,
) -> CargoResult<(Vec<Package>, MetadataResolve)> {
    let target_info = match target {
        Some(target) => {
            let config = ws.config();
            let ct = CompileTarget::new(target)?;
            let short_name = ct.short_name().to_string();
            let kind = CompileKind::Target(ct);
            let rustc = config.load_global_rustc(Some(ws))?;
            Some((short_name, TargetInfo::new(config, kind, &rustc, kind)?))
        }
        None => None,
    };
    // Resolve entire workspace.
    let specs = Packages::All.to_package_id_specs(ws)?;
    let ws_resolve = ops::resolve_ws_with_opts(ws, resolve_opts, &specs)?;
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
            target_info.as_ref(),
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
    target: Option<&(String, TargetInfo)>,
) {
    if node_map.contains_key(&pkg_id) {
        return;
    }
    let features = resolve
        .features_sorted(pkg_id)
        .into_iter()
        .map(|s| s.to_string())
        .collect();
    let deps: Vec<Dep> = resolve
        .deps(pkg_id)
        .filter(|(_dep_id, deps)| match target {
            Some((short_name, info)) => deps.iter().any(|dep| {
                let platform = match dep.platform() {
                    Some(p) => p,
                    None => return true,
                };
                platform.matches(short_name, info.cfg())
            }),
            None => true,
        })
        .filter_map(|(dep_id, deps)| {
            package_map
                .get(&dep_id)
                .and_then(|pkg| pkg.targets().iter().find(|t| t.is_lib()))
                .and_then(|lib_target| resolve.extern_crate_name(pkg_id, dep_id, lib_target).ok())
                .map(|name| Dep {
                    name,
                    pkg: dep_id,
                    dep_kinds: deps.iter().map(DepKindInfo::from).collect(),
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
        build_resolve_graph_r(node_map, dep_id, resolve, package_map, target);
    }
}
