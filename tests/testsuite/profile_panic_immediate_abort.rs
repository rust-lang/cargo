//! Tests for `panic = "immediate-abort"`.

use crate::prelude::*;
use cargo_test_support::project;
use cargo_test_support::str;

#[cargo_test]
fn gated_manifest() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"

                [profile.dev]
                panic = "immediate-abort"
           "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check")
        .masquerade_as_nightly_cargo(&["panic-immediate-abort"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  feature `panic-immediate-abort` is required
...
"#]])
        .run();
}

#[cargo_test]
fn gated_config_toml() {
    let p = project()
        .file(
            ".cargo/config.toml",
            r#"
                [profile.dev]
                panic = "immediate-abort"
           "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check")
        .masquerade_as_nightly_cargo(&["panic-immediate-abort"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] config profile `dev` is not valid (defined in `[ROOT]/foo/.cargo/config.toml`)

Caused by:
  feature `panic-immediate-abort` is required
...
"#]])
        .run();
}

#[cargo_test(nightly, reason = "-Cpanic=immediate-abort is unstable")]
fn manifest_gate_works() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["panic-immediate-abort"]
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"

                [profile.dev]
                panic = "immediate-abort"
           "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build --verbose")
        .masquerade_as_nightly_cargo(&["panic-immediate-abort"])
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc [..]-C panic=immediate-abort -Z unstable-options[..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test(nightly, reason = "-Cpanic=immediate-abort is unstable")]
fn cli_gate_works() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"

                [profile.dev]
                panic = "immediate-abort"
           "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build --verbose -Z panic-immediate-abort")
        .masquerade_as_nightly_cargo(&["panic-immediate-abort"])
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc [..]-C panic=immediate-abort -Z unstable-options[..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}
