use std::collections::HashMap;

use semver::VersionReq;

use core::{MultiShell, Package};
use core::registry::PackageRegistry;
use core::resolver::{mod, Resolve};
use core::source::Source;
use ops;
use sources::PathSource;
use util::{CargoResult, Config};
use util::profile;

/// Resolve all dependencies for the specified `package` using the previous
/// lockfile as a guide if present.
///
/// This function will also generate a write the result of resolution as a new
/// lockfile.
pub fn resolve_pkg(registry: &mut PackageRegistry, package: &Package)
                   -> CargoResult<Resolve> {
    let prev = try!(ops::load_pkg_lockfile(package));
    let resolve = try!(resolve_with_previous(registry, package, prev.as_ref()));
    try!(ops::write_pkg_lockfile(package, &resolve));
    Ok(resolve)
}

/// Resolve all dependencies for a package using an optional prevoius instance
/// of resolve to guide the resolution process.
///
/// The previous resolve normally comes from a lockfile. This function does not
/// read or write lockfiles from the filesystem.
pub fn resolve_with_previous(registry: &mut PackageRegistry,
                             package: &Package,
                             previous: Option<&Resolve>)
                             -> CargoResult<Resolve> {
    let root = package.get_package_id().get_source_id().clone();
    try!(registry.add_sources(&[root]));

    match previous {
        Some(r) => {
            let v = r.iter().map(|p| p.get_source_id().clone())
                     .collect::<Vec<_>>();
            try!(registry.add_sources(v.as_slice()));
        }
        None => {}
    };

    let mut resolved = try!(resolver::resolve(package.get_summary(),
                                              resolver::ResolveEverything,
                                              registry));
    match previous {
        Some(r) => resolved.copy_metadata(previous),
        None => {}
    }
    Ok(resolved)
}
