use std::path::Path;

use core::registry::PackageRegistry;
use core::{Source, PackageId};
use ops;
use sources::PathSource;
use util::{CargoResult, Config, human, ChainError};

/// Contains informations about how a package should be fetched.
pub struct FetchOptions<'a> {
    pub config: &'a Config,
    pub num_tries: Option<u32>,
}

/// Executes `cargo fetch`.
pub fn fetch(manifest_path: &Path, opts: &FetchOptions) -> CargoResult<()> {
    let mut source = try!(PathSource::for_path(manifest_path.parent().unwrap(),
                                               opts.config));
    try!(source.update());
    let package = try!(source.root_package());

    let mut registry = PackageRegistry::new(opts.config);
    registry.preload(package.package_id().source_id(), Box::new(source));
    let resolve = try!(ops::resolve_pkg(&mut registry, &package));

    let ids: Vec<PackageId> = resolve.iter().cloned().collect();

    let num_tries = opts.num_tries.unwrap_or(0);

    try!(registry.get(&ids, num_tries).chain_error(|| {
        human("unable to get packages from source")
    }));
    Ok(())
}
