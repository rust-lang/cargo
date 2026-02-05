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
[WARNING] missing `[lints]` to inherit `[workspace.lints]`
 --> bar/Cargo.toml
  = [NOTE] `cargo::missing_lints_inheritance` is set to `warn` by default
[HELP] to inherit `workspace.lints, add:
  |
5 ~ edition = "2015"
6 + [lints]
7 + workspace = true
  |
[HELP] to clarify your intent to not inherit, add:
  |
5 ~ edition = "2015"
6 + [lints]
  |
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
[CHECKING] bar v0.0.1 ([ROOT]/foo/bar)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}
