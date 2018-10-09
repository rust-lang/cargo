use cargo::core::Workspace;
use cargo::util::{config::Config, errors::ManifestError};

use support::project;

/// Tests inclusion of a `ManifestError` pointing to a member manifest
/// when that manifest fails to deserialize.
#[test]
fn toml_deserialize_manifest_error() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
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
            [project]
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

    let manifest_errs: Vec<_> = error
        .iter_chain()
        .filter_map(|err| err.downcast_ref::<ManifestError>())
        .map(|err| err.manifest_path())
        .collect();

    assert_eq!(manifest_errs.len(), 2, "{:?}", manifest_errs);
    assert_eq!(manifest_errs[0], &root_manifest_path);
    assert_eq!(manifest_errs[1], &member_manifest_path);
}

/// Tests inclusion of a `ManifestError` pointing to a member manifest
/// when that manifest has an invalid dependency path.
#[test]
fn member_manifest_path_io_error() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
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
            [project]
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

    let manifest_errs: Vec<_> = error
        .iter_chain()
        .filter_map(|err| err.downcast_ref::<ManifestError>())
        .map(|err| err.manifest_path())
        .collect();

    assert_eq!(manifest_errs.len(), 3, "{:?}", manifest_errs);
    assert_eq!(manifest_errs[0], &root_manifest_path);
    assert_eq!(manifest_errs[1], &member_manifest_path);
    assert_eq!(manifest_errs[2], &missing_manifest_path);
}
