//! Tests for local-registry sources.

use std::fs;

use cargo_test_support::paths;
use cargo_test_support::prelude::*;
use cargo_test_support::registry::{registry_path, Package};
use cargo_test_support::{basic_manifest, project, str, t};

fn setup() {
    let root = paths::root();
    t!(fs::create_dir(&root.join(".cargo")));
    t!(fs::write(
        root.join(".cargo/config.toml"),
        r#"
            [source.crates-io]
            registry = 'https://wut'
            replace-with = 'my-awesome-local-registry'

            [source.my-awesome-local-registry]
            local-registry = 'registry'
        "#
    ));
}

#[cargo_test]
fn simple() {
    setup();
    Package::new("bar", "0.0.1")
        .local(true)
        .file("src/lib.rs", "pub fn bar() {}")
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies]
                bar = "0.0.1"
            "#,
        )
        .file(
            "src/lib.rs",
            "extern crate bar; pub fn foo() { bar::bar(); }",
        )
        .build();

    p.cargo("build")
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to latest compatible version
[UNPACKING] bar v0.0.1 (registry `[ROOT]/registry`)
[COMPILING] bar v0.0.1
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    p.cargo("build")
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    p.cargo("test").run();
}

#[cargo_test]
fn not_found() {
    setup();
    // Publish a package so that the directory hierarchy is created.
    // Note, however, that we declare a dependency on baZ.
    Package::new("bar", "0.0.1").local(true).publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies]
                baz = "0.0.1"
            "#,
        )
        .file(
            "src/lib.rs",
            "extern crate baz; pub fn foo() { baz::bar(); }",
        )
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] no matching package named `baz` found
location searched: `[ROOT]/registry` index (which is replacing registry `crates-io`)
required by package `foo v0.0.1 ([ROOT]/foo)`

"#]])
        .run();
}

#[cargo_test]
fn depend_on_yanked() {
    setup();
    Package::new("bar", "0.0.1").local(true).publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies]
                bar = "0.0.1"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    // Run cargo to create lock file.
    p.cargo("check").run();

    registry_path().join("index").join("3").rm_rf();
    Package::new("bar", "0.0.1")
        .local(true)
        .yanked(true)
        .publish();

    p.cargo("check")
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn multiple_versions() {
    setup();
    Package::new("bar", "0.0.1").local(true).publish();
    Package::new("bar", "0.1.0")
        .local(true)
        .file("src/lib.rs", "pub fn bar() {}")
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies]
                bar = "*"
            "#,
        )
        .file(
            "src/lib.rs",
            "extern crate bar; pub fn foo() { bar::bar(); }",
        )
        .build();

    p.cargo("check")
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to latest compatible version
[UNPACKING] bar v0.1.0 (registry `[ROOT]/registry`)
[CHECKING] bar v0.1.0
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    Package::new("bar", "0.2.0")
        .local(true)
        .file("src/lib.rs", "pub fn bar() {}")
        .publish();

    p.cargo("update")
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to latest compatible version
[UPDATING] bar v0.1.0 -> v0.2.0

"#]])
        .run();
}

#[cargo_test]
fn multiple_names() {
    setup();
    Package::new("bar", "0.0.1")
        .local(true)
        .file("src/lib.rs", "pub fn bar() {}")
        .publish();
    Package::new("baz", "0.1.0")
        .local(true)
        .file("src/lib.rs", "pub fn baz() {}")
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies]
                bar = "*"
                baz = "*"
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
                extern crate bar;
                extern crate baz;
                pub fn foo() {
                    bar::bar();
                    baz::baz();
                }
            "#,
        )
        .build();

    p.cargo("check")
        .with_stderr_data(
            str![[r#"
[LOCKING] 2 packages to latest compatible versions
[UNPACKING] bar v0.0.1 (registry `[ROOT]/registry`)
[UNPACKING] baz v0.1.0 (registry `[ROOT]/registry`)
[CHECKING] bar v0.0.1
[CHECKING] baz v0.1.0
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test]
fn interdependent() {
    setup();
    Package::new("bar", "0.0.1")
        .local(true)
        .file("src/lib.rs", "pub fn bar() {}")
        .publish();
    Package::new("baz", "0.1.0")
        .local(true)
        .dep("bar", "*")
        .file("src/lib.rs", "extern crate bar; pub fn baz() {}")
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies]
                bar = "*"
                baz = "*"
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
                extern crate bar;
                extern crate baz;
                pub fn foo() {
                    bar::bar();
                    baz::baz();
                }
            "#,
        )
        .build();

    p.cargo("check")
        .with_stderr_data(str![[r#"
[LOCKING] 2 packages to latest compatible versions
[UNPACKING] bar v0.0.1 (registry `[ROOT]/registry`)
[UNPACKING] baz v0.1.0 (registry `[ROOT]/registry`)
[CHECKING] bar v0.0.1
[CHECKING] baz v0.1.0
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn path_dep_rewritten() {
    setup();
    Package::new("bar", "0.0.1")
        .local(true)
        .file("src/lib.rs", "pub fn bar() {}")
        .publish();
    Package::new("baz", "0.1.0")
        .local(true)
        .dep("bar", "*")
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "baz"
                version = "0.1.0"
                edition = "2015"
                authors = []

                [dependencies]
                bar = { path = "bar", version = "*" }
            "#,
        )
        .file("src/lib.rs", "extern crate bar; pub fn baz() {}")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file("bar/src/lib.rs", "pub fn bar() {}")
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies]
                bar = "*"
                baz = "*"
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
                extern crate bar;
                extern crate baz;
                pub fn foo() {
                    bar::bar();
                    baz::baz();
                }
            "#,
        )
        .build();

    p.cargo("check")
        .with_stderr_data(str![[r#"
[LOCKING] 2 packages to latest compatible versions
[UNPACKING] bar v0.0.1 (registry `[ROOT]/registry`)
[UNPACKING] baz v0.1.0 (registry `[ROOT]/registry`)
[CHECKING] bar v0.0.1
[CHECKING] baz v0.1.0
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn invalid_dir_bad() {
    setup();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies]
                bar = "*"
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            ".cargo/config.toml",
            r#"
                [source.crates-io]
                registry = 'https://wut'
                replace-with = 'my-awesome-local-directory'

                [source.my-awesome-local-directory]
                local-registry = '/path/to/nowhere'
            "#,
        )
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to get `bar` as a dependency of package `foo v0.0.1 ([ROOT]/foo)`

Caused by:
  failed to load source for dependency `bar`

Caused by:
  Unable to update registry `crates-io`

Caused by:
  failed to update replaced source registry `crates-io`

Caused by:
  local registry path is not a directory: [..]path[..]to[..]nowhere

"#]])
        .run();
}

#[cargo_test]
fn different_directory_replacing_the_registry_is_bad() {
    setup();

    // Move our test's .cargo/config to a temporary location and publish a
    // registry package we're going to use first.
    let config = paths::root().join(".cargo");
    let config_tmp = paths::root().join(".cargo-old");
    t!(fs::rename(&config, &config_tmp));

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies]
                bar = "*"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    // Generate a lock file against the crates.io registry
    Package::new("bar", "0.0.1").publish();
    p.cargo("check").run();

    // Switch back to our directory source, and now that we're replacing
    // crates.io make sure that this fails because we're replacing with a
    // different checksum
    config.rm_rf();
    t!(fs::rename(&config_tmp, &config));
    Package::new("bar", "0.0.1")
        .file("src/lib.rs", "invalid")
        .local(true)
        .publish();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] checksum for `bar v0.0.1` changed between lock files

this could be indicative of a few possible errors:

    * the lock file is corrupt
    * a replacement source in use (e.g., a mirror) returned a different checksum
    * the source itself may be corrupt in one way or another

unable to verify that `bar v0.0.1` is the same as when the lockfile was generated


"#]])
        .run();
}

#[cargo_test]
fn crates_io_registry_url_is_optional() {
    let root = paths::root();
    t!(fs::create_dir(&root.join(".cargo")));
    t!(fs::write(
        root.join(".cargo/config.toml"),
        r#"
            [source.crates-io]
            replace-with = 'my-awesome-local-registry'

            [source.my-awesome-local-registry]
            local-registry = 'registry'
        "#
    ));

    Package::new("bar", "0.0.1")
        .local(true)
        .file("src/lib.rs", "pub fn bar() {}")
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies]
                bar = "0.0.1"
            "#,
        )
        .file(
            "src/lib.rs",
            "extern crate bar; pub fn foo() { bar::bar(); }",
        )
        .build();

    p.cargo("build")
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to latest compatible version
[UNPACKING] bar v0.0.1 (registry `[ROOT]/registry`)
[COMPILING] bar v0.0.1
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    p.cargo("build")
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    p.cargo("test").run();
}
