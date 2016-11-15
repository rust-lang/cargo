use ops;
use core::{PackageIdSpec, Workspace};
use util::CargoResult;

pub fn pkgid(ws: &Workspace, spec: Option<&str>) -> CargoResult<PackageIdSpec> {
    let resolve = match ops::load_pkg_lockfile(ws)? {
        Some(resolve) => resolve,
        None => bail!("a Cargo.lock must exist for this command"),
    };

    let pkgid = match spec {
        Some(spec) => PackageIdSpec::query_str(spec, resolve.iter())?,
        None => ws.current()?.package_id(),
    };
    Ok(PackageIdSpec::from_package_id(pkgid))
}
