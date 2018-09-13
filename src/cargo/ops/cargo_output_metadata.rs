use std::collections::HashMap;

use serde::ser;

use core::resolver::Resolve;
use core::{Package, PackageId, Workspace};
use ops::{self, Packages};
use util::CargoResult;

const VERSION: u32 = 1;

pub struct OutputMetadataOptions {
    pub features: Vec<String>,
    pub no_default_features: bool,
    pub all_features: bool,
    pub no_deps: bool,
    pub version: u32,
}

/// Loads the manifest, resolves the dependencies of the project to the concrete
/// used versions - considering overrides - and writes all dependencies in a JSON
/// format to stdout.
pub fn output_metadata(ws: &Workspace, opt: &OutputMetadataOptions) -> CargoResult<ExportInfo> {
    if opt.version != VERSION {
        bail!(
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

fn metadata_no_deps(ws: &Workspace, _opt: &OutputMetadataOptions) -> CargoResult<ExportInfo> {
    Ok(ExportInfo {
        packages: ws.members().cloned().collect(),
        workspace_members: ws.members().map(|pkg| pkg.package_id().clone()).collect(),
        resolve: None,
        target_directory: ws.target_dir().display().to_string(),
        version: VERSION,
        workspace_root: ws.root().display().to_string(),
    })
}

fn metadata_full(ws: &Workspace, opt: &OutputMetadataOptions) -> CargoResult<ExportInfo> {
    let specs = Packages::All.to_package_id_specs(ws)?;
    let (package_set, resolve) = ops::resolve_ws_precisely(
        ws,
        None,
        &opt.features,
        opt.all_features,
        opt.no_default_features,
        &specs,
    )?;
    let mut packages = HashMap::new();
    let mut downloads = package_set.enable_download()?;
    for id in package_set.package_ids() {
        if let Some(pkg) = downloads.start(id)? {
            packages.insert(id.clone(), pkg.clone());
        }
    }
    while downloads.remaining() > 0 {
        let pkg = downloads.wait()?;
        packages.insert(pkg.package_id().clone(), pkg.clone());
    }
    drop(downloads);

    Ok(ExportInfo {
        packages: packages.values().map(|p| (*p).clone()).collect(),
        workspace_members: ws.members().map(|pkg| pkg.package_id().clone()).collect(),
        resolve: Some(MetadataResolve {
            resolve: (packages, resolve),
            root: ws.current_opt().map(|pkg| pkg.package_id().clone()),
        }),
        target_directory: ws.target_dir().display().to_string(),
        version: VERSION,
        workspace_root: ws.root().display().to_string(),
    })
}

#[derive(Serialize)]
pub struct ExportInfo {
    packages: Vec<Package>,
    workspace_members: Vec<PackageId>,
    resolve: Option<MetadataResolve>,
    target_directory: String,
    version: u32,
    workspace_root: String,
}

/// Newtype wrapper to provide a custom `Serialize` implementation.
/// The one from lockfile does not fit because it uses a non-standard
/// format for `PackageId`s
#[derive(Serialize)]
struct MetadataResolve {
    #[serde(rename = "nodes", serialize_with = "serialize_resolve")]
    resolve: (HashMap<PackageId, Package>, Resolve),
    root: Option<PackageId>,
}

fn serialize_resolve<S>((packages, resolve): &(HashMap<PackageId, Package>, Resolve), s: S) -> Result<S::Ok, S::Error>
where
    S: ser::Serializer,
{
    #[derive(Serialize)]
    struct Dep<'a> {
        name: Option<String>,
        pkg: &'a PackageId
    }

    #[derive(Serialize)]
    struct Node<'a> {
        id: &'a PackageId,
        dependencies: Vec<&'a PackageId>,
        deps: Vec<Dep<'a>>,
        features: Vec<&'a str>,
    }

    s.collect_seq(resolve
        .iter()
        .map(|id| Node {
            id,
            dependencies: resolve.deps(id).map(|(pkg, _deps)| pkg).collect(),
            deps: resolve.deps(id)
                .map(|(pkg, _deps)| {
                    let name = packages.get(pkg)
                        .and_then(|pkg| pkg.targets().iter().find(|t| t.is_lib()))
                        .and_then(|lib_target| {
                            resolve.extern_crate_name(id, pkg, lib_target).ok()
                        });

                    Dep { name, pkg }
                })
                .collect(),
            features: resolve.features_sorted(id),
        }))
}
