//! Tests for `hint-msrv`

use crate::prelude::*;
use cargo_test_support::{basic_manifest, project, str};

// Test that `hint-msrv` is not available on stable
#[cargo_test]
fn hint_msrv_stable() {
    let p = project().file("src/lib.rs", "").build();

    p.cargo("build -Zhint-msrv").with_status(101).with_stderr_data(str![[r#"
[ERROR] the `-Z` flag is only accepted on the nightly channel of Cargo, but this is the `stable` channel
See https://doc.rust-lang.org/book/appendix-07-nightly-rust.html for more information about Rust release channels.

"#]]).run();
}

fn msrv_manifest(name: &str, version: &str, msrv: &str) -> String {
    format!(
        r#"
        [package]
        name = "{}"
        version = "{}"
        authors = []
        edition = "2015"
        rust-version = "{}"
    "#,
        name, version, msrv
    )
}

// Test that `hint-msrv` passes `package.rust-version`
#[cargo_test(nightly, reason = "tests nightly flag")]
fn hint_msrv() {
    let p = project()
        .file("src/lib.rs", "")
        .file("Cargo.toml", &msrv_manifest("foo", "0.0.0", "1.78.0"))
        .build();

    p.cargo("build --verbose -Zhint-msrv")
        .masquerade_as_nightly_cargo(&["hint-msrv"])
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.0 ([ROOT]/foo)
[RUNNING] `rustc [..]-Z hint-msrv=1.78.0[..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .with_status(0)
        .run();

    // Test that a missing package.rust-version is not an error, and does not pass a value
    let p = project()
        .file("src/lib.rs", "")
        .file("Cargo.toml", &basic_manifest("foo", "0.0.0"))
        .build();

    p.cargo("build --verbose -Zhint-msrv")
        .masquerade_as_nightly_cargo(&["hint-msrv"])
        .with_stderr_does_not_contain("hint-msrv")
        .with_status(0)
        .run();
}
