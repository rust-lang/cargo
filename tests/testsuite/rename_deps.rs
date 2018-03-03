use cargotest::support::{project, execs};
use cargotest::support::registry::Package;
use cargotest::ChannelChanger;
use hamcrest::assert_that;

#[test]
fn gated() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            bar = { package = "foo", version = "0.1" }
        "#)
        .file("src/lib.rs", "")
        .build();

    assert_that(p.cargo("build").masquerade_as_nightly_cargo(),
                execs().with_status(101)
                       .with_stderr("\
error: failed to parse manifest at `[..]`

Caused by:
  feature `rename-dependency` is required

consider adding `cargo-features = [\"rename-dependency\"]` to the manifest
"));

    let p = project("bar")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            bar = { version = "0.1", package = "baz" }
        "#)
        .file("src/lib.rs", "")
        .build();

    assert_that(p.cargo("build").masquerade_as_nightly_cargo(),
                execs().with_status(101)
                       .with_stderr("\
error: failed to parse manifest at `[..]`

Caused by:
  feature `rename-dependency` is required

consider adding `cargo-features = [\"rename-dependency\"]` to the manifest
"));
}

#[test]
fn rename_dependency() {
    Package::new("bar", "0.1.0").publish();
    Package::new("bar", "0.2.0").publish();

    let p = project("foo")
        .file("Cargo.toml", r#"
            cargo-features = ["rename-dependency"]

            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            bar = { version = "0.1.0" }
            baz = { version = "0.2.0", package = "bar" }
        "#)
        .file("src/lib.rs", "
            extern crate bar;
            extern crate baz;
        ")
        .build();

    assert_that(p.cargo("build").masquerade_as_nightly_cargo(),
                execs().with_status(0));
}

#[test]
fn rename_with_different_names() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            cargo-features = ["rename-dependency"]

            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            baz = { path = "bar", package = "bar" }
        "#)
        .file("src/lib.rs", "
            extern crate baz;
        ")
        .file("bar/Cargo.toml", r#"
            [project]
            name = "bar"
            version = "0.0.1"
            authors = []

            [lib]
            name = "random_name"
        "#)
        .file("bar/src/lib.rs", "")
        .build();

    assert_that(p.cargo("build").masquerade_as_nightly_cargo(),
                execs().with_status(0));
}
