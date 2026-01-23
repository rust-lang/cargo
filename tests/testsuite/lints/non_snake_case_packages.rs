use crate::prelude::*;
use cargo_test_support::project;
use cargo_test_support::str;

#[cargo_test]
fn package_name_explicit() {
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
[package]
name = "foo-bar"
version = "0.0.1"
edition = "2015"
authors = []

[lints.cargo]
non_snake_case_packages = "warn"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    foo.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints", "test-dummy-unstable"])
        .with_stderr_data(str![[r#"
[WARNING] unknown lint: `non_snake_case_packages`
 --> Cargo.toml:9:1
  |
9 | non_snake_case_packages = "warn"
  | ^^^^^^^^^^^^^^^^^^^^^^^
  |
  = [NOTE] `cargo::unknown_lints` is set to `warn` by default
[CHECKING] foo-bar v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
fn package_name_from_script_name() {
    let p = cargo_test_support::project()
        .file(
            "foo-bar",
            r#"
---
[lints.cargo]
non_snake_case_packages = "warn"
---
fn main() {}"#,
        )
        .build();

    p.cargo("check -Zcargo-lints -Zscript --manifest-path foo-bar")
        .masquerade_as_nightly_cargo(&["cargo-lints", "script"])
        .with_stderr_data(str![[r#"
[WARNING] `package.edition` is unspecified, defaulting to `[..]`
[WARNING] unknown lint: `non_snake_case_packages`
 --> foo-bar:4:1
  |
4 | non_snake_case_packages = "warn"
  | ^^^^^^^^^^^^^^^^^^^^^^^
  |
  = [NOTE] `cargo::unknown_lints` is set to `warn` by default
[CHECKING] foo-bar v0.0.0 ([ROOT]/foo/foo-bar)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}
