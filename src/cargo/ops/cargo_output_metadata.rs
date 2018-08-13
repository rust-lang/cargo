use serde::ser;

use core::resolver::Resolve;
use core::{Package, PackageId, Workspace, PackageSet};
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
pub fn output_metadata<'a>(ws: &'a Workspace, opt: &OutputMetadataOptions) -> CargoResult<ExportInfo<'a>> {
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

fn metadata_no_deps<'a>(ws: &'a Workspace, _opt: &OutputMetadataOptions) -> CargoResult<ExportInfo<'a>> {
    Ok(ExportInfo {
        packages: ws.members().cloned().collect(),
        workspace_members: ws.members().map(|pkg| pkg.package_id().clone()).collect(),
        resolve: None,
        target_directory: ws.target_dir().display().to_string(),
        version: VERSION,
        workspace_root: ws.root().display().to_string(),
    })
}

fn metadata_full<'a>(ws: &'a Workspace, opt: &OutputMetadataOptions) -> CargoResult<ExportInfo<'a>> {
    let specs = Packages::All.to_package_id_specs(ws)?;
    let deps = ops::resolve_ws_precisely(
        ws,
        None,
        &opt.features,
        opt.all_features,
        opt.no_default_features,
        &specs,
    )?;
    let (package_set, resolve) = deps;

    let packages = package_set
        .package_ids()
        .map(|i| package_set.get(i).map(|p| p.clone()))
        .collect::<CargoResult<Vec<_>>>()?;

    Ok(ExportInfo {
        packages,
        workspace_members: ws.members().map(|pkg| pkg.package_id().clone()).collect(),
        resolve: Some(MetadataResolve {
            resolve: (package_set, resolve),
            root: ws.current_opt().map(|pkg| pkg.package_id().clone()),
        }),
        target_directory: ws.target_dir().display().to_string(),
        version: VERSION,
        workspace_root: ws.root().display().to_string(),
    })
}

#[derive(Serialize)]
pub struct ExportInfo<'a> {
    packages: Vec<Package>,
    workspace_members: Vec<PackageId>,
    resolve: Option<MetadataResolve<'a>>,
    target_directory: String,
    version: u32,
    workspace_root: String,
}

/// Newtype wrapper to provide a custom `Serialize` implementation.
/// The one from lockfile does not fit because it uses a non-standard
/// format for `PackageId`s
#[derive(Serialize)]
struct MetadataResolve<'a> {
    #[serde(rename = "nodes", serialize_with = "serialize_resolve")]
    resolve: (PackageSet<'a>, Resolve),
    root: Option<PackageId>,
}

fn serialize_resolve<S>((package_set, resolve): &(PackageSet, Resolve), s: S) -> Result<S::Ok, S::Error>
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
                    let name = package_set.get(pkg).ok()
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
