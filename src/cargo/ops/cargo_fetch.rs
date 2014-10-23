use core::{MultiShell, Package};
use core::registry::PackageRegistry;
use core::resolver::{mod, Resolve};
use core::source::Source;
use ops;
use sources::PathSource;
use util::{CargoResult, Config};
use util::profile;

/// Executes `cargo fetch`.
pub fn fetch(manifest_path: &Path,
             shell: &mut MultiShell) -> CargoResult<()> {
    let mut source = try!(PathSource::for_path(&manifest_path.dir_path()));
    try!(source.update());
    let package = try!(source.get_root_package());

    let mut config = try!(Config::new(shell, None, None));
    let mut registry = PackageRegistry::new(&mut config);
    try!(ops::resolve_pkg(&mut registry, &package));
    Ok(())
}
