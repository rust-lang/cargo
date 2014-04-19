#![crate_id="cargo-read-manifest"]
#![allow(deprecated_owned_vector)]

extern crate cargo;

use cargo::execute_main_without_stdin;
use cargo::ops::cargo_read_manifest::execute;

fn main() {
    execute_main_without_stdin(execute);
}
