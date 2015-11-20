use std::path::Path;

use ops;
use core::{PackageIdSpec, Package};
use util::{CargoResult, Config};

pub fn pkgid(manifest_path: &Path,
             spec: Option<&str>,
             config: &Config) -> CargoResult<PackageIdSpec> {
    let package = try!(Package::for_path(manifest_path, config));

    let lockfile = package.root().join("Cargo.lock");
    let source_id = package.package_id().source_id();
    let resolve = match try!(ops::load_lockfile(&lockfile, source_id)) {
        Some(resolve) => resolve,
        None => bail!("a Cargo.lock must exist for this command"),
    };

    let pkgid = match spec {
        Some(spec) => try!(PackageIdSpec::query_str(spec, resolve.iter())),
        None => package.package_id(),
    };
    Ok(PackageIdSpec::from_package_id(pkgid))
}
