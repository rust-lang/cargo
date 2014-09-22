use ops;
use core::{MultiShell, Source, PackageIdSpec};
use sources::{PathSource};
use util::{CargoResult, human};

pub fn pkgid(manifest_path: &Path,
             spec: Option<&str>,
             _shell: &mut MultiShell) -> CargoResult<PackageIdSpec> {
    let mut source = try!(PathSource::for_path(&manifest_path.dir_path()));
    try!(source.update());
    let package = try!(source.get_root_package());

    let lockfile = package.get_root().join("Cargo.lock");
    let source_id = package.get_package_id().get_source_id();
    let resolve = match try!(ops::load_lockfile(&lockfile, source_id)) {
        Some(resolve) => resolve,
        None => return Err(human("A Cargo.lock must exist for this command"))
    };

    let pkgid = match spec {
        Some(spec) => try!(resolve.query(spec)),
        None => package.get_package_id(),
    };
    Ok(PackageIdSpec::from_package_id(pkgid))
}
