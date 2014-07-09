#![crate_name="cargo-clean"]
#![feature(phase)]

extern crate cargo;

#[phase(plugin, link)]
extern crate hammer;

#[phase(plugin, link)]
extern crate log;

extern crate serialize;

use std::os;
use cargo::ops;
use cargo::{execute_main_without_stdin};
use cargo::core::MultiShell;
use cargo::util::{CliResult, CliError};
use cargo::util::important_paths::find_project_manifest;

#[deriving(PartialEq,Clone,Decodable,Encodable)]
pub struct Options {
    manifest_path: Option<String>
}

hammer_config!(Options)

fn main() {
    execute_main_without_stdin(execute);
}

fn execute(options: Options, _shell: &mut MultiShell) -> CliResult<Option<()>> {
    debug!("executing; cmd=cargo-clean; args={}", os::args());

    let root = match options.manifest_path {
        Some(path) => Path::new(path),
        None => try!(find_project_manifest(&os::getcwd(), "Cargo.toml")
                    .map_err(|_| {
                        CliError::new("Could not find Cargo.toml in this \
                                       directory or any parent directory",
                                      102)
                    }))
    };

    ops::clean(&root).map(|_| None).map_err(|err| {
      CliError::from_boxed(err, 101)
    })
}
