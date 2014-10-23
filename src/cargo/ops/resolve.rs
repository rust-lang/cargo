use std::collections::{HashMap, HashSet};

use core::{Package, PackageId};
use core::registry::PackageRegistry;
use core::resolver::{mod, Resolve};
use ops;
use util::CargoResult;

/// Resolve all dependencies for the specified `package` using the previous
/// lockfile as a guide if present.
///
/// This function will also generate a write the result of resolution as a new
/// lockfile.
pub fn resolve_pkg(registry: &mut PackageRegistry, package: &Package)
                   -> CargoResult<Resolve> {
    let prev = try!(ops::load_pkg_lockfile(package));
    let resolve = try!(resolve_with_previous(registry, package,
                                             resolver::ResolveEverything,
                                             prev.as_ref(), None));
    try!(ops::write_pkg_lockfile(package, &resolve));
    Ok(resolve)
}

/// Resolve all dependencies for a package using an optional previous instance
/// of resolve to guide the resolution process.
///
/// This also takes an optional hash set, `to_avoid`, which is a list of package
/// ids that should be avoided when consulting the previous instance of resolve
/// (often used in pairings with updates).
///
/// The previous resolve normally comes from a lockfile. This function does not
/// read or write lockfiles from the filesystem.
pub fn resolve_with_previous<'a>(registry: &mut PackageRegistry,
                                 package: &Package,
                                 method: resolver::ResolveMethod,
                                 previous: Option<&'a Resolve>,
                                 to_avoid: Option<&HashSet<&'a PackageId>>)
                                 -> CargoResult<Resolve> {
    let root = package.get_package_id().get_source_id().clone();
    try!(registry.add_sources(&[root]));

    let summary = package.get_summary().clone();
    let summary = match previous {
        Some(r) => {
            // In the case where a previous instance of resolve is available, we
            // want to lock as many packages as possible to the previous version
            // without disturbing the graph structure. To this end we perform
            // two actions here:
            //
            // 1. We inform the package registry of all locked packages. This
            //    involves informing it of both the locked package's id as well
            //    as the versions of all locked dependencies. The registry will
            //    then takes this information into account when it is queried.
            //
            // 2. The specified package's summary will have its dependencies
            //    modified to their precise variants. This will instruct the
            //    first step of the resolution process to not query for ranges
            //    but rather precise dependency versions.
            //
            //    This process must handle altered dependencies, however, as
            //    it's possible for a manifest to change over time to have
            //    dependencies added, removed, or modified to different version
            //    ranges. To deal with this, we only actually lock a dependency
            //    to the previously resolved version if the dependency listed
            //    still matches the locked version.
            for node in r.iter().filter(|p| keep(p, to_avoid)) {
                let deps = r.deps(node).into_iter().flat_map(|i| i)
                            .filter(|p| keep(p, to_avoid))
                            .map(|p| p.clone()).collect();
                registry.register_lock(node.clone(), deps);
            }

            let map = r.deps(r.root()).into_iter().flat_map(|i| i).filter(|p| {
                keep(p, to_avoid)
            }).map(|d| {
                (d.get_name(), d)
            }).collect::<HashMap<_, _>>();
            summary.map_dependencies(|d| {
                match map.find_equiv(&d.get_name()) {
                    Some(&lock) if d.matches_id(lock) => d.lock_to(lock),
                    _ => d,
                }
            })
        }
        None => summary,
    };

    let mut resolved = try!(resolver::resolve(&summary, method, registry));
    match previous {
        Some(r) => resolved.copy_metadata(previous),
        None => {}
    }
    return Ok(resolved);

    fn keep<'a>(p: &&'a PackageId, to_avoid: Option<&HashSet<&'a PackageId>>)
                -> bool {
        match to_avoid {
            Some(set) => !set.contains(p),
            None => true,
        }
    }
}
