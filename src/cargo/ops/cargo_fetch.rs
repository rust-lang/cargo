use core::{Resolve, PackageSet, Workspace};
use ops;
use util::CargoResult;

/// Executes `cargo fetch`.
pub fn fetch<'a>(ws: &Workspace<'a>) -> CargoResult<(Resolve, PackageSet<'a>)> {
    let (packages, resolve) = ops::resolve_ws(ws)?;
    {
        let pkg_ids: Vec<_> = resolve.iter().collect();
        packages.get(&*pkg_ids)?;
    }
    Ok((resolve, packages))
}
