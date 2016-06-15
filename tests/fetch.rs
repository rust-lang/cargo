extern crate cargotest;
extern crate hamcrest;

use cargotest::support::{project, execs};
use cargotest::support::registry::Package;
use hamcrest::assert_that;

#[test]
fn no_deps() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            authors = []
            version = "0.0.1"
        "#)
        .file("src/main.rs", r#"
            mod a; fn main() {}
        "#)
        .file("src/a.rs", "");

    assert_that(p.cargo_process("fetch"),
                execs().with_status(0).with_stdout(""));
}

#[test]
fn warn_about_multiple_versions() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            bar = "0.0.1"
            baz = "*"
        "#)
        .file("src/main.rs", "fn main() {}");

    Package::new("bar", "0.0.1").publish();
    Package::new("bar", "0.0.2").publish();
    Package::new("baz", "0.0.1").dep("bar", "0.0.2").publish();

    assert_that(p.cargo_process("fetch"),
                execs().with_status(0)
                       .with_stderr_contains("\
[WARNING] using multiple versions of crate \"bar\"
versions: v0.0.1, v0.0.2
"));

    // Warning should be generated only once
    assert_that(p.cargo("fetch"),
                execs().with_status(0)
                       .with_stderr(""));
}
