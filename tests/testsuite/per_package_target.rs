//! Tests for the `-Z per-package-target` feature.

use crate::prelude::*;
use cargo_test_support::project;
use cargo_test_support::registry::Package;

use cargo_test_support::cross_compile;
use std::process::Command;

#[cargo_test]
fn forced_target_with_artifact_dep() {
    let target = cross_compile::alternate();

    // Check if target is installed by trying to compile a dummy file.
    let mut child = Command::new("rustc")
        .arg("--target")
        .arg(target)
        .arg("-")
        .arg("--crate-type=bin")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();

    use std::io::Write;
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"fn main() {}")
        .unwrap();

    if !child.wait().unwrap().success() {
        return;
    }

    Package::new("bar", "1.0.0")
        .file("src/lib.rs", "pub fn bar() {}")
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                cargo-features = ["per-package-target"]
                [package]
                name = "foo"
                version = "0.1.0"
                forced-target = "{target}"

                [build-dependencies]
                bar = {{ version = "1.0", artifact = "bin", target = "target" }}
                "#,
                target = target
            ),
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("build -Z bindeps")
        .masquerade_as_nightly_cargo(&["per-package-target"])
        .env("RUSTC_BOOTSTRAP", "1")
        .run();
}
