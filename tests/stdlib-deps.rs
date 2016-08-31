#[macro_use]
extern crate cargotest;
extern crate hamcrest;

use std::fs::{self, File};
use std::io::prelude::*;

use cargotest::support::{paths, project, execs};
use cargotest::support::registry::Package;
use hamcrest::assert_that;

fn setup() {
    let root = paths::root();
    t!(fs::create_dir(&root.join(".cargo")));
    t!(t!(File::create(root.join(".cargo/config"))).write_all(br#"
        keep-stdlib-dependencies = true

        [source.compiler]
        registry = 'https://wut'
        replace-with = 'my-awesome-local-registry'

        [source.my-awesome-local-registry]
        local-registry = 'registry'
    "#));
}

#[test]
fn explicit_stdlib_deps() {
    setup();
    Package::new("core", "1.0.0").local(true).publish();

    let p = project("local")
        .file("Cargo.toml", r#"
            [package]
            name = "local"
            version = "0.0.1"
            authors = []

            [dependencies]
            core = { version = "1", stdlib = true }
        "#)
        .file("src/lib.rs", "");

    assert_that(p.cargo_process("build").arg("--verbose"),
                execs().with_status(0)
                .with_stderr_contains(
                    "[WARNING] the `keep-stdlib-dependencies` config key is unstable")
                .with_stderr_contains(
                    "[WARNING] the \"compiler source\" is unstable [..]")
                .with_stderr_contains(
                    "[WARNING] explicit dependencies are unstable"));
}

#[test]
fn unresolved_explicit_stdlib_deps() {
    setup();
    let p = project("local")
        .file("Cargo.toml", r#"
            [package]
            name = "local"
            version = "0.0.1"
            authors = []

            [dependencies]
            core = { version = "1", stdlib = true }
        "#)
        .file("src/lib.rs", "");

    assert_that(p.cargo_process("build").arg("--verbose"),
                execs().with_status(101)
                .with_stderr_contains(
                    "[WARNING] the \"compiler source\" is unstable [..]")
                .with_stderr_contains(
                    "[WARNING] the `keep-stdlib-dependencies` config key is unstable")
                .with_stderr_contains(
                    "[WARNING] explicit dependencies are unstable")
                .with_stderr_contains("\
[ERROR] failed to load source for a dependency on `core`

Caused by:
  Unable to update registry file://[..]

Caused by:
  failed to update replaced source `registry file://[..]

Caused by:
  local registry path is not a directory: [..]
"));
}

#[test]
fn stdlib_replacement() {
    setup();
    let p = project("local")
        .file("Cargo.toml", r#"
            [package]
            name = "local"
            version = "0.0.1"
            authors = []

            [replace]
            "foo:1.0.0" = { stdlib = true }
        "#)
        .file("src/lib.rs", "");

    assert_that(p.cargo_process("update").arg("--verbose"),
                execs().with_status(101).with_stderr_contains("\
[ERROR] failed to parse manifest at [..]

Caused by:
  replacements cannot be standard library packages, but found one for `foo:1.0.0`
"));
}

#[test]
fn good_explicit_stdlib_deps_pruned() {
    let foo = project("foo")
    .file("Cargo.toml", r#"
        [package]
        name = "foo"
        version = "0.0.0"
        authors = []

        [dependencies]
        core = { version = "1", stdlib = true }
        alloc = { version = "1", stdlib = true }
    "#)
    .file("src/lib.rs", "")
    .file(".cargo/config", r#"
        keep-stdlib-dependencies = false
    "#);
    assert_that(foo.cargo_process("build").arg("-v"),
                execs().with_status(0));
}

#[test]
fn bad_explicit_deps_enabled_pruned_still_error() {
    setup();
    let foo = project("foo")
    .file("Cargo.toml", r#"
        [package]
        name = "foo"
        version = "0.0.0"
        authors = []

        [dependencies]
        core = { version = "bad bad bad", stdlib = true }
    "#)
    .file("src/lib.rs", "")
    .file(".cargo/config", r#"
        keep-stdlib-dependecies = false
    "#);
    assert_that(foo.cargo_process("build").arg("-v"),
                execs().with_status(101).with_stderr("\
error: failed to parse manifest at [..]

Caused by:
  [..]
"));
}
