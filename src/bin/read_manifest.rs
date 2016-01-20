use std::env;

use cargo::core::{Package, Source};
use cargo::util::{CliResult, Config};
use cargo::util::important_paths::{find_root_manifest_for_wd};
use cargo::sources::{PathSource};

#[derive(RustcDecodable)]
pub struct Options {
    flag_manifest_path: Option<String>,
    flag_color: Option<String>,
}

pub const USAGE: &'static str = "
Print a JSON representation of a Cargo.toml manifest

Usage:
    cargo read-manifest [options]
    cargo read-manifest -h | --help

Options:
    -h, --help               Print this message
    -v, --verbose            Use verbose output
    --manifest-path PATH     Path to the manifest
    --color WHEN             Coloring: auto, always, never
";

pub fn execute(options: Options, config: &Config) -> CliResult<Option<Package>> {
    debug!("executing; cmd=cargo-read-manifest; args={:?}",
           env::args().collect::<Vec<_>>());
    try!(config.shell().set_color_config(options.flag_color.as_ref().map(|s| &s[..])));

    let root = try!(find_root_manifest_for_wd(options.flag_manifest_path, config.cwd()));

    let mut source = try!(PathSource::for_path(root.parent().unwrap(), config));
    try!(source.update());

    let pkg = try!(source.root_package());
    Ok(Some(pkg))
}
