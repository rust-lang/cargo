#![crate_id="cargo-read-manifest"]
#![allow(deprecated_owned_vector)]

extern crate cargo;
extern crate serialize;
extern crate hammer;

use hammer::FlagConfig;
use cargo::{execute_main_without_stdin,CLIResult};
use cargo::core::Package;
use cargo::ops::cargo_read_manifest::read_manifest;

#[deriving(Eq,Clone,Decodable)]
struct Options {
    manifest_path: ~str
}

impl FlagConfig for Options {}

fn main() {
    execute_main_without_stdin(execute);
}

fn execute(options: Options) -> CLIResult<Option<Package>> {
    read_manifest(options.manifest_path.as_slice()).map(|m| Some(m))
}
