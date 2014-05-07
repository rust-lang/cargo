#![crate_id="cargo-compile"]
#![allow(deprecated_owned_vector)]

extern crate cargo;
extern crate hammer;
extern crate serialize;

use cargo::ops::cargo_compile::compile;
use cargo::core::errors::{CLIResult,CLIError,ToResult};
use cargo::util::important_paths::find_project;
use hammer::{FlagDecoder,FlagConfig,HammerError};
use serialize::Decodable;
use std::os;

#[deriving(Eq,Clone,Decodable,Encodable)]
pub struct Options {
    manifest_path: Option<~str>
}

impl FlagConfig for Options {}

fn flags<T: FlagConfig + Decodable<FlagDecoder, HammerError>>() -> CLIResult<T> {
    let mut decoder = FlagDecoder::new::<T>(std::os::args().tail());
    Decodable::decode(&mut decoder).to_result(|e: HammerError| CLIError::new(e.message, None, 1))
}

fn execute() -> CLIResult<()> {
    let options = try!(flags::<Options>());

    let root = match options.manifest_path {
        Some(path) => Path::new(path),
        None => try!(find_project(os::getcwd(), "Cargo.toml".to_owned())
                    .map(|path| path.join("Cargo.toml"))
                    .to_result(|err|
                        CLIError::new("Could not find Cargo.toml in this directory or any parent directory", Some(err.to_str()), 1)))
    };

    compile(root.as_str().unwrap().as_slice()).to_result(|err|
        CLIError::new(format!("Compilation failed: {}", err), None, 1))
}

fn main() {
    match execute() {
        Err(err) => fail!("{}", err),
        Ok(_) => return
    }
}
