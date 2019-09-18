//! This build script is basically the whole hack that makes this entire "mock
//! std" feature work. Here we print out `rustc-link-search` pointing to the
//! sysroot of the actual compiler itself, and that way we can indeed implicitly
//! pull in those crates, but only via `extern crate`. That means that we can
//! build tiny shim core/std/etc crates while they actually load all the various
//! language/library details from the actual crates, meaning that instead of
//! literally compiling libstd we compile just our own tiny shims.

use std::process::Command;
use std::env;

fn main() {
    let output = Command::new("rustc")
        .arg("--print")
        .arg("sysroot")
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let stdout = stdout.trim();
    let host = env::var("HOST").unwrap();
    println!("cargo:rustc-link-search={}/lib/rustlib/{}/lib", stdout, host);
}
