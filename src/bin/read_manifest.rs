use cargo::core::{Package, Source};
use cargo::util::{CliResult, Config};
use cargo::sources::{PathSource};

#[derive(RustcDecodable)]
struct Options {
    flag_manifest_path: String,
}

pub const USAGE: &'static str = "
Usage:
    cargo read-manifest [options] --manifest-path=PATH
    cargo read-manifest -h | --help

Options:
    -h, --help              Print this message
    -v, --verbose           Use verbose output
";

pub fn execute(options: Options, config: &Config) -> CliResult<Option<Package>> {
    let path = Path::new(options.flag_manifest_path.as_slice());
    let mut source = try!(PathSource::for_path(&path, config));
    try!(source.update());
    let pkg = try!(source.get_root_package());
    Ok(Some(pkg))
}
