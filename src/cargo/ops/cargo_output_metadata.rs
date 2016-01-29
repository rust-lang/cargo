use std::path::Path;

use rustc_serialize::{Encodable, Encoder};

use core::resolver::Resolve;
use core::{Source, Package, PackageId};
use ops;
use sources::PathSource;
use util::config::Config;
use util::CargoResult;

const VERSION: u32 = 1;

pub struct OutputMetadataOptions<'a> {
    pub features: Vec<String>,
    pub manifest_path: &'a Path,
    pub no_default_features: bool,
    pub version: u32,
}

/// Loads the manifest, resolves the dependencies of the project to the concrete
/// used versions - considering overrides - and writes all dependencies in a JSON
/// format to stdout.
pub fn output_metadata(opt: OutputMetadataOptions, config: &Config) -> CargoResult<ExportInfo> {
    let deps = try!(resolve_dependencies(opt.manifest_path,
                                         config,
                                         opt.features,
                                         opt.no_default_features));
    let (packages, resolve) = deps;

    assert_eq!(opt.version, VERSION);
    Ok(ExportInfo {
        packages: packages,
        resolve: MetadataResolve(resolve),
        version: VERSION,
    })
}

#[derive(RustcEncodable)]
pub struct ExportInfo {
    packages: Vec<Package>,
    resolve: MetadataResolve,
    version: u32,
}

/// Newtype wrapper to provide a custom `Encodable` implementation.
/// The one from lockfile does not fit because it uses a non-standard
/// format for `PackageId`s
struct MetadataResolve(Resolve);

impl Encodable for MetadataResolve {
    fn encode<S: Encoder>(&self, s: &mut S) -> Result<(), S::Error> {
        #[derive(RustcEncodable)]
        struct EncodableResolve<'a> {
            root: &'a PackageId,
            nodes: Vec<Node<'a>>,
        }

        #[derive(RustcEncodable)]
        struct Node<'a> {
            id: &'a PackageId,
            dependencies: Vec<&'a PackageId>,
        }

        let resolve = &self.0;
        let encodable = EncodableResolve {
            root: resolve.root(),
            nodes: resolve.iter().map(|id| {
                Node {
                    id: id,
                    dependencies: resolve.deps(id)
                        .map(|it| it.collect())
                        .unwrap_or(Vec::new()),
                }
            }).collect(),
        };

        encodable.encode(s)
    }
}

/// Loads the manifest and resolves the dependencies of the project to the
/// concrete used versions. Afterwards available overrides of dependencies are applied.
fn resolve_dependencies(manifest: &Path,
                        config: &Config,
                        features: Vec<String>,
                        no_default_features: bool)
                        -> CargoResult<(Vec<Package>, Resolve)> {
    let mut source = try!(PathSource::for_path(manifest.parent().unwrap(), config));
    try!(source.update());

    let package = try!(source.root_package());

    let deps = try!(ops::resolve_dependencies(&package,
                                              config,
                                              Some(Box::new(source)),
                                              features,
                                              no_default_features));

    let (packages, resolve_with_overrides, _) = deps;

    Ok((packages, resolve_with_overrides))
}
