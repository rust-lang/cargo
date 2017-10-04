#[macro_use]
extern crate cargotest;
extern crate hamcrest;

use std::fs::{self, File};
use std::io::prelude::*;

use cargotest::support::paths::{self, CargoPathExt};
use cargotest::support::registry::Package;
use cargotest::support::{project, execs};
use hamcrest::assert_that;

fn setup() {
    let root = paths::root();
    t!(fs::create_dir(&root.join(".cargo")));
    t!(t!(File::create(root.join(".cargo/config"))).write_all(br#"
        [source.crates-io]
        registry = 'https://wut'
        replace-with = 'my-awesome-local-registry'

        [source.my-awesome-local-registry]
        local-registry = 'registry'
    "#));
}

#[test]
fn simple() {
    setup();
    Package::new("foo", "0.0.1")
            .local(true)
            .file("src/lib.rs", "pub fn foo() {}")
            .publish();

    let p = project("bar")
        .file("Cargo.toml", r#"
            [project]
            name = "bar"
            version = "0.0.1"
            authors = []

            [dependencies]
            foo = "0.0.1"
        "#)
        .file("src/lib.rs", r#"
            extern crate foo;
            pub fn bar() {
                foo::foo();
            }
        "#);

    assert_that(p.cargo_process("build"),
                execs().with_status(0).with_stderr(&format!("\
[UNPACKING] foo v0.0.1 ([..])
[COMPILING] foo v0.0.1
[COMPILING] bar v0.0.1 ({dir})
[FINISHED] [..]
",
        dir = p.url())));
    assert_that(p.cargo("build"), execs().with_status(0).with_stderr("\
[FINISHED] [..]
"));
    assert_that(p.cargo("test"), execs().with_status(0));
}

#[test]
fn multiple_versions() {
    setup();
    Package::new("foo", "0.0.1").local(true).publish();
    Package::new("foo", "0.1.0")
            .local(true)
            .file("src/lib.rs", "pub fn foo() {}")
            .publish();

    let p = project("bar")
        .file("Cargo.toml", r#"
            [project]
            name = "bar"
            version = "0.0.1"
            authors = []

            [dependencies]
            foo = "*"
        "#)
        .file("src/lib.rs", r#"
            extern crate foo;
            pub fn bar() {
                foo::foo();
            }
        "#);

    assert_that(p.cargo_process("build"),
                execs().with_status(0).with_stderr(&format!("\
[UNPACKING] foo v0.1.0 ([..])
[COMPILING] foo v0.1.0
[COMPILING] bar v0.0.1 ({dir})
[FINISHED] [..]
",
        dir = p.url())));

    Package::new("foo", "0.2.0")
            .local(true)
            .file("src/lib.rs", "pub fn foo() {}")
            .publish();

    assert_that(p.cargo("update").arg("-v"),
                execs().with_status(0).with_stderr("\
[UPDATING] foo v0.1.0 -> v0.2.0
"));
}

#[test]
fn multiple_names() {
    setup();
    Package::new("foo", "0.0.1")
            .local(true)
            .file("src/lib.rs", "pub fn foo() {}")
            .publish();
    Package::new("bar", "0.1.0")
            .local(true)
            .file("src/lib.rs", "pub fn bar() {}")
            .publish();

    let p = project("local")
        .file("Cargo.toml", r#"
            [project]
            name = "local"
            version = "0.0.1"
            authors = []

            [dependencies]
            foo = "*"
            bar = "*"
        "#)
        .file("src/lib.rs", r#"
            extern crate foo;
            extern crate bar;
            pub fn local() {
                foo::foo();
                bar::bar();
            }
        "#);

    assert_that(p.cargo_process("build"),
                execs().with_status(0).with_stderr(&format!("\
[UNPACKING] [..]
[UNPACKING] [..]
[COMPILING] [..]
[COMPILING] [..]
[COMPILING] local v0.0.1 ({dir})
[FINISHED] [..]
",
        dir = p.url())));
}

#[test]
fn interdependent() {
    setup();
    Package::new("foo", "0.0.1")
            .local(true)
            .file("src/lib.rs", "pub fn foo() {}")
            .publish();
    Package::new("bar", "0.1.0")
            .local(true)
            .dep("foo", "*")
            .file("src/lib.rs", "extern crate foo; pub fn bar() {}")
            .publish();

    let p = project("local")
        .file("Cargo.toml", r#"
            [project]
            name = "local"
            version = "0.0.1"
            authors = []

            [dependencies]
            foo = "*"
            bar = "*"
        "#)
        .file("src/lib.rs", r#"
            extern crate foo;
            extern crate bar;
            pub fn local() {
                foo::foo();
                bar::bar();
            }
        "#);

    assert_that(p.cargo_process("build"),
                execs().with_status(0).with_stderr(&format!("\
[UNPACKING] [..]
[UNPACKING] [..]
[COMPILING] foo v0.0.1
[COMPILING] bar v0.1.0
[COMPILING] local v0.0.1 ({dir})
[FINISHED] [..]
",
        dir = p.url())));
}

#[test]
fn path_dep_rewritten() {
    setup();
    Package::new("foo", "0.0.1")
            .local(true)
            .file("src/lib.rs", "pub fn foo() {}")
            .publish();
    Package::new("bar", "0.1.0")
            .local(true)
            .dep("foo", "*")
            .file("Cargo.toml", r#"
                [project]
                name = "bar"
                version = "0.1.0"
                authors = []

                [dependencies]
                foo = { path = "foo", version = "*" }
            "#)
            .file("src/lib.rs", "extern crate foo; pub fn bar() {}")
            .file("foo/Cargo.toml", r#"
                [project]
                name = "foo"
                version = "0.0.1"
                authors = []
            "#)
            .file("foo/src/lib.rs", "pub fn foo() {}")
            .publish();

    let p = project("local")
        .file("Cargo.toml", r#"
            [project]
            name = "local"
            version = "0.0.1"
            authors = []

            [dependencies]
            foo = "*"
            bar = "*"
        "#)
        .file("src/lib.rs", r#"
            extern crate foo;
            extern crate bar;
            pub fn local() {
                foo::foo();
                bar::bar();
            }
        "#);

    assert_that(p.cargo_process("build"),
                execs().with_status(0).with_stderr(&format!("\
[UNPACKING] [..]
[UNPACKING] [..]
[COMPILING] foo v0.0.1
[COMPILING] bar v0.1.0
[COMPILING] local v0.0.1 ({dir})
[FINISHED] [..]
",
        dir = p.url())));
}

#[test]
fn invalid_dir_bad() {
    setup();
    let p = project("local")
        .file("Cargo.toml", r#"
            [project]
            name = "local"
            version = "0.0.1"
            authors = []

            [dependencies]
            foo = "*"
        "#)
        .file("src/lib.rs", "")
        .file(".cargo/config", r#"
            [source.crates-io]
            registry = 'https://wut'
            replace-with = 'my-awesome-local-directory'

            [source.my-awesome-local-directory]
            local-registry = '/path/to/nowhere'
        "#);


    assert_that(p.cargo_process("build"),
                execs().with_status(101).with_stderr("\
[ERROR] failed to load source for a dependency on `foo`

Caused by:
  Unable to update registry `https://[..]`

Caused by:
  failed to update replaced source registry `https://[..]`

Caused by:
  local registry path is not a directory: [..]path[..]to[..]nowhere
"));
}

#[test]
fn different_directory_replacing_the_registry_is_bad() {
    setup();

    // Move our test's .cargo/config to a temporary location and publish a
    // registry package we're going to use first.
    let config = paths::root().join(".cargo");
    let config_tmp = paths::root().join(".cargo-old");
    t!(fs::rename(&config, &config_tmp));

    let p = project("local")
        .file("Cargo.toml", r#"
            [project]
            name = "local"
            version = "0.0.1"
            authors = []

            [dependencies]
            foo = "*"
        "#)
        .file("src/lib.rs", "");
    p.build();

    // Generate a lock file against the crates.io registry
    Package::new("foo", "0.0.1").publish();
    assert_that(p.cargo("build"), execs().with_status(0));

    // Switch back to our directory source, and now that we're replacing
    // crates.io make sure that this fails because we're replacing with a
    // different checksum
    config.rm_rf();
    t!(fs::rename(&config_tmp, &config));
    Package::new("foo", "0.0.1")
            .file("src/lib.rs", "invalid")
            .local(true)
            .publish();

    assert_that(p.cargo("build"),
                execs().with_status(101).with_stderr("\
[ERROR] checksum for `foo v0.0.1` changed between lock files

this could be indicative of a few possible errors:

    * the lock file is corrupt
    * a replacement source in use (e.g. a mirror) returned a different checksum
    * the source itself may be corrupt in one way or another

unable to verify that `foo v0.0.1` is the same as when the lockfile was generated

"));
}

#[test]
fn crates_io_registry_url_is_optional() {
    let root = paths::root();
    t!(fs::create_dir(&root.join(".cargo")));
    t!(t!(File::create(root.join(".cargo/config"))).write_all(br#"
        [source.crates-io]
        replace-with = 'my-awesome-local-registry'

        [source.my-awesome-local-registry]
        local-registry = 'registry'
    "#));

    Package::new("foo", "0.0.1")
            .local(true)
            .file("src/lib.rs", "pub fn foo() {}")
            .publish();

    let p = project("bar")
        .file("Cargo.toml", r#"
            [project]
            name = "bar"
            version = "0.0.1"
            authors = []

            [dependencies]
            foo = "0.0.1"
        "#)
        .file("src/lib.rs", r#"
            extern crate foo;
            pub fn bar() {
                foo::foo();
            }
        "#);

    assert_that(p.cargo_process("build"),
                execs().with_status(0).with_stderr(&format!("\
[UNPACKING] foo v0.0.1 ([..])
[COMPILING] foo v0.0.1
[COMPILING] bar v0.0.1 ({dir})
[FINISHED] [..]
",
        dir = p.url())));
    assert_that(p.cargo("build"), execs().with_status(0).with_stderr("\
[FINISHED] [..]
"));
    assert_that(p.cargo("test"), execs().with_status(0));
}
