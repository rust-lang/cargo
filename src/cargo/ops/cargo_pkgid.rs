use ops;
use core::{PackageIdSpec, Workspace};
use util::CargoResult;

pub fn pkgid(ws: &Workspace, spec: Option<&str>) -> CargoResult<PackageIdSpec> {
    let resolve = match try!(ops::load_pkg_lockfile(ws)) {
        Some(resolve) => resolve,
        None => bail!("a Cargo.lock must exist for this command"),
    };

    let pkgid = match spec {
        Some(spec) => try!(PackageIdSpec::query_str(spec, resolve.iter())),
        None => try!(ws.current()).package_id(),
    };
    Ok(PackageIdSpec::from_package_id(pkgid))
}
