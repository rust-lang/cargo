extern crate cargo;
extern crate cargotest;
extern crate hamcrest;

use cargotest::ChannelChanger;
use cargotest::support::{project, execs};
use hamcrest::assert_that;

#[test]
fn feature_required() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "a"
            version = "0.0.1"
            authors = []
            im-a-teapot = true
        "#)
        .file("src/lib.rs", "");
    assert_that(p.cargo_process("build")
                 .masquerade_as_nightly_cargo(),
                execs().with_status(101)
                       .with_stderr("\
error: failed to parse manifest at `[..]`

Caused by:
  the `im-a-teapot` manifest key is unstable and may not work properly in England

Caused by:
  feature `test-dummy-unstable` is required

consider adding `cargo-features = [\"test-dummy-unstable\"]` to the manifest
"));

    assert_that(p.cargo("build"),
                execs().with_status(101)
                       .with_stderr("\
error: failed to parse manifest at `[..]`

Caused by:
  the `im-a-teapot` manifest key is unstable and may not work properly in England

Caused by:
  feature `test-dummy-unstable` is required

this Cargo does not support nightly features, but if you
switch to nightly channel you can add
`cargo-features = [\"test-dummy-unstable\"]` to enable this feature
"));
}

#[test]
fn unknown_feature() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "a"
            version = "0.0.1"
            authors = []
            cargo-features = ["foo"]
        "#)
        .file("src/lib.rs", "");
    assert_that(p.cargo_process("build"),
                execs().with_status(101)
                       .with_stderr("\
error: failed to parse manifest at `[..]`

Caused by:
  unknown cargo feature `foo`
"));
}

#[test]
fn stable_feature_warns() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "a"
            version = "0.0.1"
            authors = []
            cargo-features = ["test-dummy-stable"]
        "#)
        .file("src/lib.rs", "");
    assert_that(p.cargo_process("build"),
                execs().with_status(0)
                       .with_stderr("\
warning: the cargo feature `test-dummy-stable` is now stable and is no longer \
necessary to be listed in the manifest
[COMPILING] a [..]
[FINISHED] [..]
"));
}

#[test]
fn nightly_feature_requires_nightly() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "a"
            version = "0.0.1"
            authors = []
            im-a-teapot = true
            cargo-features = ["test-dummy-unstable"]
        "#)
        .file("src/lib.rs", "");
    assert_that(p.cargo_process("build")
                 .masquerade_as_nightly_cargo(),
                execs().with_status(0)
                       .with_stderr("\
[COMPILING] a [..]
[FINISHED] [..]
"));

    assert_that(p.cargo("build"),
                execs().with_status(101)
                       .with_stderr("\
error: failed to parse manifest at `[..]`

Caused by:
  the cargo feature `test-dummy-unstable` requires a nightly version of Cargo, \
  but this is the `stable` channel
"));
}

#[test]
fn cant_publish() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "a"
            version = "0.0.1"
            authors = []
            im-a-teapot = true
            cargo-features = ["test-dummy-unstable"]
        "#)
        .file("src/lib.rs", "");
    assert_that(p.cargo_process("build")
                 .masquerade_as_nightly_cargo(),
                execs().with_status(0)
                       .with_stderr("\
[COMPILING] a [..]
[FINISHED] [..]
"));

    assert_that(p.cargo("build"),
                execs().with_status(101)
                       .with_stderr("\
error: failed to parse manifest at `[..]`

Caused by:
  the cargo feature `test-dummy-unstable` requires a nightly version of Cargo, \
  but this is the `stable` channel
"));
}
