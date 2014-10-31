use std::collections::HashSet;

use core::PackageId;
use core::registry::PackageRegistry;
use core::{MultiShell, Source, Resolve, resolver};
use ops;
use sources::{PathSource};
use util::config::{Config};
use util::{CargoResult, human};

pub struct UpdateOptions<'a> {
    pub shell: &'a mut MultiShell,
    pub to_update: Option<&'a str>,
    pub precise: Option<&'a str>,
    pub aggressive: bool,
}

pub fn generate_lockfile(manifest_path: &Path,
                         shell: &mut MultiShell)
                         -> CargoResult<()> {
    let mut source = try!(PathSource::for_path(&manifest_path.dir_path()));
    try!(source.update());
    let package = try!(source.get_root_package());
    let mut config = try!(Config::new(shell, None, None));
    let mut registry = PackageRegistry::new(&mut config);
    let resolve = try!(ops::resolve_with_previous(&mut registry, &package,
                                                  resolver::ResolveEverything,
                                                  None, None));
    try!(ops::write_pkg_lockfile(&package, &resolve));
    Ok(())
}

pub fn update_lockfile(manifest_path: &Path,
                       opts: &mut UpdateOptions) -> CargoResult<()> {
    let mut source = try!(PathSource::for_path(&manifest_path.dir_path()));
    try!(source.update());
    let package = try!(source.get_root_package());

    let previous_resolve = match try!(ops::load_pkg_lockfile(&package)) {
        Some(resolve) => resolve,
        None => return Err(human("A Cargo.lock must exist before it is updated"))
    };

    if opts.aggressive && opts.precise.is_some() {
        return Err(human("cannot specify both aggressive and precise \
                          simultaneously"))
    }

    let mut config = try!(Config::new(opts.shell, None, None));
    let mut registry = PackageRegistry::new(&mut config);
    let mut to_avoid = HashSet::new();

    match opts.to_update {
        Some(name) => {
            let dep = try!(previous_resolve.query(name));
            if opts.aggressive {
                fill_with_deps(&previous_resolve, dep, &mut to_avoid,
                               &mut HashSet::new());
            } else {
                to_avoid.insert(dep);
                match opts.precise {
                    Some(precise) => {
                        let precise = dep.get_source_id().clone()
                                         .with_precise(Some(precise.to_string()));
                        try!(registry.add_sources(&[precise]));
                    }
                    None => {}
                }
            }
        }
        None => to_avoid.extend(previous_resolve.iter()),
    }

    let resolve = try!(ops::resolve_with_previous(&mut registry,
                                                  &package,
                                                  resolver::ResolveEverything,
                                                  Some(&previous_resolve),
                                                  Some(&to_avoid)));
    try!(ops::write_pkg_lockfile(&package, &resolve));
    return Ok(());

    fn fill_with_deps<'a>(resolve: &'a Resolve, dep: &'a PackageId,
                          set: &mut HashSet<&'a PackageId>,
                          visited: &mut HashSet<&'a PackageId>) {
        if !visited.insert(dep) { return }
        set.insert(dep);
        match resolve.deps(dep) {
            Some(mut deps) => {
                for dep in deps {
                    fill_with_deps(resolve, dep, set, visited);
                }
            }
            None => {}
        }
    }
}
