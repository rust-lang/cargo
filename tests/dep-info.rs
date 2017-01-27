extern crate cargotest;
extern crate hamcrest;

use cargotest::support::{basic_bin_manifest, main_file, execs, project};
use hamcrest::{assert_that, existing_file};

#[test]
fn build_dep_info() {
    let p = project("foo")
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]));

    assert_that(p.cargo_process("build"), execs().with_status(0));

    let depinfo_bin_path = &p.bin("foo").with_extension("d");

    assert_that(depinfo_bin_path, existing_file());
}

#[test]
fn build_dep_info_lib() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [[example]]
            name = "ex"
            crate-type = ["lib"]
        "#)
        .file("src/lib.rs", "")
        .file("examples/ex.rs", "");

    assert_that(p.cargo_process("build").arg("--example=ex"), execs().with_status(0));
    assert_that(&p.example_lib("ex", "lib").with_extension("d"), existing_file());
}


#[test]
fn build_dep_info_rlib() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [[example]]
            name = "ex"
            crate-type = ["rlib"]
        "#)
        .file("src/lib.rs", "")
        .file("examples/ex.rs", "");

    assert_that(p.cargo_process("build").arg("--example=ex"), execs().with_status(0));
    assert_that(&p.example_lib("ex", "rlib").with_extension("d"), existing_file());
}

#[test]
fn build_dep_info_dylib() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [[example]]
            name = "ex"
            crate-type = ["dylib"]
        "#)
        .file("src/lib.rs", "")
        .file("examples/ex.rs", "");

    assert_that(p.cargo_process("build").arg("--example=ex"), execs().with_status(0));
    assert_that(&p.example_lib("ex", "dylib").with_extension("d"), existing_file());
}
