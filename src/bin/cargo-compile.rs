#![crate_id="cargo-compile"]
#![allow(deprecated_owned_vector)]

extern crate cargo;
extern crate hammer;
extern crate serialize;

use cargo::{execute_main_without_stdin,CLIResult,CLIError,ToResult};
use cargo::ops::cargo_compile::compile;
use cargo::util::important_paths::find_project;
use cargo::util::ToCLI;
use hammer::FlagConfig;
use std::os;

#[deriving(Eq,Clone,Decodable,Encodable)]
pub struct Options {
    manifest_path: Option<StrBuf>
}

impl FlagConfig for Options {}

fn main() {
    execute_main_without_stdin(execute);
}

fn execute(options: Options) -> CLIResult<Option<()>> {
    let root = match options.manifest_path {
        Some(path) => Path::new(path),
        None => try!(find_project(os::getcwd(), "Cargo.toml".to_owned())
                    .map(|path| path.join("Cargo.toml"))
                    .to_result(|err|
                        CLIError::new("Could not find Cargo.toml in this directory or any parent directory", Some(err), 102)))
    };

    compile(root.as_str().unwrap().as_slice()).map(|_| None).to_cli(101)
}
