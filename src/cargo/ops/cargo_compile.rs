/**
 * Cargo compile currently does the following steps:
 *
 * All configurations are already injected as environment variables via the main cargo command
 *
 * 1. Read the manifest
 * 2. Shell out to `cargo-resolve` with a list of dependencies and sources as stdin
 *    a. Shell out to `--do update` and `--do list` for each source
 *    b. Resolve dependencies and return a list of name/version/source
 * 3. Shell out to `--do download` for each source
 * 4. Shell out to `--do get` for each source, and build up the list of paths to pass to rustc -L
 * 5. Call `cargo-rustc` with the results of the resolver zipped together with the results of the `get`
 *    a. Topologically sort the dependencies
 *    b. Compile each dependency in order, passing in the -L's pointing at each previously compiled dependency
 */

use std::vec::Vec;
use std::os;
use util::config;
use util::config::{all_configs,ConfigValue};
use core::resolver::resolve;
use core::package::PackageSet;
use core::source::Source;
use core::dependency::Dependency;
use sources::path::PathSource;
use ops::cargo_rustc;
use core::errors::{CargoError,CLIError,CLIResult,ToResult};

pub fn compile(manifest_path: &str) -> CLIResult<()> {
    let configs = try!(all_configs(os::getcwd()).to_result(|err: CargoError|
        CLIError::new("Could not load configurations", Some(err.to_str()), 1)));

    let config_paths = configs.find(&("paths".to_owned())).map(|v| v.clone()).unwrap_or_else(|| ConfigValue::new());

    let paths = match config_paths.get_value() {
        &config::String(_) => return Err(CLIError::new("The path was configured as a String instead of a List", None, 1)),
        &config::List(ref list) => list.iter().map(|path| Path::new(path.as_slice())).collect()
    };

    let source = PathSource::new(paths);
    let names = try!(source.list().to_result(|err| CLIError::new(format!("Unable to list packages from {}", source), Some(err.to_str()), 1)));
    try!(source.download(names.as_slice()).to_result(|err| CLIError::new(format!("Unable to download packages from {}", source), Some(err.to_str()), 1)));

    let deps: Vec<Dependency> = names.iter().map(|namever| {
        Dependency::with_namever(namever)
    }).collect();

    let packages = try!(source.get(names.as_slice()).to_result(|err|
        CLIError::new(format!("Unable to get packages from {} for {}", source, names), Some(err.to_str()), 1)));

    let registry = PackageSet::new(packages.as_slice());

    let resolved = try!(resolve(deps.as_slice(), &registry).to_result(|err: CargoError|
        CLIError::new("Unable to resolve dependencies", Some(err.to_str()), 1)));

    try!(cargo_rustc::compile(&resolved));

    Ok(())
}
