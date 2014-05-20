#![crate_id="cargo-read-manifest"]
#![allow(deprecated_owned_vector)]

extern crate cargo;
extern crate serialize;
extern crate hammer;

use hammer::FlagConfig;
use cargo::{execute_main_without_stdin,CLIResult,CLIError};
use cargo::core::Package;
use cargo::ops::cargo_read_manifest::read_manifest;

#[deriving(Eq,Clone,Decodable)]
struct Options {
    manifest_path: StrBuf
}

impl FlagConfig for Options {}

fn main() {
    execute_main_without_stdin(execute);
}

fn execute(options: Options) -> CLIResult<Option<Package>> {
    read_manifest(options.manifest_path.as_slice()).map(|m| Some(m))
        .map_err(|err| CLIError {
            msg: err.get_desc().to_strbuf(),
            detail: err.get_detail().map(|s| s.to_strbuf()),
            exit_code: 1
        })
}
