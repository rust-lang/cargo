use std::path::Path;

use core::registry::PackageRegistry;
use core::{Package, PackageId, Resolve};
use ops;
use util::{CargoResult, Config, human, ChainError};

/// Executes `cargo fetch`.
pub fn fetch(manifest_path: &Path, config: &Config) -> CargoResult<()> {
    let package = try!(Package::for_path(manifest_path, config));
    let mut registry = PackageRegistry::new(config);
    let resolve = try!(ops::resolve_pkg(&mut registry, &package));
    let _ = get_resolved_packages(&resolve, &mut registry);
    Ok(())
}

pub fn get_resolved_packages(resolve: &Resolve, registry: &mut PackageRegistry)
                             -> CargoResult<Vec<Package>> {
    let ids: Vec<PackageId> = resolve.iter().cloned().collect();
    registry.get(&ids).chain_error(|| {
        human("Unable to get packages from source")
    })
}
