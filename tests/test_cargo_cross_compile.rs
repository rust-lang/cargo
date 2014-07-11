// Currently the only cross compilers available via nightlies are on linux/osx,
// so we can only run these tests on those platforms
#![cfg(target_os = "linux")]
#![cfg(target_os = "macos")]

use std::os;

use support::{project, execs, basic_bin_manifest};
use hamcrest::{assert_that, existing_file};
use cargo::util::process;

fn setup() {
}

fn alternate() -> &'static str {
    match os::consts::SYSNAME {
        "linux" => "i686-unknown-linux-gnu",
        "darwin" => "i686-apple-darwin",
        _ => unreachable!(),
    }
}

test!(simple_cross {
    let p = project("foo")
        .file("Cargo.toml", basic_bin_manifest("foo").as_slice())
        .file("src/foo.rs", r#"
            use std::os;
            fn main() {
                assert_eq!(os::consts::ARCH, "x86");
            }
        "#);

    let target = alternate();
    assert_that(p.cargo_process("cargo-build").arg("--target").arg(target),
                execs().with_status(0));
    assert_that(&p.target_bin(target, "foo"), existing_file());

    assert_that(
      process(p.target_bin(target, "foo")),
      execs().with_status(0));
})

test!(simple_deps {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.bar]
            path = "../bar"
        "#)
        .file("src/main.rs", r#"
            extern crate bar;
            fn main() { bar::bar(); }
        "#);
    let p2 = project("bar")
        .file("Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/lib.rs", "pub fn bar() {}");
    p2.build();

    let target = alternate();
    assert_that(p.cargo_process("cargo-build").arg("--target").arg(target),
                execs().with_status(0));
    assert_that(&p.target_bin(target, "main"), existing_file());

    assert_that(
      process(p.target_bin(target, "main")),
      execs().with_status(0));
})


