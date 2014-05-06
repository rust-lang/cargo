#![crate_id="cargo-compile"]
#![allow(deprecated_owned_vector)]

extern crate cargo;
extern crate hammer;
extern crate serialize;

use cargo::ops::cargo_compile::compile;
use cargo::core::errors::{CLIResult,CLIError,ToResult};
use hammer::{FlagDecoder,FlagConfig,HammerError};
use serialize::Decodable;

#[deriving(Eq,Clone,Decodable,Encodable)]
pub struct Options {
    manifest_path: ~str
}

impl FlagConfig for Options {}

fn flags<T: FlagConfig + Decodable<FlagDecoder, HammerError>>() -> CLIResult<T> {
    let mut decoder = FlagDecoder::new::<T>(std::os::args().tail());
    Decodable::decode(&mut decoder).to_result(|e: HammerError| CLIError::new(e.message, None, 1))
}

fn execute() -> CLIResult<()> {
    compile(try!(flags::<Options>()).manifest_path.as_slice()).to_result(|_|
        CLIError::new("Compilation failed", None, 1))
}

fn main() {
    match execute() {
        Err(err) => fail!("{}", err),
        Ok(_) => return
    }
}
