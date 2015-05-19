use std::collections::{HashMap, HashSet};
use std::path::Path;

use core::PackageId;
use core::registry::PackageRegistry;
use core::{Source, Resolve};
use core::resolver::Method;
use ops;
use sources::{PathSource};
use util::config::{Config};
use util::{CargoResult, human};

pub struct UpdateOptions<'a, 'b: 'a> {
    pub config: &'a Config<'b>,
    pub to_update: Option<&'a str>,
    pub precise: Option<&'a str>,
    pub aggressive: bool,
}

pub fn generate_lockfile(manifest_path: &Path, config: &Config)
                         -> CargoResult<()> {
    let mut source = try!(PathSource::for_path(manifest_path.parent().unwrap(),
                                               config));
    try!(source.update());
    let package = try!(source.root_package());
    let mut registry = PackageRegistry::new(config);
    let resolve = try!(ops::resolve_with_previous(&mut registry, &package,
                                                  Method::Everything,
                                                  None, None));
    try!(ops::write_pkg_lockfile(&package, &resolve));
    Ok(())
}

pub fn update_lockfile(manifest_path: &Path,
                       opts: &UpdateOptions) -> CargoResult<()> {
    let mut source = try!(PathSource::for_path(manifest_path.parent().unwrap(),
                                               opts.config));
    try!(source.update());
    let package = try!(source.root_package());

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
    for dep in compare_dependency_graphs(&previous_resolve, &resolve) {
        try!(match dep {
            (None, Some(pkg)) => opts.config.shell().status("Adding",
                format!("{} v{}", pkg.name(), pkg.version())),
            (Some(pkg), None) => opts.config.shell().status("Removing",
                format!("{} v{}", pkg.name(), pkg.version())),
            (Some(pkg1), Some(pkg2)) => {
                if pkg1.version() != pkg2.version() {
                    opts.config.shell().status("Updating",
                        format!("{} v{} -> v{}", pkg1.name(), pkg1.version(), pkg2.version()))
                } else {Ok(())}
            }
            (None, None) => unreachable!(),
        });
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
                                     Vec<(Option<&'a PackageId>, Option<&'a PackageId>)> {
        let mut changes = HashMap::new();
        for dep in previous_resolve.iter() {
            changes.insert(dep.name(), (Some(dep), None));
        }
        for dep in resolve.iter() {
            if !changes.contains_key(dep.name()) {
                changes.insert(dep.name(), (None, None));
            }
            let value = changes.get_mut(dep.name()).unwrap();
            value.1 = Some(dep);
        }
        let mut package_names: Vec<&str> = changes.keys().map(|x| *x).collect();
        package_names.sort();
        package_names.iter().map(|name| *changes.get(name).unwrap()).collect()
    }
}
