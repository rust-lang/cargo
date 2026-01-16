use crate::prelude::*;
use cargo_test_support::project;
use cargo_test_support::str;

#[cargo_test]
fn bin_name_explicit() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
[package]
name = "foo"
version = "0.0.1"
edition = "2015"
authors = []

[[bin]]
name = "foo_bar"
path = "src/main.rs"

[lints.cargo]
non_kebab_case_bin = "warn"
"#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[WARNING] unknown lint: `non_kebab_case_bin`
  --> Cargo.toml:13:1
   |
13 | non_kebab_case_bin = "warn"
   | ^^^^^^^^^^^^^^^^^^
   |
   = [NOTE] `cargo::unknown_lints` is set to `warn` by default
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn bin_name_from_package() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
[package]
name = "foo_bar"
version = "0.0.1"
edition = "2015"
authors = []

[lints.cargo]
non_kebab_case_bin = "warn"
"#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[WARNING] unknown lint: `non_kebab_case_bin`
 --> Cargo.toml:9:1
  |
9 | non_kebab_case_bin = "warn"
  | ^^^^^^^^^^^^^^^^^^
  |
  = [NOTE] `cargo::unknown_lints` is set to `warn` by default
[CHECKING] foo_bar v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn bin_name_from_path() {
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
non_kebab_case_bin = "warn"
"#,
        )
        .file("src/bin/foo_bar.rs", "fn main() {}")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[WARNING] unknown lint: `non_kebab_case_bin`
 --> Cargo.toml:9:1
  |
9 | non_kebab_case_bin = "warn"
  | ^^^^^^^^^^^^^^^^^^
  |
  = [NOTE] `cargo::unknown_lints` is set to `warn` by default
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
fn bin_name_from_script_name() {
    let p = cargo_test_support::project()
        .file(
            "foo_bar",
            r#"
---
[lints.cargo]
non_kebab_case_bin = "warn"
---
fn main() {}"#,
        )
        .build();

    p.cargo("check -Zcargo-lints -Zscript --manifest-path foo_bar")
        .masquerade_as_nightly_cargo(&["cargo-lints", "script"])
        .with_stderr_data(str![[r#"
[WARNING] `package.edition` is unspecified, defaulting to `[..]`
[WARNING] unknown lint: `non_kebab_case_bin`
 --> foo_bar:4:1
  |
4 | non_kebab_case_bin = "warn"
  | ^^^^^^^^^^^^^^^^^^
  |
  = [NOTE] `cargo::unknown_lints` is set to `warn` by default
[CHECKING] foo_bar v0.0.0 ([ROOT]/foo/foo_bar)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}
