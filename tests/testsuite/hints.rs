//! Tests for hints.

use crate::prelude::*;
use cargo_test_support::{project, str};

#[cargo_test]
fn empty_hints_warn() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            edition = "2015"

            [hints]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();
    p.cargo("check -v")
        .with_stderr_data(str![[r#"
[WARNING] unused manifest key: hints
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn unknown_hints_warn() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            edition = "2015"

            [hints]
            this-is-an-unknown-hint = true
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();
    p.cargo("check -v")
        .with_stderr_data(str![[r#"
[WARNING] unused manifest key: hints
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}
