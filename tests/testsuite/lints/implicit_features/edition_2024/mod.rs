use cargo_test_support::prelude::*;
use cargo_test_support::registry::Package;
use cargo_test_support::str;
use cargo_test_support::{file, project};

#[cargo_test(nightly, reason = "edition2024 is not stable")]
fn case() {
    Package::new("bar", "0.1.0").publish();
    Package::new("baz", "0.1.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
cargo-features = ["edition2024"]
[package]
name = "foo"
version = "0.1.0"
edition = "2024"

[dependencies]
bar = { version = "0.1.0", optional = true }
baz = { version = "0.1.0", optional = true }

[features]
baz = ["dep:baz"]
"#,
        )
        .file("src/lib.rs", "")
        .build();

    snapbox::cmd::Command::cargo_ui()
        .masquerade_as_nightly_cargo(&["always_nightly"])
        .current_dir(p.root())
        .arg("check")
        .assert()
        .code(101)
        .stdout_matches(str![""])
        .stderr_matches(file!["stderr.term.svg"]);
}
