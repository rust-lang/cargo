//! Tests for `paths` overrides.

use cargo_test_support::registry::Package;
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
                authors = []

                [dependencies]
                bar = "0.2"
            "#,
        )
        .file("a2/src/lib.rs", "")
        .file(".cargo/config", r#"paths = ["a2"]"#)
        .build();

    p.cargo("check")
        .with_stderr(
            "\
[UPDATING] [..]
warning: path override for crate `a` has altered the original list of
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
[DOWNLOADED] [..]
[CHECKING] [..]
[CHECKING] [..]
[CHECKING] [..]
[FINISHED] [..]
",
        )
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
        .file(".cargo/config", r#"paths = ["bar"]"#)
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
                authors = []

                [dependencies]
                baz = { version = "0.1", optional = true }
            "#,
        )
        .file("bar2/src/lib.rs", "")
        .file(".cargo/config", r#"paths = ["bar2"]"#)
        .build();

    p.cargo("check")
        .with_stderr(
            "\
[CHECKING] bar v0.1.0 ([..]bar2)
[CHECKING] foo v0.0.1 ([..])
[FINISHED] [..]
",
        )
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
                authors = []

                [dependencies]
                baz = { version = "0.1", optional = true }
            "#,
        )
        .file("bar2/src/lib.rs", "")
        .file(".cargo/config", r#"paths = ["bar2"]"#)
        .build();

    p.cargo("check")
        .with_stderr_contains(
            "\
warning: path override for crate `bar` has altered the original list of
dependencies; the dependency on `baz` was either added or\
",
        )
        .run();
}
