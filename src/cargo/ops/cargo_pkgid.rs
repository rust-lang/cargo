use crate::core::{PackageIdSpec, Workspace};
use crate::ops;
use crate::util::CargoResult;

pub fn pkgid(ws: &Workspace<'_>, spec: Option<&str>) -> CargoResult<PackageIdSpec> {
    let Some(resolve) = ops::load_pkg_lockfile(ws)? else {
        anyhow::bail!("a Cargo.lock must exist for this command")
    };

    let pkgid = match spec {
        Some(spec) => PackageIdSpec::query_str(spec, resolve.iter())?,
        None => ws.current()?.package_id(),
    };
    Ok(PackageIdSpec::from_package_id(pkgid))
}
