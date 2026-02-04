use crate::prelude::*;
use cargo_test_support::project;
use cargo_test_support::str;

#[cargo_test]
fn no_lints() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
[workspace]
members = ["bar"]
"#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "bar/Cargo.toml",
            r#"
[package]
name = "bar"
version = "0.0.1"
edition = "2015"
"#,
        )
        .file("bar/src/main.rs", "fn main() {}")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[CHECKING] bar v0.0.1 ([ROOT]/foo/bar)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn ws_lints() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
[workspace]
members = ["bar"]

[workspace.lints.cargo]
missing_lints_inheritance = "warn"
"#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "bar/Cargo.toml",
            r#"
[package]
name = "bar"
version = "0.0.1"
edition = "2015"
"#,
        )
        .file("bar/src/main.rs", "fn main() {}")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[WARNING] unknown lint: `missing_lints_inheritance`
 --> Cargo.toml:6:1
  |
6 | missing_lints_inheritance = "warn"
  | ^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = [NOTE] `cargo::unknown_lints` is set to `warn` by default
[CHECKING] bar v0.0.1 ([ROOT]/foo/bar)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn empty_pkg_lints() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
[workspace]
members = ["bar"]

[workspace.lints.cargo]
missing_lints_inheritance = "warn"
"#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "bar/Cargo.toml",
            r#"
[package]
name = "bar"
version = "0.0.1"
edition = "2015"

[lints]
"#,
        )
        .file("bar/src/main.rs", "fn main() {}")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[WARNING] unknown lint: `missing_lints_inheritance`
 --> Cargo.toml:6:1
  |
6 | missing_lints_inheritance = "warn"
  | ^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = [NOTE] `cargo::unknown_lints` is set to `warn` by default
[CHECKING] bar v0.0.1 ([ROOT]/foo/bar)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn inherit_lints() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
[workspace]
members = ["bar"]

[workspace.lints.cargo]
missing_lints_inheritance = "warn"
"#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "bar/Cargo.toml",
            r#"
[package]
name = "bar"
version = "0.0.1"
edition = "2015"

[lints]
workspace = true
"#,
        )
        .file("bar/src/main.rs", "fn main() {}")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[WARNING] unknown lint: `missing_lints_inheritance`
 --> Cargo.toml:6:1
  |
6 | missing_lints_inheritance = "warn"
  | ^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = [NOTE] `cargo::unknown_lints` is set to `warn` by default
[CHECKING] bar v0.0.1 ([ROOT]/foo/bar)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}
