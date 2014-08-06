#![feature(phase)]

extern crate serialize;
extern crate cargo;
extern crate docopt;
#[phase(plugin)] extern crate docopt_macros;

use cargo::{execute_main_without_stdin};
use cargo::core::{MultiShell, Package, Source};
use cargo::util::{CliResult, CliError};
use cargo::sources::{PathSource};

docopt!(Options, "
Usage:
    cargo-clean [options] --manifest-path=PATH

Options:
    -h, --help              Print this message
    -v, --verbose           Use verbose output
")

fn main() {
    execute_main_without_stdin(execute, false);
}

fn execute(options: Options, _: &mut MultiShell) -> CliResult<Option<Package>> {
    let path = Path::new(options.flag_manifest_path.as_slice());
    let mut source = try!(PathSource::for_path(&path).map_err(|e| {
        CliError::new(e.description(), 1)
    }));

    try!(source.update().map_err(|err| CliError::new(err.description(), 1)));

    source
        .get_root_package()
        .map(|pkg| Some(pkg))
        .map_err(|err| CliError::from_boxed(err, 1))
}
