use std::io::File;

use serialize::Encodable;
use toml::{mod, Encoder};

use core::registry::PackageRegistry;
use core::{MultiShell, Source, Resolve, resolver, Package};
use sources::{PathSource};
use util::config::{Config};
use util::{CargoResult};

pub fn generate_lockfile(manifest_path: &Path,
                         shell: &mut MultiShell,
                         update: bool)
                         -> CargoResult<()> {

    log!(4, "compile; manifest-path={}", manifest_path.display());

    let mut source = PathSource::for_path(&manifest_path.dir_path());
    try!(source.update());

    // TODO: Move this into PathSource
    let package = try!(source.get_root_package());
    debug!("loaded package; package={}", package);

    let source_ids = package.get_source_ids();

    let resolve = {
        let mut config = try!(Config::new(shell, update, None, None));

        let mut registry = PackageRegistry::new(&mut config);
        try!(registry.add_sources(source_ids));
        try!(resolver::resolve(package.get_package_id(),
                               package.get_dependencies(),
                               &mut registry))
    };

    try!(write_resolve(&package, &resolve));
    Ok(())
}

pub fn write_resolve(pkg: &Package, resolve: &Resolve) -> CargoResult<()> {
    let mut e = Encoder::new();
    resolve.encode(&mut e).unwrap();

    let out = toml::Table(e.toml).to_string();
    let loc = pkg.get_root().join("Cargo.lock");
    try!(File::create(&loc).write_str(out.as_slice()));

    Ok(())
}
