use std::collections::HashSet;

use core::{PackageId, PackageIdSpec, SourceId, Workspace};
use core::registry::PackageRegistry;
use core::resolver::{self, Resolve, Method};
use ops;
use util::CargoResult;

/// Resolve all dependencies for the specified `package` using the previous
/// lockfile as a guide if present.
///
/// This function will also write the result of resolution as a new
/// lockfile.
pub fn resolve_ws(registry: &mut PackageRegistry, ws: &Workspace)
                   -> CargoResult<Resolve> {
    let prev = try!(ops::load_pkg_lockfile(ws));
    let resolve = try!(resolve_with_previous(registry, ws,
                                             Method::Everything,
                                             prev.as_ref(), None, &[]));

    // Avoid writing a lockfile if we are `cargo install`ing a non local package.
    if ws.current_opt().map(|pkg| pkg.package_id().source_id().is_path()).unwrap_or(true) {
        try!(ops::write_pkg_lockfile(ws, &resolve));
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
                                 ws: &Workspace,
                                 method: Method,
                                 previous: Option<&'a Resolve>,
                                 to_avoid: Option<&HashSet<&'a PackageId>>,
                                 specs: &[PackageIdSpec])
                                 -> CargoResult<Resolve> {
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
    if let Some(r) = previous {
        for node in r.iter().filter(|p| keep(p, to_avoid, &to_avoid_sources)) {
            let deps = r.deps_not_replaced(node)
                        .filter(|p| keep(p, to_avoid, &to_avoid_sources))
                        .cloned().collect();
            registry.register_lock(node.clone(), deps);
        }
    }

    let mut summaries = Vec::new();
    for member in ws.members() {
        try!(registry.add_sources(&[member.package_id().source_id()
                                          .clone()]));

        // If we're resolving everything then we include all members of the
        // workspace. If we want a specific set of requirements and we're
        // compiling only a single workspace crate then resolve only it. This
        // case should only happen after we have a previous resolution, however,
        // so assert that the previous exists.
        if let Method::Required { .. } = method {
            assert!(previous.is_some());
            if let Some(current) = ws.current_opt() {
                if member.package_id() != current.package_id() &&
                   !specs.iter().any(|spec| spec.matches(member.package_id())) {
                    continue;
                }
            }
        }

        let summary = registry.lock(member.summary().clone());
        summaries.push((summary, method));
    }

    let root_replace = ws.root_replace();

    let replace = match previous {
        Some(r) => {
            root_replace.iter().map(|&(ref spec, ref dep)| {
                for (key, val) in r.replacements().iter() {
                    if spec.matches(key) &&
                       dep.matches_id(val) &&
                       keep(&val, to_avoid, &to_avoid_sources) {
                        return (spec.clone(), dep.clone().lock_to(val))
                    }
                }
                (spec.clone(), dep.clone())
            }).collect::<Vec<_>>()
        }
        None => root_replace.to_vec(),
    };

    let mut resolved = try!(resolver::resolve(&summaries, &replace, registry));
    if let Some(previous) = previous {
        try!(resolved.merge_from(previous));
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
