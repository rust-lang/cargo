//! Tests for workspace member errors.

use cargo::core::resolver::ResolveError;
use cargo::core::{compiler::CompileMode, Shell, Workspace};
use cargo::ops::{self, CompileOptions};
use cargo::util::{config::Config, errors::ManifestError};

use cargo_test_support::install::cargo_home;
use cargo_test_support::project;
use cargo_test_support::registry;

/// Tests inclusion of a `ManifestError` pointing to a member manifest
/// when that manifest fails to deserialize.
#[cargo_test]
fn toml_deserialize_manifest_error() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                authors = []

                [dependencies]
                bar = { path = "bar" }

                [workspace]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.1.0"
                authors = []

                [dependencies]
                foobar == "0.55"
            "#,
        )
        .file("bar/src/main.rs", "fn main() {}")
        .build();

    let root_manifest_path = p.root().join("Cargo.toml");
    let member_manifest_path = p.root().join("bar").join("Cargo.toml");

    let error = Workspace::new(&root_manifest_path, &Config::default().unwrap()).unwrap_err();
    eprintln!("{:?}", error);

    let manifest_err: &ManifestError = error.downcast_ref().expect("Not a ManifestError");
    assert_eq!(manifest_err.manifest_path(), &root_manifest_path);

    let causes: Vec<_> = manifest_err.manifest_causes().collect();
    assert_eq!(causes.len(), 1, "{:?}", causes);
    assert_eq!(causes[0].manifest_path(), &member_manifest_path);
}

/// Tests inclusion of a `ManifestError` pointing to a member manifest
/// when that manifest has an invalid dependency path.
#[cargo_test]
fn member_manifest_path_io_error() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                authors = []

                [dependencies]
                bar = { path = "bar" }

                [workspace]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.1.0"
                authors = []

                [dependencies]
                foobar = { path = "nosuch" }
            "#,
        )
        .file("bar/src/main.rs", "fn main() {}")
        .build();

    let root_manifest_path = p.root().join("Cargo.toml");
    let member_manifest_path = p.root().join("bar").join("Cargo.toml");
    let missing_manifest_path = p.root().join("bar").join("nosuch").join("Cargo.toml");

    let error = Workspace::new(&root_manifest_path, &Config::default().unwrap()).unwrap_err();
    eprintln!("{:?}", error);

    let manifest_err: &ManifestError = error.downcast_ref().expect("Not a ManifestError");
    assert_eq!(manifest_err.manifest_path(), &root_manifest_path);

    let causes: Vec<_> = manifest_err.manifest_causes().collect();
    assert_eq!(causes.len(), 2, "{:?}", causes);
    assert_eq!(causes[0].manifest_path(), &member_manifest_path);
    assert_eq!(causes[1].manifest_path(), &missing_manifest_path);
}

/// Tests dependency version errors provide which package failed via a `ResolveError`.
#[cargo_test]
fn member_manifest_version_error() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                authors = []

                [dependencies]
                bar = { path = "bar" }

                [workspace]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.1.0"
                authors = []

                [dependencies]
                i-dont-exist = "0.55"
            "#,
        )
        .file("bar/src/main.rs", "fn main() {}")
        .build();

    // Prevent this test from accessing the network by setting up .cargo/config.
    registry::init();
    let config = Config::new(
        Shell::from_write(Box::new(Vec::new())),
        cargo_home(),
        cargo_home(),
    );
    let ws = Workspace::new(&p.root().join("Cargo.toml"), &config).unwrap();
    let compile_options = CompileOptions::new(&config, CompileMode::Build).unwrap();
    let member_bar = ws.members().find(|m| &*m.name() == "bar").unwrap();

    let error = ops::compile(&ws, &compile_options).map(|_| ()).unwrap_err();
    eprintln!("{:?}", error);

    let resolve_err: &ResolveError = error.downcast_ref().expect("Not a ResolveError");
    let package_path = resolve_err.package_path();
    assert_eq!(package_path.len(), 1, "package_path: {:?}", package_path);
    assert_eq!(package_path[0], member_bar.package_id());
}
