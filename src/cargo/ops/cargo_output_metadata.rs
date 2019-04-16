use std::collections::HashMap;
use std::path::PathBuf;

use serde::ser;
use serde::Serialize;

use crate::core::resolver::Resolve;
use crate::core::{Package, PackageId, Workspace};
use crate::ops::{self, Packages};
use crate::util::CargoResult;

const VERSION: u32 = 1;

pub struct OutputMetadataOptions {
    pub features: Vec<String>,
    pub no_default_features: bool,
    pub all_features: bool,
    pub no_deps: bool,
    pub version: u32,
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
    if opt.no_deps {
        metadata_no_deps(ws, opt)
    } else {
        metadata_full(ws, opt)
    }
}

fn metadata_no_deps(ws: &Workspace<'_>, _opt: &OutputMetadataOptions) -> CargoResult<ExportInfo> {
    Ok(ExportInfo {
        packages: ws.members().cloned().collect(),
        workspace_members: ws.members().map(|pkg| pkg.package_id()).collect(),
        resolve: None,
        target_directory: ws.target_dir().into_path_unlocked(),
        version: VERSION,
        workspace_root: ws.root().to_path_buf(),
    })
}

fn metadata_full(ws: &Workspace<'_>, opt: &OutputMetadataOptions) -> CargoResult<ExportInfo> {
    let specs = Packages::All.to_package_id_specs(ws)?;
    let (package_set, resolve) = ops::resolve_ws_precisely(
        ws,
        &opt.features,
        opt.all_features,
        opt.no_default_features,
        &specs,
    )?;
    let mut packages = HashMap::new();
    for pkg in package_set.get_many(package_set.package_ids())? {
        packages.insert(pkg.package_id(), pkg.clone());
    }

    Ok(ExportInfo {
        packages: packages.values().map(|p| (*p).clone()).collect(),
        workspace_members: ws.members().map(|pkg| pkg.package_id()).collect(),
        resolve: Some(MetadataResolve {
            resolve: (packages, resolve),
            root: ws.current_opt().map(|pkg| pkg.package_id()),
        }),
        target_directory: ws.target_dir().into_path_unlocked(),
        version: VERSION,
        workspace_root: ws.root().to_path_buf(),
    })
}

#[derive(Serialize)]
pub struct ExportInfo {
    packages: Vec<Package>,
    workspace_members: Vec<PackageId>,
    resolve: Option<MetadataResolve>,
    target_directory: PathBuf,
    version: u32,
    workspace_root: PathBuf,
}

/// Newtype wrapper to provide a custom `Serialize` implementation.
/// The one from lock file does not fit because it uses a non-standard
/// format for `PackageId`s
#[derive(Serialize)]
struct MetadataResolve {
    #[serde(rename = "nodes", serialize_with = "serialize_resolve")]
    resolve: (HashMap<PackageId, Package>, Resolve),
    root: Option<PackageId>,
}

fn serialize_resolve<S>(
    (packages, resolve): &(HashMap<PackageId, Package>, Resolve),
    s: S,
) -> Result<S::Ok, S::Error>
where
    S: ser::Serializer,
{
    #[derive(Serialize)]
    struct Dep {
        name: String,
        pkg: PackageId,
    }

    #[derive(Serialize)]
    struct Node<'a> {
        id: PackageId,
        dependencies: Vec<PackageId>,
        deps: Vec<Dep>,
        features: Vec<&'a str>,
    }

    s.collect_seq(resolve.iter().map(|id| {
        Node {
            id,
            dependencies: resolve.deps(id).map(|(pkg, _deps)| pkg).collect(),
            deps: resolve
                .deps(id)
                .filter_map(|(pkg, _deps)| {
                    packages
                        .get(&pkg)
                        .and_then(|pkg| pkg.targets().iter().find(|t| t.is_lib()))
                        .and_then(|lib_target| resolve.extern_crate_name(id, pkg, lib_target).ok())
                        .map(|name| Dep { name, pkg })
                })
                .collect(),
            features: resolve.features_sorted(id),
        }
    }))
}
