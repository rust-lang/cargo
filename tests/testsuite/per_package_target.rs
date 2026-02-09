//! Tests for the `-Z per-package-target` feature.

use crate::prelude::*;
use cargo_test_support::project;
use cargo_test_support::registry::Package;

#[cargo_test]
fn forced_target_with_artifact_dep() {
    Package::new("bar", "1.0.0")
        .file("src/lib.rs", "pub fn bar() {}")
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            cargo-features = ["per-package-target"]
            [package]
            name = "foo"
            version = "0.1.0"
            forced-target = "x86_64-unknown-linux-gnu"

            [build-dependencies]
            bar = { version = "1.0", artifact = "bin", target = "target" }
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("build -Z bindeps")
        .masquerade_as_nightly_cargo(&["per-package-target"])
        .run();
}
