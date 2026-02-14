//! Tests for the `cargo verify-project` command.

use crate::prelude::*;
use cargo_test_support::{basic_bin_manifest, main_file, project, str};

#[cargo_test]
fn cargo_verify_project_path_to_cargo_toml_relative() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]))
        .build();

    p.cargo("verify-project --manifest-path foo/Cargo.toml")
        .cwd(p.root().parent().unwrap())
        .with_stdout_data(str![[r#"
{"success":"true"}

"#]])
        .run();
}

#[cargo_test]
fn cargo_verify_project_path_to_cargo_toml_absolute() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]))
        .build();

    p.cargo("verify-project --manifest-path")
        .arg(p.root().join("Cargo.toml"))
        .cwd(p.root().parent().unwrap())
        .with_stdout_data(str![[r#"
{"success":"true"}

"#]])
        .run();
}

#[cargo_test]
fn cargo_verify_project_cwd() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]))
        .build();

    p.cargo("verify-project")
        .with_stdout_data(str![[r#"
{"success":"true"}

"#]])
        .run();
}

#[cargo_test]
fn cargo_verify_project_honours_unstable_features() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["test-dummy-unstable"]

                [package]
                name = "foo"
                version = "0.0.1"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("verify-project")
        .masquerade_as_nightly_cargo(&["test-dummy-unstable"])
        .with_stdout_data(str![[r#"
{"success":"true"}

"#]])
        .run();

    p.cargo("verify-project")
        .with_status(1)
        .with_stdout_data(str![[r#"
{"invalid":"failed to parse manifest at `[..]`"}

"#]])
        .run();
}

#[cargo_test]
fn verify_project_invalid_toml_syntax() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
[package]
name = "foo"
version = "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("verify-project")
        .with_status(1)
        .with_stdout_data(str![[r#"
{"invalid":"TOML parse error at line 4, column 11/n  |/n4 | version = /n  |           ^/nstring values must be quoted, expected literal string/n"}

"#]])
        .run();
}
