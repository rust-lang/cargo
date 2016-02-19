use std::path::Path;

use core::registry::PackageRegistry;
use core::{Package, PackageId, Resolve, PackageSet};
use ops;
use util::{CargoResult, Config, human, ChainError};

/// Executes `cargo fetch`.
pub fn fetch<'a>(manifest_path: &Path,
                 config: &'a Config)
                 -> CargoResult<(Resolve, PackageSet<'a>)> {
    let package = try!(Package::for_path(manifest_path, config));
    let mut registry = PackageRegistry::new(config);
    let resolve = try!(ops::resolve_pkg(&mut registry, &package));
    get_resolved_packages(&resolve, registry).map(move |packages| {
        (resolve, packages)
    })
}

pub fn get_resolved_packages<'a>(resolve: &Resolve,
                                 registry: PackageRegistry<'a>)
                                 -> CargoResult<PackageSet<'a>> {
    let ids: Vec<PackageId> = resolve.iter().cloned().collect();
    registry.get(&ids).chain_error(|| {
        human("unable to get packages from source")
    })
}
