use std::path::Path;

use core::resolver::Resolve;
use core::{Source, Package};
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
pub fn output_metadata<'a>(opt: OutputMetadataOptions,
                           config: &'a Config)
                           -> CargoResult<ExportInfo> {
    let deps = try!(resolve_dependencies(opt.manifest_path,
                                         config,
                                         opt.features,
                                         opt.no_default_features));
    let (packages, resolve) = deps;

    assert_eq!(opt.version, VERSION);
    Ok(ExportInfo {
        packages: packages,
        resolve: resolve,
        version: VERSION,
    })
}

#[derive(RustcEncodable)]
pub struct ExportInfo {
    packages: Vec<Package>,
    resolve: Resolve,
    version: u32,
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
