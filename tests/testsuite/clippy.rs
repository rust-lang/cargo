//! Tests for the `cargo clippy` command.

use cargo_test_support::{command_is_available, project, registry::Package};

#[cargo_test]
// Clippy should never be considered fresh.
fn clippy_force_rebuild() {
    if !command_is_available("clippy-driver") {
        return;
    }

    Package::new("dep1", "0.1.0").publish();

    // This is just a random clippy lint (assertions_on_constants) that
    // hopefully won't change much in the future.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"

            [dependencies]
            dep1 = "0.1"
            "#,
        )
        .file("src/lib.rs", "pub fn f() { assert!(true); }")
        .build();

    p.cargo("clippy-preview -Zunstable-options -v")
        .masquerade_as_nightly_cargo()
        .with_stderr_contains("[..]assert!(true)[..]")
        .run();

    // Make sure it runs again.
    p.cargo("clippy-preview -Zunstable-options -v")
        .masquerade_as_nightly_cargo()
        .with_stderr_contains("[FRESH] dep1 v0.1.0")
        .with_stderr_contains("[..]assert!(true)[..]")
        .run();
}

#[cargo_test]
fn clippy_passes_args() {
    if !command_is_available("clippy-driver") {
        return;
    }

    // This is just a random clippy lint (assertions_on_constants) that
    // hopefully won't change much in the future.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"

            [dependencies]
            "#,
        )
        .file("src/lib.rs", "pub fn f() { assert!(true); }")
        .build();

    p.cargo("clippy-preview -Zunstable-options -v -- -Aclippy::assertions_on_constants")
        .masquerade_as_nightly_cargo()
        .with_stderr_does_not_contain("[..]assert!(true)[..]")
        .run();

    // Make sure it runs again.
    p.cargo("clippy-preview -Zunstable-options -v")
        .masquerade_as_nightly_cargo()
        .with_stderr_contains("[..]assert!(true)[..]")
        .run();
}
