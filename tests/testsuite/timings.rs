//! Tests for --timings.

use crate::prelude::*;
use cargo_test_support::basic_manifest;
use cargo_test_support::project;
use cargo_test_support::registry::Package;
use cargo_test_support::str;

#[cargo_test]
fn timings_works() {
    Package::new("dep", "0.1.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            edition = "2015"

            [dependencies]
            dep = "0.1"
            "#,
        )
        .file("src/lib.rs", "")
        .file("src/main.rs", "fn main() {}")
        .file("tests/t1.rs", "")
        .file("examples/ex1.rs", "fn main() {}")
        .build();

    p.cargo("build --all-targets --timings")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] dep v0.1.0 (registry `dummy-registry`)
[COMPILING] dep v0.1.0
[COMPILING] foo v0.1.0 ([ROOT]/foo)
      Timing report saved to [ROOT]/foo/target/cargo-timings/cargo-timing-[..].html
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("clean").run();

    p.cargo("test --timings").run();

    p.cargo("clean").run();

    p.cargo("check --timings").run();

    p.cargo("clean").run();

    p.cargo("doc --timings").run();
}

#[cargo_test]
fn doc_test_units_not_reported() {
    // Doctest should not show up as zero-duration rows.
    // See https://github.com/rust-lang/cargo/issues/17212.
    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "0.0.0"))
        .file(
            "src/lib.rs",
            r#"
            /// ```
            /// assert_eq!(foo::add(1, 2), 3);
            /// ```
            pub fn add(a: u64, b: u64) -> u64 {
                a + b
            }
            "#,
        )
        .build();

    p.cargo("test --timings").run();

    let html = p.read_file("target/cargo-timings/cargo-timing.html");

    assert!(html.contains(r#"\"lib\" (test)"#));
    assert!(!html.contains("(doc test)"));
}

#[cargo_test]
fn fresh_units_not_reported() {
    // A rebuild where nothing is dirty should report no units,
    // rather than a zero-duration row for every fresh unit.
    // See https://github.com/rust-lang/cargo/issues/17212.
    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "0.0.0"))
        .file("src/lib.rs", "")
        .build();

    p.cargo("build --timings").run();
    let html = p.read_file("target/cargo-timings/cargo-timing.html");
    assert!(html.contains(r#""name": "foo""#));

    p.cargo("build --timings").run();
    let html = p.read_file("target/cargo-timings/cargo-timing.html");
    assert!(html.contains("const UNIT_DATA = [];"));
}

#[cargo_test]
fn report_generated_without_any_units() {
    // `cargo test --timings` with nothing to build should still generate
    // a report instead of failing with "no timing data found in log".
    // See https://github.com/rust-lang/cargo/issues/17212.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.0"
            edition = "2015"

            [lib]
            test = false
            doctest = false
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("test --timings")
        .with_stderr_data(str![[r#"
      Timing report saved to [ROOT]/foo/target/cargo-timings/cargo-timing-[..].html
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    let html = p.read_file("target/cargo-timings/cargo-timing.html");
    assert!(html.contains("const UNIT_DATA = [];"));
}
