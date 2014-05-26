#![crate_id="cargo-compile"]
#![allow(deprecated_owned_vector)]
#![feature(phase)]

extern crate cargo;
extern crate hammer;
extern crate serialize;

#[phase(syntax, link)]
extern crate log;

use std::os;
use hammer::FlagConfig;
use cargo::{execute_main_without_stdin,CLIResult,CLIError,ToResult};
use cargo::ops;
use cargo::util::important_paths::find_project;
use cargo::util::ToCLI;

#[deriving(Eq,Clone,Decodable,Encodable)]
pub struct Options {
    manifest_path: Option<String>
}

impl FlagConfig for Options {}

fn main() {
    execute_main_without_stdin(execute);
}

fn execute(options: Options) -> CLIResult<Option<()>> {
    debug!("executing; cmd=cargo-compile; args={}", os::args());

    let root = match options.manifest_path {
        Some(path) => Path::new(path),
        None => try!(find_project(os::getcwd(), "Cargo.toml")
                    .map(|path| path.join("Cargo.toml"))
                    .to_result(|err|
                        CLIError::new("Could not find Cargo.toml in this directory or any parent directory", Some(err), 102)))
    };

    ops::compile(root.as_str().unwrap().as_slice()).map(|_| None).to_cli(101)
}
