//! Tests for targets with `min-rust-version`.

use cargo_test_support::project;
use cargo_test_support::registry::Package;

#[cargo_test]
fn min_rust_version_satisfied() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
            min-rust-version = "1.1.1"

            [[bin]]
            name = "foo"
        "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("build").run();
    p.cargo("build --ignore-min-rust-version").run();
}

#[cargo_test]
fn min_rust_version_too_high() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
            min-rust-version = "1.9876.0"

            [[bin]]
            name = "foo"
        "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("build")
        .with_status(101)
        .with_stderr(
            "\
error: package foo requires rust version 1.9876.0 or greater (currently have [..])
",
        )
        .run();
    p.cargo("build --ignore-min-rust-version").run();
}

#[cargo_test]
fn min_rust_version_pre_release_ignored() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
            min-rust-version = "1.2.3-nightly"

            [[bin]]
            name = "foo"
        "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("build")
        .with_status(0)
        .with_stderr(
            "\
warning: pre-release part of min-rust-version ([AlphaNumeric(\"nightly\")]) is ignored.
   Compiling foo v0.0.1 ([..])
    Finished dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
    p.cargo("build --ignore-min-rust-version").run();
}

#[cargo_test]
fn min_rust_version_local_dependency_fails() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.bar]
            path = "../bar"
            #[dependencies.baz]
            #path = "../baz"
        "#,
        )
        .file("src/main.rs", "fn main(){}")
        .build();
    let _bar = project()
        .at("bar")
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "bar"
            version = "0.0.1"
            authors = []
            min-rust-version = "1.2345.0"
        "#,
        )
        .file("src/lib.rs", "fn other_stuff(){}")
        .build();

    p.cargo("build")
        .with_status(101)
        .with_stderr(
            "\
error: failed to select a version for the requirement `bar = \"*\"`
  which would be compatible with current rust version of [..]
  candidate versions found which didn't match: 0.0.1 (rust>=1.2345.0)
  location searched: [..]
required by package `foo v0.0.1 ([..])`
",
        )
        .run();
    p.cargo("build --ignore-min-rust-version").run();
}

#[cargo_test]
fn min_rust_version_registry() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
            min-rust-version = "1.0.0"

            [dependencies]
            bar = "0.1"
        "#,
        )
        .file("src/lib.rs", "fn stuff(){}")
        .build();

    Package::new("bar", "0.1.0")
        .min_rust_version(Some("1.987.0".to_string()))
        .publish();

    p.cargo("build")
        .with_status(101)
        .with_stderr(
            "    \
    Updating `[..]` index
error: failed to select a version for the requirement `bar = \"^0.1\"`
  which would be compatible with current rust version of [..]
  candidate versions found which didn't match: 0.1.0 (rust>=1.987.0)
  location searched: [..]
required by package `foo v0.0.1 ([..])`
perhaps a crate was updated and forgotten to be re-vendored?
",
        )
        .run();
    p.cargo("build --ignore-min-rust-version").run();
}

#[cargo_test]
fn min_rust_version_registry_dependency_resolution() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
            min-rust-version = "1.0.0"

            [dependencies]
            bar = "0.1"
        "#,
        )
        .file("src/lib.rs", "fn stuff(){}")
        .build();

    Package::new("bar", "0.1.0")
        .min_rust_version(Some("1.0.0".to_string()))
        .publish();
    Package::new("bar", "0.1.1")
        .min_rust_version(Some("1.987.0".to_string()))
        .publish();

    p.cargo("build").run();
    p.cargo("build --ignore-min-rust-version").run();
}
