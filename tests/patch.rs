#[macro_use]
extern crate cargotest;
extern crate hamcrest;
extern crate toml;

use std::fs::{self, File};
use std::io::{Read, Write};

use cargotest::support::git;
use cargotest::support::paths;
use cargotest::support::registry::Package;
use cargotest::support::{execs, project};
use hamcrest::assert_that;

#[test]
fn replace() {
    Package::new("foo", "0.1.0").publish();
    Package::new("deep-foo", "0.1.0")
        .file("src/lib.rs", r#"
            extern crate foo;
            pub fn deep() {
                foo::foo();
            }
        "#)
        .dep("foo", "0.1.0")
        .publish();

    let p = project("bar")
        .file("Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.0.1"
            authors = []

            [dependencies]
            foo = "0.1.0"
            deep-foo = "0.1.0"

            [patch.crates-io]
            foo = { path = "foo" }
        "#)
        .file("src/lib.rs", "
            extern crate foo;
            extern crate deep_foo;
            pub fn bar() {
                foo::foo();
                deep_foo::deep();
            }
        ")
        .file("foo/Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []
        "#)
        .file("foo/src/lib.rs", r#"
            pub fn foo() {}
        "#);

    assert_that(p.cargo_process("build"),
                execs().with_status(0).with_stderr("\
[UPDATING] registry `file://[..]`
[DOWNLOADING] deep-foo v0.1.0 ([..])
[COMPILING] foo v0.1.0 (file://[..])
[COMPILING] deep-foo v0.1.0
[COMPILING] bar v0.0.1 (file://[..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
"));

    assert_that(p.cargo("build"),//.env("RUST_LOG", "trace"),
                execs().with_status(0).with_stderr("[FINISHED] [..]"));
}

#[test]
fn nonexistent() {
    Package::new("baz", "0.1.0").publish();

    let p = project("bar")
        .file("Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.0.1"
            authors = []

            [dependencies]
            foo = "0.1.0"

            [patch.crates-io]
            foo = { path = "foo" }
        "#)
        .file("src/lib.rs", "
            extern crate foo;
            pub fn bar() {
                foo::foo();
            }
        ")
        .file("foo/Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []
        "#)
        .file("foo/src/lib.rs", r#"
            pub fn foo() {}
        "#);

    assert_that(p.cargo_process("build"),
                execs().with_status(0).with_stderr("\
[UPDATING] registry `file://[..]`
[COMPILING] foo v0.1.0 (file://[..])
[COMPILING] bar v0.0.1 (file://[..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
"));
    assert_that(p.cargo("build"),
                execs().with_status(0).with_stderr("[FINISHED] [..]"));
}

#[test]
fn patch_git() {
    let foo = git::repo(&paths::root().join("override"))
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []
        "#)
        .file("src/lib.rs", "");
    foo.build();

    let p = project("bar")
        .file("Cargo.toml", &format!(r#"
            [package]
            name = "bar"
            version = "0.0.1"
            authors = []

            [dependencies]
            foo = {{ git = '{}' }}

            [patch.'{0}']
            foo = {{ path = "foo" }}
        "#, foo.url()))
        .file("src/lib.rs", "
            extern crate foo;
            pub fn bar() {
                foo::foo();
            }
        ")
        .file("foo/Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []
        "#)
        .file("foo/src/lib.rs", r#"
            pub fn foo() {}
        "#);

    assert_that(p.cargo_process("build"),
                execs().with_status(0).with_stderr("\
[UPDATING] git repository `file://[..]`
[COMPILING] foo v0.1.0 (file://[..])
[COMPILING] bar v0.0.1 (file://[..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
"));
    assert_that(p.cargo("build"),
                execs().with_status(0).with_stderr("[FINISHED] [..]"));
}

#[test]
fn patch_to_git() {
    let foo = git::repo(&paths::root().join("override"))
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []
        "#)
        .file("src/lib.rs", "pub fn foo() {}");
    foo.build();

    Package::new("foo", "0.1.0").publish();

    let p = project("bar")
        .file("Cargo.toml", &format!(r#"
            [package]
            name = "bar"
            version = "0.0.1"
            authors = []

            [dependencies]
            foo = "0.1"

            [patch.crates-io]
            foo = {{ git = '{}' }}
        "#, foo.url()))
        .file("src/lib.rs", "
            extern crate foo;
            pub fn bar() {
                foo::foo();
            }
        ");

    assert_that(p.cargo_process("build"),//.env("RUST_LOG", "cargo=trace"),
                execs().with_status(0).with_stderr("\
[UPDATING] git repository `file://[..]`
[UPDATING] registry `file://[..]`
[COMPILING] foo v0.1.0 (file://[..])
[COMPILING] bar v0.0.1 (file://[..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
"));
    assert_that(p.cargo("build"),
                execs().with_status(0).with_stderr("[FINISHED] [..]"));
}

#[test]
fn unused() {
    Package::new("foo", "0.1.0").publish();

    let p = project("bar")
        .file("Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.0.1"
            authors = []

            [dependencies]
            foo = "0.1.0"

            [patch.crates-io]
            foo = { path = "foo" }
        "#)
        .file("src/lib.rs", "")
        .file("foo/Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.2.0"
            authors = []
        "#)
        .file("foo/src/lib.rs", r#"
            not rust code
        "#);

    assert_that(p.cargo_process("build"),
                execs().with_status(0).with_stderr("\
[UPDATING] registry `file://[..]`
[DOWNLOADING] foo v0.1.0 [..]
[COMPILING] foo v0.1.0
[COMPILING] bar v0.0.1 (file://[..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
"));
    assert_that(p.cargo("build"),
                execs().with_status(0).with_stderr("[FINISHED] [..]"));

    // unused patch should be in the lock file
    let mut lock = String::new();
    File::open(p.root().join("Cargo.lock")).unwrap()
        .read_to_string(&mut lock).unwrap();
    let toml: toml::Value = toml::from_str(&lock).unwrap();
    assert_eq!(toml["patch"]["unused"].as_array().unwrap().len(), 1);
    assert_eq!(toml["patch"]["unused"][0]["name"].as_str(), Some("foo"));
    assert_eq!(toml["patch"]["unused"][0]["version"].as_str(), Some("0.2.0"));
}

#[test]
fn unused_git() {
    Package::new("foo", "0.1.0").publish();

    let foo = git::repo(&paths::root().join("override"))
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.2.0"
            authors = []
        "#)
        .file("src/lib.rs", "");
    foo.build();

    let p = project("bar")
        .file("Cargo.toml", &format!(r#"
            [package]
            name = "bar"
            version = "0.0.1"
            authors = []

            [dependencies]
            foo = "0.1"

            [patch.crates-io]
            foo = {{ git = '{}' }}
        "#, foo.url()))
        .file("src/lib.rs", "");

    assert_that(p.cargo_process("build"),
                execs().with_status(0).with_stderr("\
[UPDATING] git repository `file://[..]`
[UPDATING] registry `file://[..]`
[DOWNLOADING] foo v0.1.0 [..]
[COMPILING] foo v0.1.0
[COMPILING] bar v0.0.1 (file://[..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
"));
    assert_that(p.cargo("build"),
                execs().with_status(0).with_stderr("[FINISHED] [..]"));
}

#[test]
fn add_patch() {
    Package::new("foo", "0.1.0").publish();

    let p = project("bar")
        .file("Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.0.1"
            authors = []

            [dependencies]
            foo = "0.1.0"
        "#)
        .file("src/lib.rs", "")
        .file("foo/Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []
        "#)
        .file("foo/src/lib.rs", r#""#);

    assert_that(p.cargo_process("build"),
                execs().with_status(0).with_stderr("\
[UPDATING] registry `file://[..]`
[DOWNLOADING] foo v0.1.0 [..]
[COMPILING] foo v0.1.0
[COMPILING] bar v0.0.1 (file://[..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
"));
    assert_that(p.cargo("build"),
                execs().with_status(0).with_stderr("[FINISHED] [..]"));

    t!(t!(File::create(p.root().join("Cargo.toml"))).write_all(br#"
            [package]
            name = "bar"
            version = "0.0.1"
            authors = []

            [dependencies]
            foo = "0.1.0"

            [patch.crates-io]
            foo = { path = 'foo' }
    "#));

    assert_that(p.cargo("build"),
                execs().with_status(0).with_stderr("\
[COMPILING] foo v0.1.0 (file://[..])
[COMPILING] bar v0.0.1 (file://[..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
"));
    assert_that(p.cargo("build"),
                execs().with_status(0).with_stderr("[FINISHED] [..]"));
}

#[test]
fn add_ignored_patch() {
    Package::new("foo", "0.1.0").publish();

    let p = project("bar")
        .file("Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.0.1"
            authors = []

            [dependencies]
            foo = "0.1.0"
        "#)
        .file("src/lib.rs", "")
        .file("foo/Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.1.1"
            authors = []
        "#)
        .file("foo/src/lib.rs", r#""#);

    assert_that(p.cargo_process("build"),
                execs().with_status(0).with_stderr("\
[UPDATING] registry `file://[..]`
[DOWNLOADING] foo v0.1.0 [..]
[COMPILING] foo v0.1.0
[COMPILING] bar v0.0.1 (file://[..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
"));
    assert_that(p.cargo("build"),
                execs().with_status(0).with_stderr("[FINISHED] [..]"));

    t!(t!(File::create(p.root().join("Cargo.toml"))).write_all(br#"
            [package]
            name = "bar"
            version = "0.0.1"
            authors = []

            [dependencies]
            foo = "0.1.0"

            [patch.crates-io]
            foo = { path = 'foo' }
    "#));

    assert_that(p.cargo("build"),
                execs().with_status(0).with_stderr("\
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
"));
    assert_that(p.cargo("build"),
                execs().with_status(0).with_stderr("[FINISHED] [..]"));
}

#[test]
fn new_minor() {
    Package::new("foo", "0.1.0").publish();

    let p = project("bar")
        .file("Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.0.1"
            authors = []

            [dependencies]
            foo = "0.1.0"

            [patch.crates-io]
            foo = { path = 'foo' }
        "#)
        .file("src/lib.rs", "")
        .file("foo/Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.1.1"
            authors = []
        "#)
        .file("foo/src/lib.rs", r#""#);

    assert_that(p.cargo_process("build"),
                execs().with_status(0).with_stderr("\
[UPDATING] registry `file://[..]`
[COMPILING] foo v0.1.1 [..]
[COMPILING] bar v0.0.1 (file://[..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
"));
}

#[test]
fn transitive_new_minor() {
    Package::new("foo", "0.1.0").publish();

    let p = project("bar")
        .file("Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.0.1"
            authors = []

            [dependencies]
            subdir = { path = 'subdir' }

            [patch.crates-io]
            foo = { path = 'foo' }
        "#)
        .file("src/lib.rs", "")
        .file("subdir/Cargo.toml", r#"
            [package]
            name = "subdir"
            version = "0.1.0"
            authors = []

            [dependencies]
            foo = '0.1.0'
        "#)
        .file("subdir/src/lib.rs", r#""#)
        .file("foo/Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.1.1"
            authors = []
        "#)
        .file("foo/src/lib.rs", r#""#);

    assert_that(p.cargo_process("build"),
                execs().with_status(0).with_stderr("\
[UPDATING] registry `file://[..]`
[COMPILING] foo v0.1.1 [..]
[COMPILING] subdir v0.1.0 [..]
[COMPILING] bar v0.0.1 (file://[..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
"));
}

#[test]
fn new_major() {
    Package::new("foo", "0.1.0").publish();

    let p = project("bar")
        .file("Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.0.1"
            authors = []

            [dependencies]
            foo = "0.2.0"

            [patch.crates-io]
            foo = { path = 'foo' }
        "#)
        .file("src/lib.rs", "")
        .file("foo/Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.2.0"
            authors = []
        "#)
        .file("foo/src/lib.rs", r#""#);

    assert_that(p.cargo_process("build"),
                execs().with_status(0).with_stderr("\
[UPDATING] registry `file://[..]`
[COMPILING] foo v0.2.0 [..]
[COMPILING] bar v0.0.1 (file://[..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
"));

    Package::new("foo", "0.2.0").publish();
    assert_that(p.cargo("update"),
                execs().with_status(0));
    assert_that(p.cargo("build"),
                execs().with_status(0).with_stderr("\
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
"));

    t!(t!(File::create(p.root().join("Cargo.toml"))).write_all(br#"
            [package]
            name = "bar"
            version = "0.0.1"
            authors = []

            [dependencies]
            foo = "0.2.0"
    "#));
    assert_that(p.cargo("build"),
                execs().with_status(0).with_stderr("\
[UPDATING] registry `file://[..]`
[DOWNLOADING] foo v0.2.0 [..]
[COMPILING] foo v0.2.0
[COMPILING] bar v0.0.1 (file://[..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
"));
}

#[test]
fn transitive_new_major() {
    Package::new("foo", "0.1.0").publish();

    let p = project("bar")
        .file("Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.0.1"
            authors = []

            [dependencies]
            subdir = { path = 'subdir' }

            [patch.crates-io]
            foo = { path = 'foo' }
        "#)
        .file("src/lib.rs", "")
        .file("subdir/Cargo.toml", r#"
            [package]
            name = "subdir"
            version = "0.1.0"
            authors = []

            [dependencies]
            foo = '0.2.0'
        "#)
        .file("subdir/src/lib.rs", r#""#)
        .file("foo/Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.2.0"
            authors = []
        "#)
        .file("foo/src/lib.rs", r#""#);

    assert_that(p.cargo_process("build"),
                execs().with_status(0).with_stderr("\
[UPDATING] registry `file://[..]`
[COMPILING] foo v0.2.0 [..]
[COMPILING] subdir v0.1.0 [..]
[COMPILING] bar v0.0.1 (file://[..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
"));
}

#[test]
fn remove_patch() {
    Package::new("foo", "0.1.0").publish();
    Package::new("bar", "0.1.0").publish();

    let p = project("bar")
        .file("Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.0.1"
            authors = []

            [dependencies]
            foo = "0.1"

            [patch.crates-io]
            foo = { path = 'foo' }
            bar = { path = 'bar' }
        "#)
        .file("src/lib.rs", "")
        .file("foo/Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []
        "#)
        .file("foo/src/lib.rs", r#""#)
        .file("bar/Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.1.0"
            authors = []
        "#)
        .file("bar/src/lib.rs", r#""#);

    // Generate a lock file where `bar` is unused
    assert_that(p.cargo_process("build"), execs().with_status(0));
    let mut lock_file1 = String::new();
    File::open(p.root().join("Cargo.lock")).unwrap()
        .read_to_string(&mut lock_file1).unwrap();

    // Remove `bar` and generate a new lock file form the old one
    File::create(p.root().join("Cargo.toml")).unwrap().write_all(r#"
        [package]
        name = "bar"
        version = "0.0.1"
        authors = []

        [dependencies]
        foo = "0.1"

        [patch.crates-io]
        foo = { path = 'foo' }
    "#.as_bytes()).unwrap();
    assert_that(p.cargo("build"), execs().with_status(0));
    let mut lock_file2 = String::new();
    File::open(p.root().join("Cargo.lock")).unwrap()
        .read_to_string(&mut lock_file2).unwrap();

    // Remove the lock file and build from scratch
    fs::remove_file(p.root().join("Cargo.lock")).unwrap();
    assert_that(p.cargo("build"), execs().with_status(0));
    let mut lock_file3 = String::new();
    File::open(p.root().join("Cargo.lock")).unwrap()
        .read_to_string(&mut lock_file3).unwrap();

    assert!(lock_file1.contains("bar"));
    assert_eq!(lock_file2, lock_file3);
    assert!(lock_file1 != lock_file2);
}

#[test]
fn non_crates_io() {
    Package::new("foo", "0.1.0").publish();

    let p = project("bar")
        .file("Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.0.1"
            authors = []

            [patch.some-other-source]
            foo = { path = 'foo' }
        "#)
        .file("src/lib.rs", "")
        .file("foo/Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []
        "#)
        .file("foo/src/lib.rs", r#""#);

    assert_that(p.cargo_process("build"),
                execs().with_status(101)
                       .with_stderr("\
error: failed to parse manifest at `[..]`

Caused by:
  invalid url `some-other-source`: relative URL without a base
"));
}

#[test]
fn replace_with_crates_io() {
    Package::new("foo", "0.1.0").publish();

    let p = project("bar")
        .file("Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.0.1"
            authors = []

            [patch.crates-io]
            foo = "0.1"
        "#)
        .file("src/lib.rs", "")
        .file("foo/Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []
        "#)
        .file("foo/src/lib.rs", r#""#);

    assert_that(p.cargo_process("build"),
                execs().with_status(101)
                       .with_stderr("\
[UPDATING] [..]
error: failed to resolve patches for `[..]`

Caused by:
  patch for `foo` in `[..]` points to the same source, but patches must point \
  to different sources
"));
}

#[test]
fn patch_in_virtual() {
    Package::new("foo", "0.1.0").publish();

    let p = project("bar")
        .file("Cargo.toml", r#"
            [workspace]
            members = ["bar"]

            [patch.crates-io]
            foo = { path = "foo" }
        "#)
        .file("foo/Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []
        "#)
        .file("foo/src/lib.rs", r#""#)
        .file("bar/Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.1.0"
            authors = []

            [dependencies]
            foo = "0.1"
        "#)
        .file("bar/src/lib.rs", r#""#);

    assert_that(p.cargo_process("build"),
                execs().with_status(0));
    assert_that(p.cargo("build"),
                execs().with_status(0).with_stderr("\
[FINISHED] [..]
"));
}
