use std::env;
use std::error::Error;

use cargo::core::{Source};
use cargo::core::registry::PackageRegistry;
use cargo::core::resolver::Method;
use cargo::util::{CliResult, CliError, Config};
use cargo::sources::{PathSource};
use cargo::util::important_paths::{find_root_manifest_for_cwd};
use cargo::ops;

#[derive(RustcDecodable)]
struct Options {
    flag_manifest_path: Option<String>,
    flag_color: Option<String>,
}

pub const USAGE: &'static str = "
Output installed locations of all dependencies.

Usage:
    cargo print-source-root [options]

Options:
    -h, --help               Print this message
    --manifest-path PATH     Path to the manifest to compile
    --color WHEN             Coloring: auto, always, never
";

#[allow(deprecated)] // connect => join in 1.3
pub fn execute(options: Options, config: &Config) -> CliResult<Option<()>> {
    debug!("executing; cmd=cargo-print-source-root; args={:?}",
           env::args().collect::<Vec<_>>());
    try!(config.shell().set_color_config(options.flag_color.as_ref().map(|s| &s[..])));

    let root = try!(find_root_manifest_for_cwd(options.flag_manifest_path));

    let mut source = try!(PathSource::for_path(root.parent().unwrap(), config).map_err(|e| {
        CliError::new(e.description(), 1)
    }));

    try!(source.update().map_err(|err| CliError::new(err.description(), 1)));

    let package = try!(source.root_package());
    let source_id = package.package_id().source_id();

    let override_ids = try!(ops::source_ids_from_config(config, package.root()));

    let mut registry = PackageRegistry::new(config);
    registry.preload(source_id, Box::new(source));

    try!(registry.add_overrides(override_ids));

    let lockfile = package.root().join("Cargo.lock");
    let lock_resolve = match try!(ops::load_lockfile(&lockfile, source_id)) {
        Some(resolve) => resolve,
        None => return Err(CliError::new("A Cargo.lock must exist for this command", 1))
    };
    let resolve = try!(ops::resolve_with_previous(&mut registry, &package,
                                                  Method::Everything,
                                                  Some(&lock_resolve), None));

    let pkgids = resolve.iter().map(|pkgid| pkgid.clone()).collect::<Vec<_>>();
    let mut result = try!(registry.get(&pkgids[..]));
    result.sort_by(|a, b| a.name().cmp(b.name()));
    let roots = result.iter()
        .map(|pkg| {
            format!("{} = \"{}\"", pkg.name(), pkg.root().display())
        }).collect::<Vec<_>>().connect("\n");
    println!("{}", roots);

    Ok(None)
}
