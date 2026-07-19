use crate::prelude::*;
use cargo_test_support::project;
use cargo_test_support::str;

#[cargo_test]
fn default() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
[package]
name = "foo"
version = "0.0.1"
edition = "2015"
authors = []

[lints.cargo]
default = { level = "allow", priority = -1 }
unknown_lints = "warn"
this_lint_does_not_exist = "warn"
"#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("fetch -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[WARNING] unknown lint: `this_lint_does_not_exist`
  --> Cargo.toml:11:1
   |
11 | this_lint_does_not_exist = "warn"
   | ^^^^^^^^^^^^^^^^^^^^^^^^
   |
   = [NOTE] `cargo::unknown_lints` is set to `warn` in `[lints]`
[WARNING] `foo` (manifest) generated 1 warning

"#]])
        .run();
}

#[cargo_test]
fn inherited() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
[workspace]
members = ["foo"]

[workspace.lints.cargo]
default = { level = "allow", priority = -1 }
unknown_lints = "warn"
this_lint_does_not_exist = "warn"
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

    p.cargo("fetch -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[WARNING] unknown lint: `this_lint_does_not_exist`
 --> Cargo.toml:8:1
  |
8 | this_lint_does_not_exist = "warn"
  | ^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = [NOTE] `cargo::unknown_lints` is set to `warn` in `[lints]`
[WARNING] workspace (manifest) generated 1 warning

"#]])
        .run();
}

#[cargo_test]
fn not_inherited() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
[workspace]
members = ["foo"]

[workspace.lints.cargo]
default = { level = "allow", priority = -1 }
unknown_lints = "warn"
this_lint_does_not_exist = "warn"
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
            "#,
        )
        .file("foo/src/lib.rs", "")
        .build();

    p.cargo("fetch -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[WARNING] unknown lint: `this_lint_does_not_exist`
 --> Cargo.toml:8:1
  |
8 | this_lint_does_not_exist = "warn"
  | ^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = [NOTE] `cargo::unknown_lints` is set to `warn` in `[lints]`
[WARNING] workspace (manifest) generated 1 warning
[WARNING] missing `[lints]` to inherit `[workspace.lints]`
 --> foo/Cargo.toml
  = [NOTE] `cargo::missing_lints_inheritance` is set to `warn` by default
[HELP] to inherit `workspace.lints, add:
  |
7 ~             
8 + [lints]
9 + workspace = true
  |
[HELP] to clarify your intent to not inherit, add:
  |
7 ~             
8 + [lints]
  |
[WARNING] `foo` (manifest) generated 1 warning

"#]])
        .run();
}
