use cargo_test_support::prelude::*;
use cargo_test_support::registry::Package;
use cargo_test_support::str;
use cargo_test_support::{file, project};

#[cargo_test]
fn case() {
    Package::new("bar", "0.1.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
[package]
name = "foo"
version = "0.1.0"
edition = "2021"

[dependencies]
bar = { version = "0.1.0", optional = true }

[lints.cargo]
implicit-features = "warn"
"#,
        )
        .file("src/lib.rs", "")
        .build();

    snapbox::cmd::Command::cargo_ui()
        .masquerade_as_nightly_cargo(&["always_nightly"])
        .current_dir(p.root())
        .arg("check")
        .arg("--quiet")
        .assert()
        .success()
        .stdout_matches(str![""])
        .stderr_matches(file!["stderr.term.svg"]);
}
