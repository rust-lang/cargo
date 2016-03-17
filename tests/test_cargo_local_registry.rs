use std::fs::{self, File};
use std::io::prelude::*;

use hamcrest::assert_that;

use support::{project, execs, COMPILING, UPDATING, UNPACKING, ERROR};
use support::paths::{self, CargoPathExt};
use support::registry::Package;

fn setup() {
    let root = paths::root();
    fs::create_dir(&root.join(".cargo")).unwrap();
    File::create(root.join(".cargo/config")).unwrap().write_all(br#"
        [source.crates-io]
        registry = 'https://wut'
        replace-with = 'my-awesome-local-directory'

        [source.my-awesome-local-directory]
        local-registry = 'registry'
    "#).unwrap();
}

test!(simple {
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
                execs().with_status(0).with_stdout(&format!("\
{unpacking} foo v0.0.1 ([..])
{compiling} foo v0.0.1
{compiling} bar v0.0.1 ({dir})
",
        compiling = COMPILING,
        unpacking = UNPACKING,
        dir = p.url())));
    assert_that(p.cargo("build"), execs().with_status(0).with_stdout(""));
    assert_that(p.cargo("test"), execs().with_status(0));
});

test!(multiple_versions {
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
                execs().with_status(0).with_stdout(&format!("\
{unpacking} foo v0.1.0 ([..])
{compiling} foo v0.1.0
{compiling} bar v0.0.1 ({dir})
",
        compiling = COMPILING,
        unpacking = UNPACKING,
        dir = p.url())));

    Package::new("foo", "0.2.0")
            .local(true)
            .file("src/lib.rs", "pub fn foo() {}")
            .publish();

    assert_that(p.cargo("update").arg("-v"),
                execs().with_status(0).with_stdout(&format!("\
{updating} foo v0.1.0 -> v0.2.0
",
        updating = UPDATING)));
});

test!(multiple_names {
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
                execs().with_status(0).with_stdout(&format!("\
{unpacking} [..]
{unpacking} [..]
{compiling} [..]
{compiling} [..]
{compiling} local v0.0.1 ({dir})
",
        compiling = COMPILING,
        unpacking = UNPACKING,
        dir = p.url())));
});

test!(interdependent {
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
                execs().with_status(0).with_stdout(&format!("\
{unpacking} [..]
{unpacking} [..]
{compiling} foo v0.0.1
{compiling} bar v0.1.0
{compiling} local v0.0.1 ({dir})
",
        compiling = COMPILING,
        unpacking = UNPACKING,
        dir = p.url())));
});

test!(path_dep_rewritten {
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
                execs().with_status(0).with_stdout(&format!("\
{unpacking} [..]
{unpacking} [..]
{compiling} foo v0.0.1
{compiling} bar v0.1.0
{compiling} local v0.0.1 ({dir})
",
        compiling = COMPILING,
        unpacking = UNPACKING,
        dir = p.url())));
});

test!(invalid_dir_bad {
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
                execs().with_status(101).with_stderr(&format!("\
{error} Unable to update registry https://[..]

Caused by:
  failed to update replaced source `registry https://[..]`

Caused by:
  local registry path is not a directory: [..]path[..]to[..]nowhere
", error = ERROR)));
});

test!(different_directory_replacing_the_registry_is_bad {
    // Move our test's .cargo/config to a temporary location and publish a
    // registry package we're going to use first.
    let config = paths::root().join(".cargo");
    let config_tmp = paths::root().join(".cargo-old");
    fs::rename(&config, &config_tmp).unwrap();

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
    let _ = config.rm_rf();
    fs::rename(&config_tmp, &config).unwrap();
    Package::new("foo", "0.0.1")
            .file("src/lib.rs", "invalid")
            .local(true)
            .publish();

    assert_that(p.cargo("build"),
                execs().with_status(101).with_stderr(&format!("\
{error} checksum for `foo v0.0.1` changed between lock files

this could be indicative of a few possible errors:

    * the lock file is corrupt
    * a replacement source in use (e.g. a mirror) returned a different checksum
    * the source itself may be corrupt in one way or another

unable to verify that `foo v0.0.1` was the same as before in any situation

", error = ERROR)));
});
