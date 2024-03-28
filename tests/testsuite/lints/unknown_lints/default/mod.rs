use cargo_test_support::prelude::*;
use cargo_test_support::project;
use cargo_test_support::{file, str};

#[cargo_test]
fn case() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
[workspace.lints.cargo]
this-lint-does-not-exist-ws = "warn"

[package]
name = "foo"
version = "0.1.0"
edition = "2021"

[lints.cargo]
this-lint-does-not-exist = "warn"
"#,
        )
        .file("src/lib.rs", "")
        .build();

    snapbox::cmd::Command::cargo_ui()
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .current_dir(p.root())
        .arg("check")
        .arg("-Zcargo-lints")
        .assert()
        .success()
        .stdout_matches(str![""])
        .stderr_matches(file!["stderr.term.svg"]);
}
