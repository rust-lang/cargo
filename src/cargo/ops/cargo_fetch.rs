use std::collections::{HashSet, HashMap};

use core::{MultiShell, Package, PackageId, Summary};
use core::registry::PackageRegistry;
use core::resolver::{mod, Resolve};
use core::source::Source;
use ops;
use sources::PathSource;
use util::{CargoResult, Config};
use util::profile;

pub fn fetch(manifest_path: &Path,
             shell: &mut MultiShell) -> CargoResult<()> {
    let mut source = try!(PathSource::for_path(&manifest_path.dir_path()));
    try!(source.update());
    let package = try!(source.get_root_package());

    let mut config = try!(Config::new(shell, None, None));
    let mut registry = PackageRegistry::new(&mut config);
    try!(resolve_and_fetch(&mut registry, &package));
    Ok(())
}

pub fn resolve_and_fetch(registry: &mut PackageRegistry, package: &Package)
                         -> CargoResult<Resolve> {
    let _p = profile::start("resolve and fetch...");

    let lockfile = package.get_manifest_path().dir_path().join("Cargo.lock");
    let source_id = package.get_package_id().get_source_id();
    match try!(ops::load_lockfile(&lockfile, source_id)) {
        Some(r) => try!(add_lockfile_sources(registry, package, &r)),
        None => try!(registry.add_sources(package.get_source_ids())),
    }

    let resolved = try!(resolver::resolve(package.get_summary(),
                                          resolver::ResolveEverything,
                                          registry));
    try!(ops::write_resolve(package, &resolved));
    Ok(resolved)
}

/// When a lockfile is present, we want to keep as many dependencies at their
/// original revision as possible. We need to account, however, for
/// modifications to the manifest in terms of modifying, adding, or deleting
/// dependencies.
///
/// This method will add any appropriate sources from the lockfile into the
/// registry, and add all other sources from the root package to the registry.
/// Any dependency which has not been modified has its source added to the
/// registry (to retain the precise field if possible). Any dependency which
/// *has* changed has its source id listed in the manifest added and all of its
/// transitive dependencies are blacklisted to not be added from the lockfile.
///
/// TODO: this won't work too well for registry-based packages, but we don't
///       have many of those anyway so we should be ok for now.
fn add_lockfile_sources(registry: &mut PackageRegistry,
                        root: &Package,
                        resolve: &Resolve) -> CargoResult<()> {
    let deps = resolve.deps(root.get_package_id()).into_iter().flat_map(|deps| {
        deps.map(|d| (d.get_name(), d))
    }).collect::<HashMap<_, &PackageId>>();

    let mut sources = vec![root.get_package_id().get_source_id().clone()];
    let mut to_avoid = HashSet::new();
    let mut to_add = HashSet::new();
    for dep in root.get_dependencies().iter() {
        match deps.find(&dep.get_name()) {
            Some(&lockfile_dep) => {
                let summary = Summary::new(lockfile_dep.clone(), Vec::new(),
                                           HashMap::new()).unwrap();
                if dep.matches(&summary) {
                    fill_with_deps(resolve, lockfile_dep, &mut to_add);
                } else {
                    fill_with_deps(resolve, lockfile_dep, &mut to_avoid);
                    sources.push(dep.get_source_id().clone());
                }
            }
            None => sources.push(dep.get_source_id().clone()),
        }
    }

    // Only afterward once we know the entire blacklist are the lockfile
    // sources added.
    for addition in to_add.iter() {
        if !to_avoid.contains(addition) {
            sources.push(addition.get_source_id().clone());
        }
    }

    return registry.add_sources(sources);

    fn fill_with_deps<'a>(resolve: &'a Resolve, dep: &'a PackageId,
                          set: &mut HashSet<&'a PackageId>) {
        if !set.insert(dep) { return }
        for mut deps in resolve.deps(dep).into_iter() {
            for dep in deps {
                fill_with_deps(resolve, dep, set);
            }
        }
    }
}
