use support::{project, execs};
use hamcrest::assert_that;

fn setup() {}

test!(bad1 {
    let foo = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.0"
            authors = []
        "#)
        .file("src/lib.rs", "")
        .file(".cargo/config", r#"
              target = "foo"
        "#);
    assert_that(foo.cargo_process("build").arg("-v"),
                execs().with_status(101).with_stderr("\
expected table for configuration key `target`, but found string in [..]config
"));
});

test!(bad2 {
    let foo = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.0"
            authors = []
        "#)
        .file("src/lib.rs", "")
        .file(".cargo/config", r#"
              [http]
                proxy = 3.0
        "#);
    assert_that(foo.cargo_process("publish").arg("-v"),
                execs().with_status(101).with_stderr("\
Couldn't load Cargo configuration

Caused by:
  failed to load TOML configuration from `[..]config`

Caused by:
  failed to parse key `http`

Caused by:
  failed to parse key `proxy`

Caused by:
  found TOML configuration value of unknown type `float`
"));
});

test!(bad3 {
    let foo = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.0"
            authors = []
        "#)
        .file("src/lib.rs", "")
        .file(".cargo/config", r#"
            [http]
              proxy = true
        "#);
    assert_that(foo.cargo_process("publish").arg("-v"),
                execs().with_status(101).with_stderr("\
invalid configuration for key `http.proxy`
expected a string, but found a boolean in [..]config
"));
});

test!(bad4 {
    let foo = project("foo")
        .file(".cargo/config", r#"
            [cargo-new]
              name = false
        "#);
    assert_that(foo.cargo_process("new").arg("-v").arg("foo"),
                execs().with_status(101).with_stderr("\
Failed to create project `foo` at `[..]`

Caused by:
  invalid configuration for key `cargo-new.name`
expected a string, but found a boolean in [..]config
"));
});


test!(bad_cargo_config_jobs {
    let foo = project("foo")
    .file("Cargo.toml", r#"
        [package]
        name = "foo"
        version = "0.0.0"
        authors = []
    "#)
    .file("src/lib.rs", "")
    .file(".cargo/config", r#"
        [build]
        jobs = -1
    "#);
    assert_that(foo.cargo_process("build").arg("-v"),
                execs().with_status(101).with_stderr("\
build.jobs must be positive, but found -1 in [..]
"));
});

test!(default_cargo_config_jobs {
    let foo = project("foo")
    .file("Cargo.toml", r#"
        [package]
        name = "foo"
        version = "0.0.0"
        authors = []
    "#)
    .file("src/lib.rs", "")
    .file(".cargo/config", r#"
        [build]
        jobs = 1
    "#);
    assert_that(foo.cargo_process("build").arg("-v"),
                execs().with_status(0));
});

test!(good_cargo_config_jobs {
    let foo = project("foo")
    .file("Cargo.toml", r#"
        [package]
        name = "foo"
        version = "0.0.0"
        authors = []
    "#)
    .file("src/lib.rs", "")
    .file(".cargo/config", r#"
        [build]
        jobs = 4
    "#);
    assert_that(foo.cargo_process("build").arg("-v"),
                execs().with_status(0));
});

test!(invalid_global_config {
    let foo = project("foo")
    .file("Cargo.toml", r#"
        [package]
        name = "foo"
        version = "0.0.0"
        authors = []

        [dependencies]
        foo = "0.1.0"
    "#)
    .file(".cargo/config", "4")
    .file("src/lib.rs", "");

    assert_that(foo.cargo_process("build").arg("-v"),
                execs().with_status(101).with_stderr("\
failed to parse manifest at `[..]Cargo.toml`

Caused by:
  Couldn't load Cargo configuration

Caused by:
  could not parse TOML configuration in `[..]config`

Caused by:
  could not parse input as TOML
[..]config:2:1 expected `=`, but found eof

"));
});
