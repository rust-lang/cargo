use std::collections::HashSet;
use std::path::Path;

use core::PackageId;
use core::registry::PackageRegistry;
use core::{Source, Resolve};
use core::resolver::Method;
use ops;
use sources::{PathSource};
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
    let mut source = try!(PathSource::for_path(manifest_path.parent().unwrap(),
                                               config));
    try!(source.update());
    let package = try!(source.root_package());
    let mut registry = PackageRegistry::new(config);
    registry.preload(package.package_id().source_id(), Box::new(source));
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

    registry.preload(package.package_id().source_id(), Box::new(source));
    let resolve = try!(ops::resolve_with_previous(&mut registry,
                                                  &package,
                                                  Method::Everything,
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
            Some(deps) => {
                for dep in deps {
                    fill_with_deps(resolve, dep, set, visited);
                }
            }
            None => {}
        }
    }
}
