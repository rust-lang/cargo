use rustc_serialize::{Encodable, Encoder};

use core::resolver::Resolve;
use core::{Package, PackageId, Workspace};
use ops;
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
        version: VERSION,
    })
}

fn metadata_full(ws: &Workspace,
                 opt: &OutputMetadataOptions) -> CargoResult<ExportInfo> {
    let deps = try!(ops::resolve_dependencies(ws,
                                              None,
                                              opt.features.clone(),
                                              opt.all_features,
                                              opt.no_default_features));
    let (packages, resolve) = deps;

    let packages = try!(packages.package_ids()
                                .map(|i| packages.get(i).map(|p| p.clone()))
                                .collect());

    Ok(ExportInfo {
        packages: packages,
        workspace_members: ws.members().map(|pkg| pkg.package_id().clone()).collect(),
        resolve: Some(MetadataResolve{
            resolve: resolve,
            root: ws.current_opt().map(|pkg| pkg.package_id().clone()),
        }),
        version: VERSION,
    })
}

#[derive(RustcEncodable)]
pub struct ExportInfo {
    packages: Vec<Package>,
    workspace_members: Vec<PackageId>,
    resolve: Option<MetadataResolve>,
    version: u32,
}

/// Newtype wrapper to provide a custom `Encodable` implementation.
/// The one from lockfile does not fit because it uses a non-standard
/// format for `PackageId`s
struct MetadataResolve{
    resolve: Resolve,
    root: Option<PackageId>,
}

impl Encodable for MetadataResolve {
    fn encode<S: Encoder>(&self, s: &mut S) -> Result<(), S::Error> {
        #[derive(RustcEncodable)]
        struct EncodableResolve<'a> {
            root: Option<&'a PackageId>,
            nodes: Vec<Node<'a>>,
        }

        #[derive(RustcEncodable)]
        struct Node<'a> {
            id: &'a PackageId,
            dependencies: Vec<&'a PackageId>,
        }

        let encodable = EncodableResolve {
            root: self.root.as_ref(),
            nodes: self.resolve.iter().map(|id| {
                Node {
                    id: id,
                    dependencies: self.resolve.deps(id).collect(),
                }
            }).collect(),
        };

        encodable.encode(s)
    }
}
