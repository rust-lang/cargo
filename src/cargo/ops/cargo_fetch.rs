use core::registry::PackageRegistry;
use core::{PackageId, Resolve, PackageSet, Workspace};
use ops;
use util::CargoResult;

/// Executes `cargo fetch`.
pub fn fetch<'a>(ws: &Workspace<'a>) -> CargoResult<(Resolve, PackageSet<'a>)> {
    let mut registry = PackageRegistry::new(ws.config());
    let resolve = try!(ops::resolve_ws(&mut registry, ws));
    let packages = get_resolved_packages(&resolve, registry);
    for id in resolve.iter() {
        try!(packages.get(id));
    }
    Ok((resolve, packages))
}

pub fn get_resolved_packages<'a>(resolve: &Resolve,
                                 registry: PackageRegistry<'a>)
                                 -> PackageSet<'a> {
    let ids: Vec<PackageId> = resolve.iter().cloned().collect();
    registry.get(&ids)
}
