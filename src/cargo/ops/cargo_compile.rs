//!
//! Cargo compile currently does the following steps:
//!
//! All configurations are already injected as environment variables via the
//! main cargo command
//!
//! 1. Read the manifest
//! 2. Shell out to `cargo-resolve` with a list of dependencies and sources as
//!    stdin
//!
//!    a. Shell out to `--do update` and `--do list` for each source
//!    b. Resolve dependencies and return a list of name/version/source
//!
//! 3. Shell out to `--do download` for each source
//! 4. Shell out to `--do get` for each source, and build up the list of paths
//!    to pass to rustc -L
//! 5. Call `cargo-rustc` with the results of the resolver zipped together with
//!    the results of the `get`
//!
//!    a. Topologically sort the dependencies
//!    b. Compile each dependency in order, passing in the -L's pointing at each
//!       previously compiled dependency
//!

use std::os;
use util::config::{ConfigValue};
use core::{MultiShell, Source, SourceId, PackageSet, resolver};
use core::registry::PackageRegistry;
use ops;
use sources::{PathSource};
use util::{CargoResult, Wrap, config, internal, human};

pub fn compile(manifest_path: &Path, shell: &mut MultiShell) -> CargoResult<()> {
    log!(4, "compile; manifest-path={}", manifest_path.display());

    let id = SourceId::for_path(&manifest_path.dir_path());
    let mut source = PathSource::new(&id);

    try!(source.update());

    // TODO: Move this into PathSource
    let package = try!(source.get_root_package());
    debug!("loaded package; package={}", package);

    let override_ids = try!(source_ids_from_config());
    let source_ids = package.get_source_ids();

    let packages = {
        let mut registry =
            try!(PackageRegistry::new(source_ids, override_ids, shell));

        let resolved =
            try!(resolver::resolve(package.get_dependencies(), &mut registry));

        try!(registry.get(resolved.as_slice()).wrap({
            human("Unable to get packages from source")
        }))
    };

    debug!("packages={}", packages);

    try!(ops::compile_packages(&package, &PackageSet::new(packages.as_slice()), shell));

    Ok(())
}

fn source_ids_from_config() -> CargoResult<Vec<SourceId>> {
    let configs = try!(config::all_configs(os::getcwd()));

    debug!("loaded config; configs={}", configs);

    let config_paths = configs.find_equiv(&"paths").map(|v| v.clone());
    let config_paths = config_paths.unwrap_or_else(|| ConfigValue::new());

    let paths: Vec<Path> = match *config_paths.get_value() {
        config::String(_) => return Err(internal("The path was configured as \
                                                   a String instead of a List")),
        config::List(ref list) => {
            list.iter().map(|path| Path::new(path.as_slice())).collect()
        }
    };

    Ok(paths.iter().map(|p| SourceId::for_path(p)).collect())
}
