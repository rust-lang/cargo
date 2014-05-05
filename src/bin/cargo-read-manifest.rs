#![crate_id="cargo-read-manifest"]
#![allow(deprecated_owned_vector)]

extern crate cargo;
extern crate serialize;
extern crate hammer;

use cargo::{CargoResult,execute_main_without_stdin};
use cargo::ops::cargo_read_manifest::read_manifest;
use cargo::core::Manifest;
use hammer::FlagConfig;

#[deriving(Decodable,Eq,Clone,Ord)]
pub struct ReadManifestFlags {
    manifest_path: ~str
}

impl FlagConfig for ReadManifestFlags {}

fn main() {
    execute_main_without_stdin(execute);
}

fn execute(flags: ReadManifestFlags) -> CargoResult<Option<Manifest>> {
    match read_manifest(flags.manifest_path) {
        Ok(manifest) => Ok(Some(manifest)),
        Err(e) => Err(e)
    }
}
