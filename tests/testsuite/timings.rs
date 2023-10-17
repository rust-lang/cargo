//! Tests for --timings.

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

    p.cargo("build --all-targets --timings")
        .with_stderr_unordered(
            "\
[UPDATING] [..]
[DOWNLOADING] crates ...
[DOWNLOADED] dep v0.1.0 [..]
[COMPILING] dep v0.1.0
[COMPILING] foo v0.1.0 [..]
[FINISHED] [..]
      Timing report saved to [..]/foo/target/cargo-timings/cargo-timing-[..].html
",
        )
        .run();

    p.cargo("clean").run();

    p.cargo("test --timings").run();

    p.cargo("clean").run();

    p.cargo("check --timings").run();

    p.cargo("clean").run();

    p.cargo("doc --timings").run();
}
