//! Tests for how a dependency's `Cargo.toml` is reported, per source kind.
//!
//! Each source kind gets the same three manifests: one with unused keys, one
//! that fails to deserialize, and one that fails to parse.

use std::fs;

use crate::prelude::*;
use cargo_test_support::registry::Package;
use cargo_test_support::{Project, git, paths, project, str, t};

const UNUSED_KEYS: &str = r#"
[package]
name = "bar"
version = "0.0.1"
edition = "2015"
unused-key = true

[unused-table]
key = 1
"#;

const INVALID_TYPE: &str = r#"
[package]
name = "bar"
version = "0.0.1"
edition = 2015
"#;

const INVALID_SYNTAX: &str = r#"
[package]
name = "bar"
version = "0.0.1
"#;

fn foo_depending_on(dep: &str) -> Project {
    project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"
                    version = "0.0.1"
                    edition = "2015"

                    [dependencies]
                    bar = {dep}
                "#
            ),
        )
        .file("src/lib.rs", "extern crate bar; pub use bar::f;")
        .build()
}

fn registry(manifest: &str) -> Project {
    Package::new("bar", "0.0.1")
        .file("Cargo.toml", manifest)
        .file("src/lib.rs", "pub fn f() {}")
        .publish();
    foo_depending_on(r#""0.0.1""#)
}

fn git(manifest: &str) -> Project {
    let bar = git::new("bar", |p| {
        p.file("Cargo.toml", manifest)
            .file("src/lib.rs", "pub fn f() {}")
    });
    foo_depending_on(&format!("{{ git = '{}' }}", bar.url()))
}

fn local_registry(manifest: &str) -> Project {
    let root = paths::root();
    t!(fs::create_dir(&root.join(".cargo")));
    t!(fs::write(
        root.join(".cargo/config.toml"),
        r#"
            [source.crates-io]
            registry = 'https://wut'
            replace-with = 'my-local-registry'

            [source.my-local-registry]
            local-registry = 'registry'
        "#
    ));
    Package::new("bar", "0.0.1")
        .local(true)
        .file("Cargo.toml", manifest)
        .file("src/lib.rs", "pub fn f() {}")
        .publish();
    foo_depending_on(r#""0.0.1""#)
}

fn directory(manifest: &str) -> Project {
    let root = paths::root();
    t!(fs::create_dir(&root.join(".cargo")));
    t!(fs::write(
        root.join(".cargo/config.toml"),
        r#"
            [source.crates-io]
            replace-with = 'my-vendor'

            [source.my-vendor]
            directory = 'vendor'
        "#
    ));
    project()
        .at("vendor/bar")
        .file("Cargo.toml", manifest)
        .file("src/lib.rs", "pub fn f() {}")
        .file(".cargo-checksum.json", r#"{"package":null,"files":{}}"#)
        .build();
    foo_depending_on(r#""0.0.1""#)
}

#[cargo_test]
fn registry_unused_keys() {
    let p = registry(UNUSED_KEYS);
    p.cargo("check")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to highest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] bar v0.0.1 (registry `dummy-registry`)
[CHECKING] bar v0.0.1
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn registry_invalid_type() {
    let p = registry(INVALID_TYPE);
    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to highest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] bar v0.0.1 (registry `dummy-registry`)
[ERROR] invalid type: integer `2015`, expected a string or workspace
 --> ../home/.cargo/registry/src/-[HASH]/bar-0.0.1/Cargo.toml:5:11
  |
5 | edition = 2015
  |           ^^^^
[ERROR] failed to download replaced source registry `crates-io`

"#]])
        .run();
}

#[cargo_test]
fn registry_invalid_syntax() {
    let p = registry(INVALID_SYNTAX);
    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to highest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] bar v0.0.1 (registry `dummy-registry`)
[ERROR] invalid basic string, expected `"`
 --> ../home/.cargo/registry/src/-[HASH]/bar-0.0.1/Cargo.toml:4:17
  |
4 | version = "0.0.1
  |                 ^
[ERROR] failed to download replaced source registry `crates-io`

"#]])
        .run();
}

#[cargo_test]
fn git_unused_keys() {
    let p = git(UNUSED_KEYS);
    p.cargo("check")
        .with_stderr_data(str![[r#"
[UPDATING] git repository `[ROOTURL]/bar`
[LOCKING] 1 package to highest compatible version
[CHECKING] bar v0.0.1 ([ROOTURL]/bar#[..])
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn git_invalid_type() {
    let p = git(INVALID_TYPE);
    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] git repository `[ROOTURL]/bar`
[ERROR] invalid type: integer `2015`, expected a string or workspace
 --> ../home/.cargo/git/checkouts/bar-[HASH]/[..]/Cargo.toml:5:11
  |
5 | edition = 2015
  |           ^^^^
[ERROR] failed to get `bar` as a dependency of package `foo v0.0.1 ([ROOT]/foo)`

Caused by:
  failed to load source for dependency `bar`

Caused by:
  unable to update [ROOTURL]/bar

"#]])
        .run();
}

#[cargo_test]
fn git_invalid_syntax() {
    let p = git(INVALID_SYNTAX);
    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] git repository `[ROOTURL]/bar`
[ERROR] invalid basic string, expected `"`
 --> ../home/.cargo/git/checkouts/bar-[HASH]/[..]/Cargo.toml:4:17
  |
4 | version = "0.0.1
  |                 ^
[ERROR] failed to get `bar` as a dependency of package `foo v0.0.1 ([ROOT]/foo)`

Caused by:
  failed to load source for dependency `bar`

Caused by:
  unable to update [ROOTURL]/bar

"#]])
        .run();
}

#[cargo_test]
fn local_registry_unused_keys() {
    let p = local_registry(UNUSED_KEYS);
    p.cargo("check")
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to highest compatible version
[UNPACKING] bar v0.0.1 (registry `[ROOT]/registry`)
[CHECKING] bar v0.0.1
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn local_registry_invalid_type() {
    let p = local_registry(INVALID_TYPE);
    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to highest compatible version
[UNPACKING] bar v0.0.1 (registry `[ROOT]/registry`)
[ERROR] invalid type: integer `2015`, expected a string or workspace
 --> ../home/.cargo/registry/src/-[HASH]/bar-0.0.1/Cargo.toml:5:11
  |
5 | edition = 2015
  |           ^^^^
[ERROR] failed to download `bar v0.0.1`

Caused by:
  unable to get packages from source

Caused by:
  failed to download replaced source registry `crates-io`

"#]])
        .run();
}

#[cargo_test]
fn local_registry_invalid_syntax() {
    let p = local_registry(INVALID_SYNTAX);
    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to highest compatible version
[UNPACKING] bar v0.0.1 (registry `[ROOT]/registry`)
[ERROR] invalid basic string, expected `"`
 --> ../home/.cargo/registry/src/-[HASH]/bar-0.0.1/Cargo.toml:4:17
  |
4 | version = "0.0.1
  |                 ^
[ERROR] failed to download `bar v0.0.1`

Caused by:
  unable to get packages from source

Caused by:
  failed to download replaced source registry `crates-io`

"#]])
        .run();
}

#[cargo_test]
fn directory_unused_keys() {
    let p = directory(UNUSED_KEYS);
    p.cargo("check")
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to highest compatible version
[CHECKING] bar v0.0.1
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn directory_invalid_type() {
    let p = directory(INVALID_TYPE);
    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] invalid type: integer `2015`, expected a string or workspace
 --> ../vendor/bar/Cargo.toml:5:11
  |
5 | edition = 2015
  |           ^^^^
[ERROR] failed to get `bar` as a dependency of package `foo v0.0.1 ([ROOT]/foo)`

Caused by:
  failed to load source for dependency `bar`

Caused by:
  unable to update registry `crates-io`

Caused by:
  failed to query replaced source registry `crates-io`

"#]])
        .run();
}

#[cargo_test]
fn directory_invalid_syntax() {
    let p = directory(INVALID_SYNTAX);
    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] invalid basic string, expected `"`
 --> ../vendor/bar/Cargo.toml:4:17
  |
4 | version = "0.0.1
  |                 ^
[ERROR] failed to get `bar` as a dependency of package `foo v0.0.1 ([ROOT]/foo)`

Caused by:
  failed to load source for dependency `bar`

Caused by:
  unable to update registry `crates-io`

Caused by:
  failed to query replaced source registry `crates-io`

"#]])
        .run();
}
