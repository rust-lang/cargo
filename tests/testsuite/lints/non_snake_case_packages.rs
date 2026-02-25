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
[WARNING] packages should have a snake-case name
 --> Cargo.toml:3:8
  |
3 | name = "foo-bar"
  |        ^^^^^^^^^
  |
  = [NOTE] `cargo::non_snake_case_packages` is set to `warn` in `[lints]`
[HELP] to change the package name to snake case, convert `package.name`
  |
3 - name = "foo-bar"
3 + name = "foo_bar"
  |
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
[WARNING] `package.edition` is unspecified, defaulting to the latest edition (currently `[..]`)
[WARNING] packages should have a snake-case name
 --> foo-bar
  = [NOTE] `cargo::non_snake_case_packages` is set to `warn` in `[lints]`
[HELP] to change the package name to snake case, convert the file stem
  |
1 - [ROOT]/foo/foo-bar
1 + [ROOT]/foo/foo_bar
  |
[CHECKING] foo-bar v0.0.0 ([ROOT]/foo/foo-bar)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}
