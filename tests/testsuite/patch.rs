//! Tests for `[patch]` table source replacement.

use std::fs::{self, File};
use std::io::{Read, Write};

use cargo_test_support::git;
use cargo_test_support::paths;
use cargo_test_support::registry::Package;
use cargo_test_support::{basic_manifest, project, t};

#[cargo_test]
fn replace() {
    Package::new("bar", "0.1.0").publish();
    Package::new("baz", "0.1.0")
        .file(
            "src/lib.rs",
            "extern crate bar; pub fn baz() { bar::bar(); }",
        )
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

    p.cargo("build")
        .with_stderr(
            "\
[UPDATING] `[ROOT][..]` index
[DOWNLOADING] crates ...
[DOWNLOADED] baz v0.1.0 ([..])
[COMPILING] bar v0.1.0 ([CWD]/bar)
[COMPILING] baz v0.1.0
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();

    p.cargo("build").with_stderr("[FINISHED] [..]").run();
}

#[cargo_test]
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
        .file(
            "src/lib.rs",
            "extern crate bar; pub fn foo() { bar::bar(); }",
        )
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "pub fn bar() {}")
        .build();

    p.cargo("build")
        .with_stderr(
            "\
[UPDATING] `[ROOT][..]` index
[COMPILING] bar v0.1.0 ([CWD]/bar)
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
    p.cargo("build").with_stderr("[FINISHED] [..]").run();
}

#[cargo_test]
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
        .file(
            "src/lib.rs",
            "extern crate bar; pub fn foo() { bar::bar(); }",
        )
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "pub fn bar() {}")
        .build();

    p.cargo("build")
        .with_stderr(
            "\
[UPDATING] git repository `file://[..]`
[COMPILING] bar v0.1.0 ([CWD]/bar)
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
    p.cargo("build").with_stderr("[FINISHED] [..]").run();
}

#[cargo_test]
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
        .file(
            "src/lib.rs",
            "extern crate bar; pub fn foo() { bar::bar(); }",
        )
        .build();

    p.cargo("build")
        .with_stderr(
            "\
[UPDATING] git repository `file://[..]`
[UPDATING] `[ROOT][..]` index
[COMPILING] bar v0.1.0 (file://[..])
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
    p.cargo("build").with_stderr("[FINISHED] [..]").run();
}

#[cargo_test]
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

    p.cargo("build")
        .with_stderr(
            "\
[UPDATING] `[ROOT][..]` index
[WARNING] Patch `bar v0.2.0 ([CWD]/bar)` was not used in the crate graph.
[..]
[..]
[..]
[..]
[DOWNLOADING] crates ...
[DOWNLOADED] bar v0.1.0 [..]
[COMPILING] bar v0.1.0
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
    p.cargo("build")
        .with_stderr(
            "\
[WARNING] Patch `bar v0.2.0 ([CWD]/bar)` was not used in the crate graph.
[..]
[..]
[..]
[..]
[FINISHED] [..]
",
        )
        .run();

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

#[cargo_test]
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

    p.cargo("build")
        .with_stderr(
            "\
[UPDATING] git repository `file://[..]`
[UPDATING] `[ROOT][..]` index
[WARNING] Patch `bar v0.2.0 ([..])` was not used in the crate graph.
[..]
[..]
[..]
[..]
[DOWNLOADING] crates ...
[DOWNLOADED] bar v0.1.0 [..]
[COMPILING] bar v0.1.0
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
    p.cargo("build")
        .with_stderr(
            "\
[WARNING] Patch `bar v0.2.0 ([..])` was not used in the crate graph.
[..]
[..]
[..]
[..]
[FINISHED] [..]
",
        )
        .run();
}

#[cargo_test]
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

    p.cargo("build")
        .with_stderr(
            "\
[UPDATING] `[ROOT][..]` index
[DOWNLOADING] crates ...
[DOWNLOADED] bar v0.1.0 [..]
[COMPILING] bar v0.1.0
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
    p.cargo("build").with_stderr("[FINISHED] [..]").run();

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

    p.cargo("build")
        .with_stderr(
            "\
[COMPILING] bar v0.1.0 ([CWD]/bar)
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
    p.cargo("build").with_stderr("[FINISHED] [..]").run();
}

#[cargo_test]
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

    p.cargo("build")
        .with_stderr(
            "\
[UPDATING] `[ROOT][..]` index
[DOWNLOADING] crates ...
[DOWNLOADED] bar v0.1.0 [..]
[COMPILING] bar v0.1.0
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
    p.cargo("build").with_stderr("[FINISHED] [..]").run();

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

    p.cargo("build")
        .with_stderr(
            "\
[WARNING] Patch `bar v0.1.1 ([CWD]/bar)` was not used in the crate graph.
[..]
[..]
[..]
[..]
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]",
        )
        .run();
    p.cargo("build")
        .with_stderr(
            "\
[WARNING] Patch `bar v0.1.1 ([CWD]/bar)` was not used in the crate graph.
[..]
[..]
[..]
[..]
[FINISHED] [..]",
        )
        .run();

    p.cargo("update").run();
    p.cargo("build")
        .with_stderr(
            "\
[COMPILING] bar v0.1.1 ([CWD]/bar)
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] dev [..]
",
        )
        .run();
}

#[cargo_test]
fn no_warn_ws_patch() {
    Package::new("c", "0.1.0").publish();

    // Don't issue an unused patch warning when the patch isn't used when
    // partially building a workspace.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["a", "b", "c"]

            [patch.crates-io]
            c = { path = "c" }
        "#,
        )
        .file("a/Cargo.toml", &basic_manifest("a", "0.1.0"))
        .file("a/src/lib.rs", "")
        .file(
            "b/Cargo.toml",
            r#"
            [package]
            name = "b"
            version = "0.1.0"
            [dependencies]
            c = "0.1.0"
        "#,
        )
        .file("b/src/lib.rs", "")
        .file("c/Cargo.toml", &basic_manifest("c", "0.1.0"))
        .file("c/src/lib.rs", "")
        .build();

    p.cargo("build -p a")
        .with_stderr(
            "\
[UPDATING] [..]
[COMPILING] a [..]
[FINISHED] [..]",
        )
        .run();
}

#[cargo_test]
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

    p.cargo("build")
        .with_stderr(
            "\
[UPDATING] `[ROOT][..]` index
[COMPILING] bar v0.1.1 [..]
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cargo_test]
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

    p.cargo("build")
        .with_stderr(
            "\
[UPDATING] `[ROOT][..]` index
[COMPILING] baz v0.1.1 [..]
[COMPILING] bar v0.1.0 [..]
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cargo_test]
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

    p.cargo("build")
        .with_stderr(
            "\
[UPDATING] `[ROOT][..]` index
[COMPILING] bar v0.2.0 [..]
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();

    Package::new("bar", "0.2.0").publish();
    p.cargo("update").run();
    p.cargo("build")
        .with_stderr("[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]")
        .run();

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
    p.cargo("build")
        .with_stderr(
            "\
[UPDATING] `[ROOT][..]` index
[DOWNLOADING] crates ...
[DOWNLOADED] bar v0.2.0 [..]
[COMPILING] bar v0.2.0
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cargo_test]
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

    p.cargo("build")
        .with_stderr(
            "\
[UPDATING] `[ROOT][..]` index
[COMPILING] baz v0.2.0 [..]
[COMPILING] bar v0.1.0 [..]
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cargo_test]
fn shared_by_transitive() {
    Package::new("baz", "0.1.1").publish();

    let baz = git::repo(&paths::root().join("override"))
        .file("Cargo.toml", &basic_manifest("baz", "0.1.2"))
        .file("src/lib.rs", "")
        .build();

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"
                    version = " 0.1.0"

                    [dependencies]
                    bar = {{ path = "bar" }}
                    baz = "0.1"

                    [patch.crates-io]
                    baz = {{ git = "{}", version = "0.1" }}
                "#,
                baz.url(),
            ),
        )
        .file("src/lib.rs", "")
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.1.0"

                [dependencies]
                baz = "0.1.1"
            "#,
        )
        .file("bar/src/lib.rs", "")
        .build();

    p.cargo("build")
        .with_stderr(
            "\
[UPDATING] git repository `file://[..]`
[UPDATING] `[ROOT][..]` index
[COMPILING] baz v0.1.2 [..]
[COMPILING] bar v0.1.0 [..]
[COMPILING] foo v0.1.0 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cargo_test]
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
    p.cargo("build").run();
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
    p.cargo("build").run();
    let mut lock_file2 = String::new();
    File::open(p.root().join("Cargo.lock"))
        .unwrap()
        .read_to_string(&mut lock_file2)
        .unwrap();

    // Remove the lock file and build from scratch
    fs::remove_file(p.root().join("Cargo.lock")).unwrap();
    p.cargo("build").run();
    let mut lock_file3 = String::new();
    File::open(p.root().join("Cargo.lock"))
        .unwrap()
        .read_to_string(&mut lock_file3)
        .unwrap();

    assert!(lock_file1.contains("foo"));
    assert_eq!(lock_file2, lock_file3);
    assert_ne!(lock_file1, lock_file2);
}

#[cargo_test]
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

    p.cargo("build")
        .with_status(101)
        .with_stderr(
            "\
error: failed to parse manifest at `[..]`

Caused by:
  [patch] entry `some-other-source` should be a URL or registry name

Caused by:
  invalid url `some-other-source`: relative URL without a base
",
        )
        .run();
}

#[cargo_test]
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

    p.cargo("build")
        .with_status(101)
        .with_stderr(
            "\
[UPDATING] [..]
error: failed to resolve patches for `[..]`

Caused by:
  patch for `bar` in `[..]` points to the same source, but patches must point \
  to different sources
",
        )
        .run();
}

#[cargo_test]
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

    p.cargo("build").run();
    p.cargo("build").with_stderr("[FINISHED] [..]").run();
}

#[cargo_test]
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

    p.cargo("build").run();

    // Nothing should be rebuilt, no registry should be updated.
    p.cargo("build").with_stderr("[FINISHED] [..]").run();
}

#[cargo_test]
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
        .file(
            "bar/src/main.rs",
            "extern crate baz; fn main() { baz::baz() }",
        )
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

    p.cargo("build").run();
}

#[cargo_test]
fn patch_older() {
    Package::new("baz", "1.0.2").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [dependencies]
                bar = { path = 'bar' }
                baz = "=1.0.1"

                [patch.crates-io]
                baz = { path = "./baz" }
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "bar/Cargo.toml",
            r#"
                [project]
                name = "bar"
                version = "0.5.0"
                authors = []

                [dependencies]
                baz = "1.0.0"
            "#,
        )
        .file("bar/src/lib.rs", "")
        .file(
            "baz/Cargo.toml",
            r#"
                [project]
                name = "baz"
                version = "1.0.1"
                authors = []
            "#,
        )
        .file("baz/src/lib.rs", "")
        .build();

    p.cargo("build")
        .with_stderr(
            "\
[UPDATING] [..]
[COMPILING] baz v1.0.1 [..]
[COMPILING] bar v0.5.0 [..]
[COMPILING] foo v0.1.0 [..]
[FINISHED] [..]
",
        )
        .run();
}

#[cargo_test]
fn cycle() {
    Package::new("a", "1.0.0").publish();
    Package::new("b", "1.0.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["a", "b"]

                [patch.crates-io]
                a = {path="a"}
                b = {path="b"}
            "#,
        )
        .file(
            "a/Cargo.toml",
            r#"
                [package]
                name = "a"
                version = "1.0.0"

                [dependencies]
                b = "1.0"
            "#,
        )
        .file("a/src/lib.rs", "")
        .file(
            "b/Cargo.toml",
            r#"
                [package]
                name = "b"
                version = "1.0.0"

                [dependencies]
                a = "1.0"
            "#,
        )
        .file("b/src/lib.rs", "")
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr(
            "\
[UPDATING] [..]
error: cyclic package dependency: [..]
package `[..]`
    ... which is depended on by `[..]`
    ... which is depended on by `[..]`
",
        )
        .run();
}

#[cargo_test]
fn multipatch() {
    Package::new("a", "1.0.0").publish();
    Package::new("a", "2.0.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"

                [dependencies]
                a1 = { version = "1", package = "a" }
                a2 = { version = "2", package = "a" }

                [patch.crates-io]
                b1 = { path = "a1", package = "a" }
                b2 = { path = "a2", package = "a" }
            "#,
        )
        .file("src/lib.rs", "pub fn foo() { a1::f1(); a2::f2(); }")
        .file(
            "a1/Cargo.toml",
            r#"
                [package]
                name = "a"
                version = "1.0.0"
            "#,
        )
        .file("a1/src/lib.rs", "pub fn f1() {}")
        .file(
            "a2/Cargo.toml",
            r#"
                [package]
                name = "a"
                version = "2.0.0"
            "#,
        )
        .file("a2/src/lib.rs", "pub fn f2() {}")
        .build();

    p.cargo("build").run();
}

#[cargo_test]
fn patch_same_version() {
    let bar = git::repo(&paths::root().join("override"))
        .file("Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("src/lib.rs", "")
        .build();

    cargo_test_support::registry::init();

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                [package]
                name = "foo"
                version = "0.0.1"
                [dependencies]
                bar = "0.1"
                [patch.crates-io]
                bar = {{ path = "bar" }}
                bar2 = {{ git = '{}', package = 'bar' }}
            "#,
                bar.url(),
            ),
        )
        .file("src/lib.rs", "")
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.1.0"
            "#,
        )
        .file("bar/src/lib.rs", "")
        .build();

    p.cargo("build")
        .with_status(101)
        .with_stderr(
            "\
[UPDATING] [..]
error: cannot have two `[patch]` entries which both resolve to `bar v0.1.0`
",
        )
        .run();
}

#[cargo_test]
fn two_semver_compatible() {
    let bar = git::repo(&paths::root().join("override"))
        .file("Cargo.toml", &basic_manifest("bar", "0.1.1"))
        .file("src/lib.rs", "")
        .build();

    cargo_test_support::registry::init();

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                [package]
                name = "foo"
                version = "0.0.1"
                [dependencies]
                bar = "0.1"
                [patch.crates-io]
                bar = {{ path = "bar" }}
                bar2 = {{ git = '{}', package = 'bar' }}
            "#,
                bar.url(),
            ),
        )
        .file("src/lib.rs", "pub fn foo() { bar::foo() }")
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.1.2"
            "#,
        )
        .file("bar/src/lib.rs", "pub fn foo() {}")
        .build();

    // assert the build succeeds and doesn't panic anywhere, and then afterwards
    // assert that the build succeeds again without updating anything or
    // building anything else.
    p.cargo("build").run();
    p.cargo("build")
        .with_stderr(
            "\
warning: Patch `bar v0.1.1 [..]` was not used in the crate graph.
Check that [..]
with the [..]
what is [..]
version. [..]
[FINISHED] [..]",
        )
        .run();
}

#[cargo_test]
fn multipatch_select_big() {
    let bar = git::repo(&paths::root().join("override"))
        .file("Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("src/lib.rs", "")
        .build();

    cargo_test_support::registry::init();

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                [package]
                name = "foo"
                version = "0.0.1"
                [dependencies]
                bar = "*"
                [patch.crates-io]
                bar = {{ path = "bar" }}
                bar2 = {{ git = '{}', package = 'bar' }}
            "#,
                bar.url(),
            ),
        )
        .file("src/lib.rs", "pub fn foo() { bar::foo() }")
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.2.0"
            "#,
        )
        .file("bar/src/lib.rs", "pub fn foo() {}")
        .build();

    // assert the build succeeds, which is only possible if 0.2.0 is selected
    // since 0.1.0 is missing the function we need. Afterwards assert that the
    // build succeeds again without updating anything or building anything else.
    p.cargo("build").run();
    p.cargo("build")
        .with_stderr(
            "\
warning: Patch `bar v0.1.0 [..]` was not used in the crate graph.
Check that [..]
with the [..]
what is [..]
version. [..]
[FINISHED] [..]",
        )
        .run();
}

#[cargo_test]
fn canonicalize_a_bunch() {
    let base = git::repo(&paths::root().join("base"))
        .file("Cargo.toml", &basic_manifest("base", "0.1.0"))
        .file("src/lib.rs", "")
        .build();

    let intermediate = git::repo(&paths::root().join("intermediate"))
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "intermediate"
                    version = "0.1.0"

                    [dependencies]
                    # Note the lack of trailing slash
                    base = {{ git = '{}' }}
                "#,
                base.url(),
            ),
        )
        .file("src/lib.rs", "pub fn f() { base::f() }")
        .build();

    let newbase = git::repo(&paths::root().join("newbase"))
        .file("Cargo.toml", &basic_manifest("base", "0.1.0"))
        .file("src/lib.rs", "pub fn f() {}")
        .build();

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"
                    version = "0.0.1"

                    [dependencies]
                    # Note the trailing slashes
                    base = {{ git = '{base}/' }}
                    intermediate = {{ git = '{intermediate}/' }}

                    [patch.'{base}'] # Note the lack of trailing slash
                    base = {{ git = '{newbase}' }}
                "#,
                base = base.url(),
                intermediate = intermediate.url(),
                newbase = newbase.url(),
            ),
        )
        .file("src/lib.rs", "pub fn a() { base::f(); intermediate::f() }")
        .build();

    // Once to make sure it actually works
    p.cargo("build").run();

    // Then a few more times for good measure to ensure no weird warnings about
    // `[patch]` are printed.
    p.cargo("build").with_stderr("[FINISHED] [..]").run();
    p.cargo("build").with_stderr("[FINISHED] [..]").run();
}
