//! Tests for variable references, e.g. { path = "$(FOO)/bar/foo" }
#![allow(unused_imports)]

use cargo_test_support::registry::Package;
use cargo_test_support::{basic_lib_manifest, basic_manifest, git, project, sleep_ms};
use std::env;
use std::fs;

#[cargo_test]
fn var_refs() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
        [workspace]
        members = ["utils/zoo", "bar"]
        "#,
        )
        .file(
            "utils/zoo/Cargo.toml",
            r#"
        [package]
        name = "zoo"
        version = "1.0.0"
        edition = "2018"
        authors = []

        [lib]
    "#,
        )
        .file(
            "utils/zoo/src/lib.rs",
            r#"
        pub fn hello() { println!("Hello, world!"); }
    "#,
        )
        .file(
            "bar/Cargo.toml",
            r#"
        [package]
        name = "bar"
        version = "1.0.0"
        edition = "2018"
        authors = []

        [dependencies]
        zoo = { path = "$(UTILS_ROOT)/zoo" }
    "#,
        )
        .file(
            "bar/src/main.rs",
            r#"
            fn main() {
                zoo::hello();
            }
            "#,
        )
        .build();

    p.cargo("build")
        .cwd("bar")
        .env("UTILS_ROOT", "../utils")
        .run();
    assert!(p.bin("bar").is_file());
}

#[cfg(todo)] // error propagation is not working correctly.
#[cargo_test]
fn var_refs_var_not_set() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
        [package]
        name = "foo"
        version = "1.0.0"
        edition = "2018"
        authors = []

        [dependencies]
        bar = { path = "$(BAD_VAR)/bar" }
        "#,
        )
        .file("src/lib.rs", r#""#)
        .build();

    p.cargo("build")
        .with_status(101)
        .with_stderr_contains("environment variable 'BAD_VAR' is not set")
        .run();
}

#[cfg(todo)] // error propagation is not working correctly.
#[cargo_test]
fn var_refs_bad_syntax() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
        [package]
        name = "foo"
        version = "1.0.0"
        edition = "2018"
        authors = []

        [dependencies]
        bar = { path = "$(BAD_VAR" }
        "#,
        )
        .file("src/lib.rs", r#""#)
        .build();

    p.cargo("build")
        .with_status(101)
        .with_stderr_contains("variable reference '$(FOO)' is missing closing parenthesis")
        .run();
}
