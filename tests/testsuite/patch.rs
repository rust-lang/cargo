use std::fs::{self, File};
use std::io::{Read, Write};

use toml;
use support::git;
use support::paths;
use support::registry::Package;
use support::{basic_manifest, execs, project};
use support::hamcrest::assert_that;

#[test]
fn replace() {
    Package::new("bar", "0.1.0").publish();
    Package::new("baz", "0.1.0")
        .file("src/lib.rs", "extern crate bar; pub fn baz() { bar::bar(); }")
        .dep("bar", "0.1.0")
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            bar = "0.1.0"
            baz = "0.1.0"

            [patch.crates-io]
            bar = { path = "bar" }
        "#,
        )
        .file(
            "src/lib.rs",
            "
            extern crate bar;
            extern crate baz;
            pub fn bar() {
                bar::bar();
                baz::baz();
            }
        ",
        )
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "pub fn bar() {}")
        .build();

    assert_that(
        p.cargo("build"),
        execs().with_stderr(
            "\
[UPDATING] registry `file://[..]`
[DOWNLOADING] baz v0.1.0 ([..])
[COMPILING] bar v0.1.0 (file://[..])
[COMPILING] baz v0.1.0
[COMPILING] foo v0.0.1 (file://[..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        ),
    );

    assert_that(
        p.cargo("build"),
        execs().with_stderr("[FINISHED] [..]"),
    );
}

#[test]
fn nonexistent() {
    Package::new("baz", "0.1.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            bar = "0.1.0"

            [patch.crates-io]
            bar = { path = "bar" }
        "#,
        )
        .file("src/lib.rs", "extern crate bar; pub fn foo() { bar::bar(); }")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "pub fn bar() {}")
        .build();

    assert_that(
        p.cargo("build"),
        execs().with_stderr(
            "\
[UPDATING] registry `file://[..]`
[COMPILING] bar v0.1.0 (file://[..])
[COMPILING] foo v0.0.1 (file://[..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        ),
    );
    assert_that(
        p.cargo("build"),
        execs().with_stderr("[FINISHED] [..]"),
    );
}

#[test]
fn patch_git() {
    let bar = git::repo(&paths::root().join("override"))
        .file("Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("src/lib.rs", "")
        .build();

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            bar = {{ git = '{}' }}

            [patch.'{0}']
            bar = {{ path = "bar" }}
        "#,
                bar.url()
            ),
        )
        .file("src/lib.rs", "extern crate bar; pub fn foo() { bar::bar(); }")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "pub fn bar() {}")
        .build();

    assert_that(
        p.cargo("build"),
        execs().with_stderr(
            "\
[UPDATING] git repository `file://[..]`
[COMPILING] bar v0.1.0 (file://[..])
[COMPILING] foo v0.0.1 (file://[..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        ),
    );
    assert_that(
        p.cargo("build"),
        execs().with_stderr("[FINISHED] [..]"),
    );
}

#[test]
fn patch_to_git() {
    let bar = git::repo(&paths::root().join("override"))
        .file("Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("src/lib.rs", "pub fn bar() {}")
        .build();

    Package::new("bar", "0.1.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            bar = "0.1"

            [patch.crates-io]
            bar = {{ git = '{}' }}
        "#,
                bar.url()
            ),
        )
        .file("src/lib.rs", "extern crate bar; pub fn foo() { bar::bar(); }")
        .build();

    assert_that(
        p.cargo("build"),
        execs().with_stderr(
            "\
[UPDATING] git repository `file://[..]`
[UPDATING] registry `file://[..]`
[COMPILING] bar v0.1.0 (file://[..])
[COMPILING] foo v0.0.1 (file://[..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        ),
    );
    assert_that(
        p.cargo("build"),
        execs().with_stderr("[FINISHED] [..]"),
    );
}

#[test]
fn unused() {
    Package::new("bar", "0.1.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            bar = "0.1.0"

            [patch.crates-io]
            bar = { path = "bar" }
        "#,
        )
        .file("src/lib.rs", "")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.2.0"))
        .file("bar/src/lib.rs", "not rust code")
        .build();

    assert_that(
        p.cargo("build"),
        execs().with_stderr(
            "\
[UPDATING] registry `file://[..]`
[DOWNLOADING] bar v0.1.0 [..]
[COMPILING] bar v0.1.0
[COMPILING] foo v0.0.1 (file://[..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        ),
    );
    assert_that(
        p.cargo("build"),
        execs().with_stderr("[FINISHED] [..]"),
    );

    // unused patch should be in the lock file
    let mut lock = String::new();
    File::open(p.root().join("Cargo.lock"))
        .unwrap()
        .read_to_string(&mut lock)
        .unwrap();
    let toml: toml::Value = toml::from_str(&lock).unwrap();
    assert_eq!(toml["patch"]["unused"].as_array().unwrap().len(), 1);
    assert_eq!(toml["patch"]["unused"][0]["name"].as_str(), Some("bar"));
    assert_eq!(
        toml["patch"]["unused"][0]["version"].as_str(),
        Some("0.2.0")
    );
}

#[test]
fn unused_git() {
    Package::new("bar", "0.1.0").publish();

    let foo = git::repo(&paths::root().join("override"))
        .file("Cargo.toml", &basic_manifest("bar", "0.2.0"))
        .file("src/lib.rs", "")
        .build();

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            bar = "0.1"

            [patch.crates-io]
            bar = {{ git = '{}' }}
        "#,
                foo.url()
            ),
        )
        .file("src/lib.rs", "")
        .build();

    assert_that(
        p.cargo("build"),
        execs().with_stderr(
            "\
[UPDATING] git repository `file://[..]`
[UPDATING] registry `file://[..]`
[DOWNLOADING] bar v0.1.0 [..]
[COMPILING] bar v0.1.0
[COMPILING] foo v0.0.1 (file://[..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        ),
    );
    assert_that(
        p.cargo("build"),
        execs().with_stderr("[FINISHED] [..]"),
    );
}

#[test]
fn add_patch() {
    Package::new("bar", "0.1.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            bar = "0.1.0"
        "#,
        )
        .file("src/lib.rs", "")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", r#""#)
        .build();

    assert_that(
        p.cargo("build"),
        execs().with_stderr(
            "\
[UPDATING] registry `file://[..]`
[DOWNLOADING] bar v0.1.0 [..]
[COMPILING] bar v0.1.0
[COMPILING] foo v0.0.1 (file://[..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        ),
    );
    assert_that(
        p.cargo("build"),
        execs().with_stderr("[FINISHED] [..]"),
    );

    t!(t!(File::create(p.root().join("Cargo.toml"))).write_all(
        br#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            bar = "0.1.0"

            [patch.crates-io]
            bar = { path = 'bar' }
    "#
    ));

    assert_that(
        p.cargo("build"),
        execs().with_stderr(
            "\
[COMPILING] bar v0.1.0 (file://[..])
[COMPILING] foo v0.0.1 (file://[..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        ),
    );
    assert_that(
        p.cargo("build"),
        execs().with_stderr("[FINISHED] [..]"),
    );
}

#[test]
fn add_ignored_patch() {
    Package::new("bar", "0.1.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            bar = "0.1.0"
        "#,
        )
        .file("src/lib.rs", "")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.1"))
        .file("bar/src/lib.rs", r#""#)
        .build();

    assert_that(
        p.cargo("build"),
        execs().with_stderr(
            "\
[UPDATING] registry `file://[..]`
[DOWNLOADING] bar v0.1.0 [..]
[COMPILING] bar v0.1.0
[COMPILING] foo v0.0.1 (file://[..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        ),
    );
    assert_that(
        p.cargo("build"),
        execs().with_stderr("[FINISHED] [..]"),
    );

    t!(t!(File::create(p.root().join("Cargo.toml"))).write_all(
        br#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            bar = "0.1.0"

            [patch.crates-io]
            bar = { path = 'bar' }
    "#
    ));

    assert_that(
        p.cargo("build"),
        execs()
            .with_stderr("[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]"),
    );
    assert_that(
        p.cargo("build"),
        execs().with_stderr("[FINISHED] [..]"),
    );
}

#[test]
fn new_minor() {
    Package::new("bar", "0.1.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            bar = "0.1.0"

            [patch.crates-io]
            bar = { path = 'bar' }
        "#,
        )
        .file("src/lib.rs", "")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.1"))
        .file("bar/src/lib.rs", r#""#)
        .build();

    assert_that(
        p.cargo("build"),
        execs().with_stderr(
            "\
[UPDATING] registry `file://[..]`
[COMPILING] bar v0.1.1 [..]
[COMPILING] foo v0.0.1 (file://[..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        ),
    );
}

#[test]
fn transitive_new_minor() {
    Package::new("baz", "0.1.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            bar = { path = 'bar' }

            [patch.crates-io]
            baz = { path = 'baz' }
        "#,
        )
        .file("src/lib.rs", "")
        .file(
            "bar/Cargo.toml",
            r#"
            [package]
            name = "bar"
            version = "0.1.0"
            authors = []

            [dependencies]
            baz = '0.1.0'
        "#,
        )
        .file("bar/src/lib.rs", r#""#)
        .file("baz/Cargo.toml", &basic_manifest("baz", "0.1.1"))
        .file("baz/src/lib.rs", r#""#)
        .build();

    assert_that(
        p.cargo("build"),
        execs().with_stderr(
            "\
[UPDATING] registry `file://[..]`
[COMPILING] baz v0.1.1 [..]
[COMPILING] bar v0.1.0 [..]
[COMPILING] foo v0.0.1 (file://[..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        ),
    );
}

#[test]
fn new_major() {
    Package::new("bar", "0.1.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            bar = "0.2.0"

            [patch.crates-io]
            bar = { path = 'bar' }
        "#,
        )
        .file("src/lib.rs", "")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.2.0"))
        .file("bar/src/lib.rs", r#""#)
        .build();

    assert_that(
        p.cargo("build"),
        execs().with_stderr(
            "\
[UPDATING] registry `file://[..]`
[COMPILING] bar v0.2.0 [..]
[COMPILING] foo v0.0.1 (file://[..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        ),
    );

    Package::new("bar", "0.2.0").publish();
    assert_that(p.cargo("update"), execs());
    assert_that(
        p.cargo("build"),
        execs()
            .with_stderr("[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]"),
    );

    t!(t!(File::create(p.root().join("Cargo.toml"))).write_all(
        br#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            bar = "0.2.0"
    "#
    ));
    assert_that(
        p.cargo("build"),
        execs().with_stderr(
            "\
[UPDATING] registry `file://[..]`
[DOWNLOADING] bar v0.2.0 [..]
[COMPILING] bar v0.2.0
[COMPILING] foo v0.0.1 (file://[..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        ),
    );
}

#[test]
fn transitive_new_major() {
    Package::new("baz", "0.1.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            bar = { path = 'bar' }

            [patch.crates-io]
            baz = { path = 'baz' }
        "#,
        )
        .file("src/lib.rs", "")
        .file(
            "bar/Cargo.toml",
            r#"
            [package]
            name = "bar"
            version = "0.1.0"
            authors = []

            [dependencies]
            baz = '0.2.0'
        "#,
        )
        .file("bar/src/lib.rs", r#""#)
        .file("baz/Cargo.toml", &basic_manifest("baz", "0.2.0"))
        .file("baz/src/lib.rs", r#""#)
        .build();

    assert_that(
        p.cargo("build"),
        execs().with_stderr(
            "\
[UPDATING] registry `file://[..]`
[COMPILING] baz v0.2.0 [..]
[COMPILING] bar v0.1.0 [..]
[COMPILING] foo v0.0.1 (file://[..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        ),
    );
}

#[test]
fn remove_patch() {
    Package::new("foo", "0.1.0").publish();
    Package::new("bar", "0.1.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            bar = "0.1"

            [patch.crates-io]
            foo = { path = 'foo' }
            bar = { path = 'bar' }
        "#,
        )
        .file("src/lib.rs", "")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", r#""#)
        .file("foo/Cargo.toml", &basic_manifest("foo", "0.1.0"))
        .file("foo/src/lib.rs", r#""#)
        .build();

    // Generate a lock file where `foo` is unused
    assert_that(p.cargo("build"), execs());
    let mut lock_file1 = String::new();
    File::open(p.root().join("Cargo.lock"))
        .unwrap()
        .read_to_string(&mut lock_file1)
        .unwrap();

    // Remove `foo` and generate a new lock file form the old one
    File::create(p.root().join("Cargo.toml"))
        .unwrap()
        .write_all(
            br#"
        [package]
        name = "foo"
        version = "0.0.1"
        authors = []

        [dependencies]
        bar = "0.1"

        [patch.crates-io]
        bar = { path = 'bar' }
    "#,
        )
        .unwrap();
    assert_that(p.cargo("build"), execs());
    let mut lock_file2 = String::new();
    File::open(p.root().join("Cargo.lock"))
        .unwrap()
        .read_to_string(&mut lock_file2)
        .unwrap();

    // Remove the lock file and build from scratch
    fs::remove_file(p.root().join("Cargo.lock")).unwrap();
    assert_that(p.cargo("build"), execs());
    let mut lock_file3 = String::new();
    File::open(p.root().join("Cargo.lock"))
        .unwrap()
        .read_to_string(&mut lock_file3)
        .unwrap();

    assert!(lock_file1.contains("foo"));
    assert_eq!(lock_file2, lock_file3);
    assert_ne!(lock_file1, lock_file2);
}

#[test]
fn non_crates_io() {
    Package::new("bar", "0.1.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [patch.some-other-source]
            bar = { path = 'bar' }
        "#,
        )
        .file("src/lib.rs", "")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", r#""#)
        .build();

    assert_that(
        p.cargo("build"),
        execs().with_status(101).with_stderr(
            "\
error: failed to parse manifest at `[..]`

Caused by:
  invalid url `some-other-source`: relative URL without a base
",
        ),
    );
}

#[test]
fn replace_with_crates_io() {
    Package::new("bar", "0.1.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [patch.crates-io]
            bar = "0.1"
        "#,
        )
        .file("src/lib.rs", "")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", r#""#)
        .build();

    assert_that(
        p.cargo("build"),
        execs().with_status(101).with_stderr(
            "\
[UPDATING] [..]
error: failed to resolve patches for `[..]`

Caused by:
  patch for `bar` in `[..]` points to the same source, but patches must point \
  to different sources
",
        ),
    );
}

#[test]
fn patch_in_virtual() {
    Package::new("bar", "0.1.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["foo"]

            [patch.crates-io]
            bar = { path = "bar" }
        "#,
        )
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", r#""#)
        .file(
            "foo/Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []

            [dependencies]
            bar = "0.1"
        "#,
        )
        .file("foo/src/lib.rs", r#""#)
        .build();

    assert_that(p.cargo("build"), execs());
    assert_that(
        p.cargo("build"),
        execs().with_stderr("[FINISHED] [..]"),
    );
}

#[test]
fn patch_depends_on_another_patch() {
    Package::new("bar", "0.1.0")
        .file("src/lib.rs", "broken code")
        .publish();

    Package::new("baz", "0.1.0")
        .dep("bar", "0.1")
        .file("src/lib.rs", "broken code")
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            authors = []
            version = "0.1.0"

            [dependencies]
            bar = "0.1"
            baz = "0.1"

            [patch.crates-io]
            bar = { path = "bar" }
            baz = { path = "baz" }
        "#,
        )
        .file("src/lib.rs", "")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.1"))
        .file("bar/src/lib.rs", r#""#)
        .file(
            "baz/Cargo.toml",
            r#"
            [package]
            name = "baz"
            version = "0.1.1"
            authors = []

            [dependencies]
            bar = "0.1"
        "#,
        )
        .file("baz/src/lib.rs", r#""#)
        .build();

    assert_that(p.cargo("build"), execs());

    // Nothing should be rebuilt, no registry should be updated.
    assert_that(
        p.cargo("build"),
        execs().with_stderr("[FINISHED] [..]"),
    );
}

#[test]
fn replace_prerelease() {
    Package::new("baz", "1.1.0-pre.1").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["bar"]

            [patch.crates-io]
            baz = { path = "./baz" }
        "#,
        )
        .file(
            "bar/Cargo.toml",
            r#"
            [project]
            name = "bar"
            version = "0.5.0"
            authors = []

            [dependencies]
            baz = "1.1.0-pre.1"
        "#,
        )
        .file("bar/src/main.rs", "extern crate baz; fn main() { baz::baz() }")
        .file(
            "baz/Cargo.toml",
            r#"
            [project]
            name = "baz"
            version = "1.1.0-pre.1"
            authors = []
            [workspace]
        "#,
        )
        .file("baz/src/lib.rs", "pub fn baz() {}")
        .build();

    assert_that(p.cargo("build"), execs());
}
