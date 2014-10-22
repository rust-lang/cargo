use core::{MultiShell, Package};
use core::registry::PackageRegistry;
use core::resolver::{mod, Resolve};
use core::source::Source;
use ops;
use sources::PathSource;
use util::{CargoResult, Config};
use util::profile;

/// Executes `cargo fetch`.
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

/// Finds all the packages required to compile the specified `Package`,
/// and loads them in the `PackageRegistry`.
///
/// Also write the `Cargo.lock` file with the results.
pub fn resolve_and_fetch(registry: &mut PackageRegistry, package: &Package)
                         -> CargoResult<Resolve> {
    let _p = profile::start("resolve and fetch...");

    let lockfile = package.get_manifest_path().dir_path().join("Cargo.lock");
    let source_id = package.get_package_id().get_source_id();
    let previous_resolve = try!(ops::load_lockfile(&lockfile, source_id));
    match previous_resolve {
        Some(ref r) => r.iter().map(|p| p.get_source_id().clone()).collect(),
        None => package.get_source_ids(),
    };
    try!(registry.add_sources(sources));

    let mut resolved = try!(resolver::resolve(package.get_summary(),
                                              resolver::ResolveEverything,
                                              registry));
    match previous_resolve {
        Some(ref prev) => resolved.copy_metadata(prev),
        None => {}
    }
    try!(ops::write_resolve(package, &resolved));
    Ok(resolved)
}
