use core::registry::PackageRegistry;
use core::source::Source;
use ops;
use sources::PathSource;
use util::{CargoResult, Config};

/// Executes `cargo fetch`.
pub fn fetch(manifest_path: &Path, config: &Config) -> CargoResult<()> {
    let mut source = try!(PathSource::for_path(&manifest_path.dir_path(),
                                               config));
    try!(source.update());
    let package = try!(source.get_root_package());

    let mut registry = PackageRegistry::new(config);
    try!(ops::resolve_pkg(&mut registry, &package));
    Ok(())
}
