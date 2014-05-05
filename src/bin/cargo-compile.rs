#![crate_id="cargo-compile"]
#![allow(deprecated_owned_vector)]

extern crate cargo;
extern crate hammer;
extern crate serialize;

use cargo::ops::cargo_compile::compile;
use cargo::{CargoResult,ToCargoError};
use hammer::{FlagDecoder,FlagConfig,HammerError};
use serialize::Decodable;

#[deriving(Eq,Clone,Decodable,Encodable)]
pub struct Options {
    manifest_path: ~str
}

impl FlagConfig for Options {}

fn flags<T: FlagConfig + Decodable<FlagDecoder, HammerError>>() -> CargoResult<T> {
    let mut decoder = FlagDecoder::new::<T>(std::os::args().tail());
    Decodable::decode(&mut decoder).to_cargo_error(|e: HammerError| e.message, 1)
}

fn execute() -> CargoResult<()> {
    compile(try!(flags::<Options>()).manifest_path.as_slice())
}

fn main() {
    match execute() {
        Err(io_error) => fail!("{}", io_error),
        Ok(_) => return
    }
}
