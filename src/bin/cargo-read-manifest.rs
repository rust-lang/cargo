#![crate_id="cargo-read-manifest"]

extern crate cargo;
extern crate serialize;
extern crate hammer;

use hammer::FlagConfig;
use cargo::{execute_main_without_stdin,CLIResult,CLIError};
use cargo::core::{Package,SourceId};
use cargo::sources::{PathSource};

#[deriving(PartialEq,Clone,Decodable)]
struct Options {
    manifest_path: String
}

impl FlagConfig for Options {}

fn main() {
    execute_main_without_stdin(execute);
}

fn execute(options: Options) -> CLIResult<Option<Package>> {
    let source_id = SourceId::for_path(&Path::new(options.manifest_path.as_slice()));

    PathSource::new(&source_id)
        .get_root_package()
        .map(|pkg| Some(pkg))
        .map_err(|err| CLIError::new(err.get_desc(), Some(err.get_detail()), 1))
}
