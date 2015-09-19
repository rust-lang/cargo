use std::path::Path;

use core::registry::PackageRegistry;
use core::{Package, PackageId};
use ops;
use util::{CargoResult, Config, human, ChainError};

/// Executes `cargo fetch`.
pub fn fetch(manifest_path: &Path, config: &Config) -> CargoResult<()> {
    let package = try!(Package::for_path(manifest_path, config));
    let mut registry = PackageRegistry::new(config);
    let resolve = try!(ops::resolve_pkg(&mut registry, &package));

    let ids: Vec<PackageId> = resolve.iter().cloned().collect();
    try!(registry.get(&ids).chain_error(|| {
        human("unable to get packages from source")
    }));
    Ok(())
}
