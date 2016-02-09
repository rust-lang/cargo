use std::collections::{HashMap, HashSet};

use core::{Package, PackageId, SourceId};
use core::registry::PackageRegistry;
use core::resolver::{self, Resolve, Method};
use ops;
use util::{CargoResult, Config};

/// Resolve all dependencies for the specified `package` using the previous
/// lockfile as a guide if present.
///
/// This function will also write the result of resolution as a new
/// lockfile.
pub fn resolve_pkg(registry: &mut PackageRegistry,
                   package: &Package,
                   config: &Config)
                   -> CargoResult<Resolve> {
    let prev = try!(ops::load_pkg_lockfile(package, config));
    let resolve = try!(resolve_with_previous(registry, package,
                                             Method::Everything,
                                             prev.as_ref(), None));
    if package.package_id().source_id().is_path() {
        try!(ops::write_pkg_lockfile(package, &resolve));
    }
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
                                 method: Method,
                                 previous: Option<&'a Resolve>,
                                 to_avoid: Option<&HashSet<&'a PackageId>>)
                                 -> CargoResult<Resolve> {
    try!(registry.add_sources(&[package.package_id().source_id()
                                       .clone()]));

    // Here we place an artificial limitation that all non-registry sources
    // cannot be locked at more than one revision. This means that if a git
    // repository provides more than one package, they must all be updated in
    // step when any of them are updated.
    //
    // TODO: This seems like a hokey reason to single out the registry as being
    //       different
    let mut to_avoid_sources = HashSet::new();
    if let Some(to_avoid) = to_avoid {
        to_avoid_sources.extend(to_avoid.iter()
                                        .map(|p| p.source_id())
                                        .filter(|s| !s.is_registry()));
    }

    let summary = package.summary().clone();
    let (summary, replace) = match previous {
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
            //    but rather for precise dependency versions.
            //
            //    This process must handle altered dependencies, however, as
            //    it's possible for a manifest to change over time to have
            //    dependencies added, removed, or modified to different version
            //    ranges. To deal with this, we only actually lock a dependency
            //    to the previously resolved version if the dependency listed
            //    still matches the locked version.
            for node in r.iter().filter(|p| keep(p, to_avoid, &to_avoid_sources)) {
                let deps = r.deps_not_replaced(node)
                            .filter(|p| keep(p, to_avoid, &to_avoid_sources))
                            .cloned().collect();
                registry.register_lock(node.clone(), deps);
            }

            let summary = {
                let map = r.deps_not_replaced(r.root()).filter(|p| {
                    keep(p, to_avoid, &to_avoid_sources)
                }).map(|d| {
                    (d.name(), d)
                }).collect::<HashMap<_, _>>();

                summary.map_dependencies(|dep| {
                    match map.get(dep.name()) {
                        Some(&lock) if dep.matches_id(lock) => dep.lock_to(lock),
                        _ => dep,
                    }
                })
            };
            let replace = package.manifest().replace();
            let replace = replace.iter().map(|&(ref spec, ref dep)| {
                for (key, val) in r.replacements().iter() {
                    if spec.matches(key) && dep.matches_id(val) {
                        return (spec.clone(), dep.clone().lock_to(val))
                    }
                }
                (spec.clone(), dep.clone())
            }).collect::<Vec<_>>();
            (summary, replace)
        }
        None => (summary, package.manifest().replace().to_owned()),
    };

    let mut resolved = try!(resolver::resolve(&summary, &method, &replace,
                                              registry));
    if let Some(previous) = previous {
        resolved.copy_metadata(previous);
    }
    return Ok(resolved);

    fn keep<'a>(p: &&'a PackageId,
                to_avoid_packages: Option<&HashSet<&'a PackageId>>,
                to_avoid_sources: &HashSet<&'a SourceId>)
                -> bool {
        !to_avoid_sources.contains(&p.source_id()) && match to_avoid_packages {
            Some(set) => !set.contains(p),
            None => true,
        }
    }
}
