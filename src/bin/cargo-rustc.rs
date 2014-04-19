#![crate_id="cargo-rustc"]
#![allow(deprecated_owned_vector)]

extern crate cargo;

use cargo::execute_main;
use cargo::ops::cargo_rustc::execute;

fn main() {
    execute_main(execute);
}
