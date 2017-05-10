use serde::ser::{self, Serialize};

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
pub fn output_metadata(ws: &Workspace,
                       opt: &OutputMetadataOptions) -> CargoResult<ExportInfo> {
    if opt.version != VERSION {
        bail!("metadata version {} not supported, only {} is currently supported",
              opt.version, VERSION);
    }
    if opt.no_deps {
        metadata_no_deps(ws, opt)
    } else {
        metadata_full(ws, opt)
    }
}

fn metadata_no_deps(ws: &Workspace,
                    _opt: &OutputMetadataOptions) -> CargoResult<ExportInfo> {
    Ok(ExportInfo {
        packages: ws.members().cloned().collect(),
        workspace_members: ws.members().map(|pkg| pkg.package_id().clone()).collect(),
        resolve: None,
        target_directory: ws.target_dir().display().to_string(),
        version: VERSION,
    })
}

fn metadata_full(ws: &Workspace,
                 opt: &OutputMetadataOptions) -> CargoResult<ExportInfo> {
    let specs = Packages::All.into_package_id_specs(ws)?;
    let deps = ops::resolve_ws_precisely(ws,
                                         None,
                                         &opt.features,
                                         opt.all_features,
                                         opt.no_default_features,
                                         &specs)?;
    let (packages, resolve) = deps;

    let packages = packages.package_ids()
                           .map(|i| packages.get(i).map(|p| p.clone()))
                           .collect::<CargoResult<Vec<_>>>()?;

    Ok(ExportInfo {
        packages: packages,
        workspace_members: ws.members().map(|pkg| pkg.package_id().clone()).collect(),
        resolve: Some(MetadataResolve{
            resolve: resolve,
            root: ws.current_opt().map(|pkg| pkg.package_id().clone()),
        }),
        target_directory: ws.target_dir().display().to_string(),
        version: VERSION,
    })
}

#[derive(Serialize)]
pub struct ExportInfo {
    packages: Vec<Package>,
    workspace_members: Vec<PackageId>,
    resolve: Option<MetadataResolve>,
    target_directory: String,
    version: u32,
}

/// Newtype wrapper to provide a custom `Serialize` implementation.
/// The one from lockfile does not fit because it uses a non-standard
/// format for `PackageId`s
#[derive(Serialize)]
struct MetadataResolve {
    #[serde(rename = "nodes", serialize_with = "serialize_resolve")]
    resolve: Resolve,
    root: Option<PackageId>,
}

fn serialize_resolve<S>(resolve: &Resolve, s: S) -> Result<S::Ok, S::Error>
    where S: ser::Serializer,
{
    #[derive(Serialize)]
    struct Node<'a> {
        id: &'a PackageId,
        dependencies: Vec<&'a PackageId>,
    }

    resolve.iter().map(|id| {
        Node {
            id: id,
            dependencies: resolve.deps(id).collect(),
        }
    }).collect::<Vec<_>>().serialize(s)
}
