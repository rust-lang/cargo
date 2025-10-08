//! Tests for `paths` overrides.

use crate::prelude::*;
use cargo_test_support::registry::Package;
use cargo_test_support::str;
use cargo_test_support::{basic_manifest, project};

#[cargo_test]
fn broken_path_override_warns() {
    Package::new("bar", "0.1.0").publish();
    Package::new("bar", "0.2.0").publish();

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
                a = { path = "a1" }
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "a1/Cargo.toml",
            r#"
                [package]
                name = "a"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies]
                bar = "0.1"
            "#,
        )
        .file("a1/src/lib.rs", "")
        .file(
            "a2/Cargo.toml",
            r#"
                [package]
                name = "a"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies]
                bar = "0.2"
            "#,
        )
        .file("a2/src/lib.rs", "")
        .file(".cargo/config.toml", r#"paths = ["a2"]"#)
        .build();

    p.cargo("check")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 2 packages to latest compatible versions
[WARNING] path override for crate `a` has altered the original list of
dependencies; the dependency on `bar` was either added or
modified to not match the previously resolved version

This is currently allowed but is known to produce buggy behavior with spurious
recompiles and changes to the crate graph. Path overrides unfortunately were
never intended to support this feature, so for now this message is just a
warning. In the future, however, this message will become a hard error.

To change the dependency graph via an override it's recommended to use the
`[patch]` feature of Cargo instead of the path override feature. This is
documented online at the url below for more information.

https://doc.rust-lang.org/cargo/reference/overriding-dependencies.html

[DOWNLOADING] crates ...
[DOWNLOADED] bar v0.2.0 (registry `dummy-registry`)
[CHECKING] bar v0.2.0
[CHECKING] a v0.0.1 ([ROOT]/foo/a2)
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn override_to_path_dep() {
    Package::new("bar", "0.1.0").dep("baz", "0.1").publish();
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
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.0.1"
                authors = []

                [dependencies]
                baz = { path = "baz" }
            "#,
        )
        .file("bar/src/lib.rs", "")
        .file("bar/baz/Cargo.toml", &basic_manifest("baz", "0.0.1"))
        .file("bar/baz/src/lib.rs", "")
        .file(".cargo/config.toml", r#"paths = ["bar"]"#)
        .build();

    p.cargo("check").run();
}

#[cargo_test]
fn paths_ok_with_optional() {
    Package::new("baz", "0.1.0").publish();

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
                bar = { path = "bar" }
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.1.0"
                edition = "2015"
                authors = []

                [dependencies]
                baz = { version = "0.1", optional = true }
            "#,
        )
        .file("bar/src/lib.rs", "")
        .file(
            "bar2/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.1.0"
                edition = "2015"
                authors = []

                [dependencies]
                baz = { version = "0.1", optional = true }
            "#,
        )
        .file("bar2/src/lib.rs", "")
        .file(".cargo/config.toml", r#"paths = ["bar2"]"#)
        .build();

    p.cargo("check")
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to latest compatible version
[CHECKING] bar v0.1.0 ([ROOT]/foo/bar2)
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn paths_add_optional_bad() {
    Package::new("baz", "0.1.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2021"
                authors = []

                [dependencies]
                bar = { path = "bar" }
            "#,
        )
        .file("src/lib.rs", "")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "")
        .file(
            "bar2/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.1.0"
                edition = "2021"
                authors = []

                [dependencies]
                baz = { version = "0.1", optional = true }
            "#,
        )
        .file("bar2/src/lib.rs", "")
        .file(".cargo/config.toml", r#"paths = ["bar2"]"#)
        .build();

    p.cargo("check")
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to latest compatible version
[WARNING] path override for crate `bar` has altered the original list of
dependencies; the dependency on `baz` was either added or
modified to not match the previously resolved version

This is currently allowed but is known to produce buggy behavior with spurious
recompiles and changes to the crate graph. Path overrides unfortunately were
never intended to support this feature, so for now this message is just a
warning. In the future, however, this message will become a hard error.

To change the dependency graph via an override it's recommended to use the
`[patch]` feature of Cargo instead of the path override feature. This is
documented online at the url below for more information.

https://doc.rust-lang.org/cargo/reference/overriding-dependencies.html

[CHECKING] bar v0.1.0 ([ROOT]/foo/bar2)
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn env_paths_overrides_not_supported() {
    Package::new("file", "0.1.0").publish();
    Package::new("cli", "0.1.0").publish();
    Package::new("env", "0.1.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                edition = "2015"

                [dependencies]
                file = "0.1.0"
                cli = "0.1.0"
                env = "0.1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .file("file/Cargo.toml", &basic_manifest("file", "0.2.0"))
        .file("file/src/lib.rs", "")
        .file("cli/Cargo.toml", &basic_manifest("cli", "0.2.0"))
        .file("cli/src/lib.rs", "")
        .file("env/Cargo.toml", &basic_manifest("env", "0.2.0"))
        .file("env/src/lib.rs", "")
        .file(".cargo/config.toml", r#"paths = ["file"]"#)
        .build();

    p.cargo("check")
        .arg("--config")
        .arg("paths=['cli']")
        // paths overrides ignore env
        .env("CARGO_PATHS", "env")
        .with_stderr_data(
            str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 3 packages to latest compatible versions
[DOWNLOADING] crates ...
[DOWNLOADED] env v0.1.0 (registry `dummy-registry`)
[CHECKING] file v0.2.0 ([ROOT]/foo/file)
[CHECKING] cli v0.2.0 ([ROOT]/foo/cli)
[CHECKING] env v0.1.0
[CHECKING] foo v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]
            .unordered(),
        )
        .run();
}
