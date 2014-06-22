#![crate_id="cargo-read-manifest"]
#![feature(phase)]

extern crate cargo;
extern crate serialize;

#[phase(plugin, link)]
extern crate hammer;

use cargo::{execute_main_without_stdin};
use cargo::core::{MultiShell, Package, Source, SourceId};
use cargo::util::{CliResult, CliError};
use cargo::sources::{PathSource};

#[deriving(PartialEq,Clone,Decodable)]
struct Options {
    manifest_path: String
}

hammer_config!(Options)

fn main() {
    execute_main_without_stdin(execute);
}

fn execute(options: Options, _: &mut MultiShell) -> CliResult<Option<Package>> {
    let path = Path::new(options.manifest_path.as_slice());
    let source_id = SourceId::for_path(&path);
    let mut source = PathSource::new(&source_id);

    try!(source.update().map_err(|err| CliError::new(err.description(), 1)));

    source
        .get_root_package()
        .map(|pkg| Some(pkg))
        .map_err(|err| CliError::from_boxed(err, 1))
}
