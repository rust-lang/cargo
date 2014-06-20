#![crate_id="cargo-compile"]
#![feature(phase)]

extern crate cargo;
extern crate hammer;
extern crate serialize;

#[phase(plugin, link)]
extern crate log;

use std::os;
use hammer::FlagConfig;
use cargo::{execute_main_without_stdin};
use cargo::ops;
use cargo::util::{CliResult, CliError};
use cargo::util::important_paths::find_project;

#[deriving(PartialEq,Clone,Decodable,Encodable)]
pub struct Options {
    manifest_path: Option<String>
}

impl FlagConfig for Options {}

fn main() {
    execute_main_without_stdin(execute);
}

fn execute(options: Options) -> CliResult<Option<()>> {
    debug!("executing; cmd=cargo-compile; args={}", os::args());

    let root = match options.manifest_path {
        Some(path) => Path::new(path),
        None => try!(find_project(os::getcwd(), "Cargo.toml")
                    .map(|path| path.join("Cargo.toml"))
                    .map_err(|_| {
                        CliError::new("Could not find Cargo.toml in this \
                                       directory or any parent directory",
                                      102)
                    }))
    };

    ops::compile(&root).map(|_| None).map_err(|err| {
        CliError::from_boxed(err, 101)
    })
}
