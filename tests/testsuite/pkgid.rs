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

#[cargo_test]
fn suggestion_bad_pkgid() {
    Package::new("crates-io", "0.1.0").publish();
    Package::new("two-ver", "0.1.0").publish();
    Package::new("two-ver", "0.2.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2018"

                [dependencies]
                crates-io = "0.1.0"
                two-ver = "0.1.0"
                two-ver2 = { package = "two-ver", version = "0.2.0" }
            "#,
        )
        .file("src/lib.rs", "")
        .file("cratesio", "")
        .build();

    p.cargo("generate-lockfile").run();

    // Bad URL.
    p.cargo("pkgid https://example.com/crates-io")
        .with_status(101)
        .with_stderr(
            "\
error: package ID specification `https://example.com/crates-io` did not match any packages
Did you mean one of these?

  crates-io:0.1.0
",
        )
        .run();

    // Bad name.
    p.cargo("pkgid crates_io")
        .with_status(101)
        .with_stderr(
            "\
error: package ID specification `crates_io` did not match any packages

<tab>Did you mean `crates-io`?
",
        )
        .run();

    // Bad version.
    p.cargo("pkgid two-ver:0.3.0")
        .with_status(101)
        .with_stderr(
            "\
error: package ID specification `two-ver:0.3.0` did not match any packages
Did you mean one of these?

  two-ver:0.1.0
  two-ver:0.2.0
",
        )
        .run();

    // Bad file URL.
    p.cargo("pkgid ./Cargo.toml")
        .with_status(101)
        .with_stderr(
            "\
error: invalid package ID specification: `./Cargo.toml`

Caused by:
  package ID specification `./Cargo.toml` looks like a file path, maybe try file://[..]/Cargo.toml
",
        )
        .run();

    // Bad file URL with simliar name.
    p.cargo("pkgid './cratesio'")
        .with_status(101)
        .with_stderr(
            "\
error: invalid package ID specification: `./cratesio`

<tab>Did you mean `crates-io`?

Caused by:
  package ID specification `./cratesio` looks like a file path, maybe try file://[..]/cratesio
",
        )
        .run();
}
