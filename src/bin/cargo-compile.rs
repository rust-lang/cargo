#![crate_id="cargo-compile"]
#![allow(deprecated_owned_vector)]

extern crate cargo;

use cargo::ops::cargo_compile::compile;

fn main() {
    match compile() {
        Err(io_error) => fail!("{}", io_error),
        Ok(_) => return
    }
}
