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

static STD: &'static str = r#"
pub mod prelude {
    pub mod v1 {
    }
}
"#;

#[test]
fn explicit_stdlib_deps() {
    setup();
    Package::new("core", "1.0.0").local(true).publish();
    Package::new("std", "1.0.0").local(true).file("src/lib.rs", STD).publish();
    Package::new("test", "1.0.0").local(true).publish();

    let p = project("local")
        .file("Cargo.toml", r#"
            [package]
            name = "local"
            version = "0.0.1"
            authors = []

            [dependencies]
            core = { version = "1", stdlib = true }
            std = { version = "1", stdlib = true }
            test = { version = "1", stdlib = true }
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
fn implicit_stdlib_deps() {
    setup();
    Package::new("core", "1.0.0").local(true).publish();
    Package::new("std", "1.0.0").local(true).file("src/lib.rs", STD).publish();
    Package::new("test", "1.0.0").local(true).publish();

    let p = project("local")
        .file("Cargo.toml", r#"
            [package]
            name = "local"
            version = "0.0.1"
            authors = []
        "#)
       .file("src/lib.rs", "");

    assert_that(p.cargo_process("build").arg("--verbose"),
                execs().with_status(0)
                .with_stderr_contains(
                    "[WARNING] the \"compiler source\" is unstable [..]"));
}

#[test]
fn unresolved_explicit_stdlib_deps() {
    setup();
    Package::new("core", "1.0.0").local(true).publish();
    // For dev & build
    Package::new("std", "1.0.0").local(true).publish();
    Package::new("test", "1.0.0").local(true).publish();

    let p = project("local")
        .file("Cargo.toml", r#"
            [package]
            name = "local"
            version = "0.0.1"
            authors = []

            [dependencies]
            foo = { version = "1", stdlib = true }
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
[ERROR] no matching package named `foo` found (required by `local`)
location searched: registry file://[..]
version required: ^1
"));
}

#[test]
fn unresolved_implicit_stdlib_deps() {
    setup();
    Package::new("core", "1.0.0").local(true).publish();

    let p = project("local")
        .file("Cargo.toml", r#"
            [package]
            name = "local"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/lib.rs", "");

    assert_that(p.cargo_process("build").arg("--verbose"),
                execs().with_status(101)
                .with_stderr_contains(
                    "[WARNING] the \"compiler source\" is unstable [..]")
                .with_stderr_contains("\
[ERROR] no matching package named `std` found (required by `local`)
location searched: registry file://[..]
version required: ^1.0
"));
}


#[test]
fn explicit_stdlib_deps_with_flag() {
    setup();
    Package::new("core", "1.0.0").local(true).publish();
    Package::new("std", "1.0.0").local(true).file("src/lib.rs", STD).publish();
    Package::new("test", "1.0.0").local(true).publish();

    let p = project("local")
        .file("Cargo.toml", r#"
            [package]
            name = "local"
            version = "0.0.1"
            authors = []
            implicit-dependencies = false

            [dependencies]
            core = { version = "1", stdlib = true }
            std = { version = "1", stdlib = true }
            test = { version = "1", stdlib = true }
        "#)
        .file("src/lib.rs", "");

    assert_that(p.cargo_process("build").arg("--verbose"),
                execs().with_status(0)
                .with_stderr_contains(
                    "[WARNING] the \"compiler source\" is unstable [..]")
                .with_stderr_contains(
                    "[WARNING] explicit dependencies are unstable"));
}

#[test]
fn implicit_stdlib_dep_with_flag() {
    setup();
    Package::new("core", "1.0.0").local(true).publish();
    Package::new("std", "1.0.0").local(true).file("src/lib.rs", STD).publish();
    Package::new("test", "1.0.0").local(true).publish();

    let p = project("local")
        .file("Cargo.toml", r#"
            [package]
            name = "local"
            version = "0.0.1"
            authors = []
            implicit-dependencies = true
        "#)
        .file("src/lib.rs", "");

    assert_that(p.cargo_process("build").arg("--verbose"),
                execs().with_status(0)
                .with_stderr_contains(
                    "[WARNING] the \"compiler source\" is unstable [..]"));
}

#[test]
fn no_primary_stdlib_deps_at_all() {
    setup();
    // For dev & build
    Package::new("core", "1.0.0")
        .file("src/lib.rs", "I AM INVALID SYNTAX CANNOT COMPILE")
        .local(true).publish();
    Package::new("std", "1.0.0").local(true).publish();
    Package::new("test", "1.0.0").local(true).publish();

    let foo = project("foo")
    .file("Cargo.toml", r#"
        [package]
        name = "foo"
        version = "0.0.0"
        authors = []
        implicit-dependencies = false
    "#)
    .file("src/lib.rs", "");
    assert_that(foo.cargo_process("build").arg("-v"),
                execs().with_status(0));
}

#[test]
fn mixed_expicit_and_implicit_stdlib_deps() {
    setup();
    let foo = project("foo")
    .file("Cargo.toml", r#"
        [package]
        name = "foo"
        version = "0.0.0"
        authors = []
        implicit-dependencies = true

        [dependencies]
        foo = { stdlib = true }
    "#)
    .file("src/lib.rs", "");
    assert_that(foo.cargo_process("build").arg("-v"),
                execs().with_status(101).with_stderr("\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  cannot use explicit stdlib deps when implicit deps were explicitly enabled.
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


#[test]
fn override_implicit_deps() {
    setup();
    Package::new("not-wanted", "0.0.1").local(true).publish();
    let foo = project("asdf")
    .file("Cargo.toml", r#"
        [package]
        name = "local"
        version = "0.0.0"
        authors = []
    "#)
    .file("src/lib.rs", "")
    .file(".cargo/config", r#"
        [custom-implicit-stdlib-dependencies]
        dependencies       = [ "foo" ]
        dev-dependencies   = [ ]
        build-dependencies = [ ]
    "#);
    assert_that(foo.cargo_process("build").arg("-v"),
                execs().with_status(101)
                .with_stderr_contains(
                    "[WARNING] the \"compiler source\" is unstable [..]")
                .with_stderr_contains(
                    "[WARNING] the `keep-stdlib-dependencies` config key is unstable")
                .with_stderr_contains(
                    "[WARNING] the `custom-implicit-stdlib-dependencies` config key is unstable")
                .with_stderr_contains("\
[ERROR] no matching package named `foo` found (required by `local`)
location searched: registry file://[..]
version required: ^1.0
"));
}
