use std::path::Path;

use core::registry::PackageRegistry;
use core::{Source, PackageId};
use ops;
use sources::PathSource;
use util::{CargoResult, Config, human, ChainError};

/// Executes `cargo fetch`.
pub fn fetch(manifest_path: &Path, config: &Config) -> CargoResult<()> {
    let mut source = try!(PathSource::for_path(manifest_path.parent().unwrap(),
                                               config));
    try!(source.update());
    let package = try!(source.root_package());

    let mut registry = PackageRegistry::new(config);
    registry.preload(package.package_id().source_id(), Box::new(source));
    let resolve = try!(ops::resolve_pkg(&mut registry, &package));

    let ids: Vec<PackageId> = resolve.iter().cloned().collect();
    try!(registry.get(&ids).chain_error(|| {
        human("unable to get packages from source")
    }));
    Ok(())
}
