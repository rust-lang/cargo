use crate::prelude::*;
use cargo_test_support::str;
use cargo_test_support::{file, project};

#[cargo_test]
fn case() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
[workspace]
members = ["foo"]

[workspace.lints.cargo]
im_a_teapot = { level = "warn", priority = 10 }
"#,
        )
        .file(
            "foo/Cargo.toml",
            r#"
[package]
name = "foo"
version = "0.0.1"
edition = "2015"
authors = []

[lints]
workspace = true
"#,
        )
        .file("foo/src/lib.rs", "")
        .build();

    snapbox::cmd::Command::cargo_ui()
        .current_dir(p.root())
        .arg("check")
        .assert()
        .code(101)
        .stdout_eq(str![""])
        .stderr_eq(file!["stderr.term.svg"]);
}
