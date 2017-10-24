extern crate cargotest;
extern crate hamcrest;

use cargotest::support::{project, execs};
use hamcrest::assert_that;

#[test]
fn parses_env() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/lib.rs", "")
        .build();

    assert_that(p.cargo("doc").env("RUSTDOCFLAGS", "--cfg=foo").arg("-v"),
                execs().with_status(0)
                       .with_stderr_contains("\
[RUNNING] `rustdoc [..] --cfg=foo[..]`
"));
}

#[test]
fn parses_config() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/lib.rs", "")
        .file(".cargo/config", r#"
            [build]
            rustdocflags = ["--cfg", "foo"]
        "#)
        .build();

    assert_that(p.cargo("doc").arg("-v"),
                execs().with_status(0)
                       .with_stderr_contains("\
[RUNNING] `rustdoc [..] --cfg foo[..]`
"));
}

#[test]
fn bad_flags() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/lib.rs", "")
        .build();

    assert_that(p.cargo("doc").env("RUSTDOCFLAGS", "--bogus"),
                execs().with_status(101));
}

#[test]
fn rerun() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/lib.rs", "")
        .build();

    assert_that(p.cargo("doc").env("RUSTDOCFLAGS", "--cfg=foo"),
                execs().with_status(0));
    assert_that(p.cargo("doc").env("RUSTDOCFLAGS", "--cfg=foo"),
                execs().with_status(0).with_stderr("\
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
"));
    assert_that(p.cargo("doc").env("RUSTDOCFLAGS", "--cfg=bar"),
                execs().with_status(0).with_stderr("\
[DOCUMENTING] foo v0.0.1 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
"));
}
