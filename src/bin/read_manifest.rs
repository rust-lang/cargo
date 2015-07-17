use std::path::Path;
use std::error::Error;

use cargo::core::{Package, Source};
use cargo::util::{CliResult, CliError, Config};
use cargo::sources::{PathSource};

#[derive(RustcDecodable)]
struct Options {
    flag_manifest_path: String,
    flag_color: Option<String>,
}

pub const USAGE: &'static str = "
Usage:
    cargo read-manifest [options] --manifest-path=PATH
    cargo read-manifest -h | --help

Options:
    -h, --help               Print this message
    -v, --verbose            Use verbose output
    --color WHEN             Coloring: auto, always, never
";

pub fn execute(options: Options, config: &Config) -> CliResult<Option<Package>> {
    try!(config.shell().set_color_config(options.flag_color.as_ref().map(|s| &s[..])));
    let path = Path::new(&options.flag_manifest_path);
    let mut source = try!(PathSource::for_path(&path, config).map_err(|e| {
        CliError::new(e.description(), 1)
    }));

    try!(source.update().map_err(|err| CliError::new(err.description(), 1)));

    source.root_package()
          .map(|pkg| Some(pkg))
          .map_err(|err| CliError::from_boxed(err, 1))
}
