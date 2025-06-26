use crate::prelude::*;
use cargo_test_support::str;
use cargo_test_support::{file, project};

#[cargo_test]
fn case() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
cargo-features = ["test-dummy-unstable"]

[package]
name = "foo"
version = "0.0.1"
edition = "2015"
authors = []
im-a-teapot = true

[lints.cargo]
im_a_teapot = "deny"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    snapbox::cmd::Command::cargo_ui()
        .masquerade_as_nightly_cargo(&["cargo-lints", "test-dummy-unstable"])
        .current_dir(p.root())
        .arg("check")
        .arg("-Zcargo-lints")
        .assert()
        .code(101)
        .stdout_eq(str![""])
        .stderr_eq(file!["stderr.term.svg"]);
}
