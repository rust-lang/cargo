//! Tests for -Ztimings.

use cargo_test_support::project;
use cargo_test_support::registry::Package;

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

            [dependencies]
            dep = "0.1"
            "#,
        )
        .file("src/lib.rs", "")
        .file("src/main.rs", "fn main() {}")
        .file("tests/t1.rs", "")
        .file("examples/ex1.rs", "fn main() {}")
        .build();

    p.cargo("build --all-targets -Ztimings")
        .masquerade_as_nightly_cargo()
        .with_stderr_unordered(
            "\
[UPDATING] [..]
[DOWNLOADING] crates ...
[DOWNLOADED] dep v0.1.0 [..]
[COMPILING] dep v0.1.0
[COMPILING] foo v0.1.0 [..]
[COMPLETED] dep v0.1.0 in [..]s
[COMPLETED] foo v0.1.0 in [..]s
[COMPLETED] foo v0.1.0 bin \"foo\" in [..]s
[COMPLETED] foo v0.1.0 example \"ex1\" in [..]s
[COMPLETED] foo v0.1.0 lib (test) in [..]s
[COMPLETED] foo v0.1.0 bin \"foo\" (test) in [..]s
[COMPLETED] foo v0.1.0 test \"t1\" (test) in [..]s
[FINISHED] [..]
      Timing report saved to [..]/foo/cargo-timing-[..].html
",
        )
        .run();

    p.cargo("clean").run();

    p.cargo("test -Ztimings")
        .masquerade_as_nightly_cargo()
        .run();

    p.cargo("clean").run();

    p.cargo("check -Ztimings")
        .masquerade_as_nightly_cargo()
        .run();

    p.cargo("clean").run();

    p.cargo("doc -Ztimings").masquerade_as_nightly_cargo().run();
}
