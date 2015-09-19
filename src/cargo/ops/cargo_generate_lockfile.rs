use std::collections::{HashMap, HashSet};
use std::path::Path;

use core::PackageId;
use core::registry::PackageRegistry;
use core::{Resolve, SourceId, Package};
use core::resolver::Method;
use ops;
use util::config::{Config};
use util::{CargoResult, human};

pub struct UpdateOptions<'a> {
    pub config: &'a Config,
    pub to_update: Option<&'a str>,
    pub precise: Option<&'a str>,
    pub aggressive: bool,
}

pub fn generate_lockfile(manifest_path: &Path, config: &Config)
                         -> CargoResult<()> {
    let package = try!(Package::for_path(manifest_path, config));
    let mut registry = PackageRegistry::new(config);
    let resolve = try!(ops::resolve_with_previous(&mut registry, &package,
                                                  Method::Everything,
                                                  None, None));
    try!(ops::write_pkg_lockfile(&package, &resolve));
    Ok(())
}

pub fn update_lockfile(manifest_path: &Path,
                       opts: &UpdateOptions) -> CargoResult<()> {
    let package = try!(Package::for_path(manifest_path, opts.config));

    let previous_resolve = match try!(ops::load_pkg_lockfile(&package)) {
        Some(resolve) => resolve,
        None => return Err(human("A Cargo.lock must exist before it is updated"))
    };

    if opts.aggressive && opts.precise.is_some() {
        return Err(human("cannot specify both aggressive and precise \
                          simultaneously"))
    }

    let mut registry = PackageRegistry::new(opts.config);
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
                        // TODO: see comment in `resolve.rs` as well, but this
                        //       seems like a pretty hokey reason to single out
                        //       the registry as well.
                        let precise = if dep.source_id().is_registry() {
                            format!("{}={}", dep.name(), precise)
                        } else {
                            precise.to_string()
                        };
                        let precise = dep.source_id().clone()
                                         .with_precise(Some(precise));
                        try!(registry.add_sources(&[precise]));
                    }
                    None => {
                        let imprecise = dep.source_id().clone()
                                           .with_precise(None);
                        try!(registry.add_sources(&[imprecise]));
                    }
                }
            }
        }
        None => to_avoid.extend(previous_resolve.iter()),
    }

    let resolve = try!(ops::resolve_with_previous(&mut registry,
                                                  &package,
                                                  Method::Everything,
                                                  Some(&previous_resolve),
                                                  Some(&to_avoid)));

    // Summarize what is changing for the user.
    let print_change = |status: &str, msg: String| {
        opts.config.shell().status(status, msg)
    };
    for (removed, added) in compare_dependency_graphs(&previous_resolve, &resolve) {
        if removed.len() == 1 && added.len() == 1 {
            if removed[0].source_id().is_git() {
                try!(print_change("Updating", format!("{} -> #{}",
                    removed[0],
                    &added[0].source_id().precise().unwrap()[..8])));
            } else {
                try!(print_change("Updating", format!("{} -> v{}",
                    removed[0],
                    added[0].version())));
            }
        }
        else {
            for package in removed.iter() {
                try!(print_change("Removing", format!("{}", package)));
            }
            for package in added.iter() {
                try!(print_change("Adding", format!("{}", package)));
            }
        }
    }

    try!(ops::write_pkg_lockfile(&package, &resolve));
    return Ok(());

    fn fill_with_deps<'a>(resolve: &'a Resolve, dep: &'a PackageId,
                          set: &mut HashSet<&'a PackageId>,
                          visited: &mut HashSet<&'a PackageId>) {
        if !visited.insert(dep) { return }
        set.insert(dep);
        match resolve.deps(dep) {
            Some(deps) => {
                for dep in deps {
                    fill_with_deps(resolve, dep, set, visited);
                }
            }
            None => {}
        }
    }

    fn compare_dependency_graphs<'a>(previous_resolve: &'a Resolve,
                                     resolve: &'a Resolve) ->
                                     Vec<(Vec<&'a PackageId>, Vec<&'a PackageId>)> {
        // Map (package name, package source) to (removed versions, added versions).
        fn changes_key<'a>(dep: &'a PackageId) -> (&'a str, &'a SourceId) {
            (dep.name(), dep.source_id())
        }

        fn vec_subtract<T>(a: &[T], b: &[T]) -> Vec<T>
            where T: Ord + Clone {
            let mut result = a.to_owned();
            let mut b = b.to_owned();
            b.sort();
            result.retain(|x| b.binary_search(x).is_err());
            result
        }

        let mut changes = HashMap::new();

        for dep in previous_resolve.iter() {
            changes.insert(changes_key(dep), (vec![dep], vec![]));
        }
        for dep in resolve.iter() {
            let (_, ref mut added) = *changes.entry(changes_key(dep))
                                             .or_insert_with(|| (vec![], vec![]));
            added.push(dep);
        }

        for (_, v) in changes.iter_mut() {
            let (ref mut old, ref mut new) = *v;
            let removed = vec_subtract(old, new);
            let added = vec_subtract(new, old);
            *old = removed;
            *new = added;
        }

        // Sort the packages by their names.
        let mut packages: Vec<_> = changes.keys().map(|x| *x).collect();
        packages.sort();
        packages.iter().map(|k| changes[k].clone()).collect()
    }
}
