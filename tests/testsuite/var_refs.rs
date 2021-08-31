//! Tests for variable references, e.g. { path = "${FOO}/bar/foo" }

use cargo_test_support::project;

#[cargo_test]
fn simple() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
        [workspace]
        members = ["utils/zoo", "bar"]
        "#,
        )
        .file(
            "utils/zoo/Cargo.toml",
            r#"
        [package]
        name = "zoo"
        version = "1.0.0"
        edition = "2018"
        authors = []

        [lib]
    "#,
        )
        .file(
            "utils/zoo/src/lib.rs",
            r#"
        pub fn hello() { println!("Hello, world!"); }
    "#,
        )
        .file(
            "bar/Cargo.toml",
            r#"
        cargo-features = ["expand-env-vars"]

        [package]
        name = "bar"
        version = "1.0.0"
        edition = "2018"
        authors = []

        [dependencies]
        zoo = { path = "${UTILS_ROOT}/zoo" }
    "#,
        )
        .file(
            "bar/src/main.rs",
            r#"
            fn main() {
                zoo::hello();
            }
            "#,
        )
        .build();

    p.cargo("build")
        .masquerade_as_nightly_cargo()
        .cwd("bar")
        .env("UTILS_ROOT", "../utils")
        .run();
    assert!(p.bin("bar").is_file());
}

#[cargo_test]
fn basic_with_default() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
        [workspace]
        members = ["utils/zoo", "bar"]
        "#,
        )
        .file(
            "utils/zoo/Cargo.toml",
            r#"
        [package]
        name = "zoo"
        version = "1.0.0"
        edition = "2018"
        authors = []

        [lib]
    "#,
        )
        .file(
            "utils/zoo/src/lib.rs",
            r#"
        pub fn hello() { println!("Hello, world!"); }
    "#,
        )
        .file(
            "bar/Cargo.toml",
            r#"
        cargo-features = ["expand-env-vars"]

        [package]
        name = "bar"
        version = "1.0.0"
        edition = "2018"
        authors = []

        [dependencies]
        zoo = { path = "${UTILS_ROOT?../utils}/zoo" }
    "#,
        )
        .file(
            "bar/src/main.rs",
            r#"
            fn main() {
                zoo::hello();
            }
            "#,
        )
        .build();

    // Note: UTILS_ROOT is not set in the environment.
    p.cargo("build")
        .masquerade_as_nightly_cargo()
        .arg("-Zunstable-options")
        .arg("-Zexpand-env-vars")
        .cwd("bar")
        .run();
    assert!(p.bin("bar").is_file());
}

#[cargo_test]
fn missing_features() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
        [package]
        name = "foo"
        version = "1.0.0"
        edition = "2018"

        [lib]

        [dependencies]
        utils = { path = "${UTILS_ROOT}/utils" }
    "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build")
        .masquerade_as_nightly_cargo()
        .arg("-Zexpand-env-vars")
        .with_status(101)
        .with_stderr_contains("[..]this manifest uses environment variable references [..] but has not specified `cargo-features = [\"expand-env-vars\"]`.[..]")
        .run();
}

#[cargo_test]
fn var_not_set() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
        cargo-features = ["expand-env-vars"]

        [package]
        name = "foo"
        version = "1.0.0"
        edition = "2018"
        authors = []

        [dependencies]
        bar = { path = "${BAD_VAR}/bar" }
        "#,
        )
        .file("src/lib.rs", r#""#)
        .build();

    p.cargo("build")
        .masquerade_as_nightly_cargo()
        .with_status(101)
        .with_stderr_contains("[..]environment variable 'BAD_VAR' is not set[..]")
        .run();
}

#[cargo_test]
fn bad_syntax() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
        cargo-features = ["expand-env-vars"]

        [package]
        name = "foo"
        version = "1.0.0"
        edition = "2018"
        authors = []

        [dependencies]
        bar = { path = "${BAD_VAR" }
        "#,
        )
        .file("src/lib.rs", r#""#)
        .build();

    p.cargo("build")
        .masquerade_as_nightly_cargo()
        .with_status(101)
        .with_stderr_contains("[..]environment variable reference is missing closing brace.[..]")
        .run();
}
