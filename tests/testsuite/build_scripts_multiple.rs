//! Tests for multiple build scripts feature.

use cargo_test_support::prelude::*;
use cargo_test_support::project;
use cargo_test_support::str;

#[cargo_test]
fn build_without_feature_enabled_aborts_with_error() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2024"
                build = ["build1.rs", "build2.rs"]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("build1.rs", "fn main() {}")
        .file("build2.rs", "fn main() {}")
        .build();
    p.cargo("check")
        .masquerade_as_nightly_cargo(&["multiple-build-scripts"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  feature `multiple-build-scripts` is required

  The package requires the Cargo feature called `multiple-build-scripts`, but that feature is not stabilized in this version of Cargo ([..]).
  Consider adding `cargo-features = ["multiple-build-scripts"]` to the top of Cargo.toml (above the [package] table) to tell Cargo you are opting in to use this unstable feature.
  See https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#multiple-build-scripts for more information about the status of this feature.

"#]])
        .run();
}

#[cargo_test]
fn empty_multiple_build_script_project() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["multiple-build-scripts"]

                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2024"
                build = ["build1.rs", "build2.rs"]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("build1.rs", "fn main() {}")
        .file("build2.rs", "fn main() {}")
        .build();
    p.cargo("check")
        .masquerade_as_nightly_cargo(&["multiple-build-scripts"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  multiple build scripts feature is not implemented yet!

"#]])
        .run();
}

#[cargo_test]
fn multiple_build_scripts_metadata() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["multiple-build-scripts"]

                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2024"
                build = ["build1.rs", "build2.rs"]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("build1.rs", "fn main() {}")
        .file("build2.rs", "fn main() {}")
        .build();
    p.cargo("metadata --format-version=1")
        .masquerade_as_nightly_cargo(&["multiple-build-scripts"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  multiple build scripts feature is not implemented yet!

"#]])
        .run();
}
