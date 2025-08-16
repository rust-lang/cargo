//! Tests for `-Zbuild-analysis`.

use crate::prelude::*;

use cargo_test_support::basic_manifest;
use cargo_test_support::project;
use cargo_test_support::str;

#[cargo_test]
fn gated() {
    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "0.0.0"))
        .file("src/lib.rs", "")
        .build();

    p.cargo("check")
        .env("CARGO_BUILD_ANALYSIS_ENABLED", "true")
        .masquerade_as_nightly_cargo(&["build-analysis"])
        .with_stderr_data(str![[r#"
[WARNING] ignoring 'build.analysis' config, pass `-Zbuild-analysis` to enable it
[CHECKING] foo v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn simple() {
    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "0.0.0"))
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Zbuild-analysis")
        .env("CARGO_BUILD_ANALYSIS_ENABLED", "true")
        .masquerade_as_nightly_cargo(&["build-analysis"])
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.0.0 ([ROOT]/foo)
      Timing report saved to [ROOT]/foo/target/cargo-timings/cargo-timing-[..].html
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}
