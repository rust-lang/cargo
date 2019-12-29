//! Tests for the `cargo pkgid` command.

use cargo_test_support::project;
use cargo_test_support::registry::Package;

#[cargo_test]
fn simple() {
    Package::new("bar", "0.1.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            edition = "2018"

            [dependencies]
            bar = "0.1.0"
        "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("generate-lockfile").run();

    p.cargo("pkgid foo")
        .with_stdout(format!("file://[..]{}#0.1.0", p.root().to_str().unwrap()))
        .run();

    p.cargo("pkgid bar")
        .with_stdout("https://github.com/rust-lang/crates.io-index#bar:0.1.0")
        .run();
}
