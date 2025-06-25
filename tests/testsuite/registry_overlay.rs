//! Tests for local-registry sources.

use crate::prelude::*;
use cargo_test_support::project;
use cargo_test_support::registry::{Package, RegistryBuilder, TestRegistry};
use cargo_test_support::str;

fn setup() -> (TestRegistry, String) {
    let alt = RegistryBuilder::new().alternative().build();
    (
        RegistryBuilder::new().http_index().build(),
        alt.index_url()
            .to_file_path()
            .unwrap()
            .into_os_string()
            .into_string()
            .unwrap(),
    )
}

#[cargo_test]
fn overlay_hit() {
    let (reg, alt_path) = setup();
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
                baz = "0.1.0"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    // baz is only in the local registry, but it gets found
    Package::new("baz", "0.1.1")
        .alternative(true)
        .local(true)
        .publish();

    p.cargo("check")
        .overlay_registry(&reg.index_url(), &alt_path)
        .run();
}

#[cargo_test]
fn registry_version_wins() {
    let (reg, alt_path) = setup();
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
                baz = "0.1.0"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    // The latest one is in the main registry, so it will get chosen.
    Package::new("baz", "0.1.1").publish();
    Package::new("baz", "0.1.0")
        .alternative(true)
        .local(true)
        .publish();

    p.cargo("check")
        .overlay_registry(&reg.index_url(), &alt_path)
        .with_stderr_data(str![[r#"
[UPDATING] `sparse+http://127.0.0.1:[..]/index/` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] baz v0.1.1 (registry `sparse+http://127.0.0.1:[..]/index/`)
[CHECKING] baz v0.1.1
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn overlay_version_wins() {
    let (reg, alt_path) = setup();
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
                baz = "0.1.0"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    // The latest one is in the overlay registry, so it will get chosen.
    Package::new("baz", "0.1.0").publish();
    Package::new("baz", "0.1.1")
        .alternative(true)
        .local(true)
        .publish();

    p.cargo("check")
        .overlay_registry(&reg.index_url(), &alt_path)
        .with_stderr_data(str![[r#"
[UPDATING] `sparse+http://127.0.0.1:[..]/index/` index
[LOCKING] 1 package to latest compatible version
[UNPACKING] baz v0.1.1 (registry `[ROOT]/alternative-registry`)
[CHECKING] baz v0.1.1
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn version_precedence() {
    let (reg, alt_path) = setup();
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
                baz = "0.1.0"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    // The one we want is in the main registry.
    Package::new("baz", "0.1.1").publish();
    Package::new("baz", "0.1.1")
        .alternative(true)
        .local(true)
        .publish();

    p.cargo("check")
        .overlay_registry(&reg.index_url(), &alt_path)
        .with_stderr_data(str![[r#"
[UPDATING] `sparse+http://127.0.0.1:[..]/index/` index
[LOCKING] 1 package to latest compatible version
[UNPACKING] baz v0.1.1 (registry `[ROOT]/alternative-registry`)
[CHECKING] baz v0.1.1
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn local_depends_on_old_registry_package() {
    let (reg, alt_path) = setup();
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
                baz = "0.1.0"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("baz", "0.0.1").publish();
    // A new local package can depend on an older version in the registry.
    Package::new("baz", "0.1.1")
        .dep("baz", "=0.0.1")
        .alternative(true)
        .local(true)
        .publish();

    p.cargo("check")
        .overlay_registry(&reg.index_url(), &alt_path)
        .run();
}

#[cargo_test]
fn registry_dep_depends_on_new_local_package() {
    let (reg, alt_path) = setup();
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
                registry-package = "0.1.0"
                workspace-package = "0.0.1"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("registry-package", "0.1.0")
        .dep("workspace-package", "0.1.0")
        .publish();
    // The local overlay contains an updated version of workspace-package
    Package::new("workspace-package", "0.1.1")
        .alternative(true)
        .local(true)
        .publish();

    // The registry contains older versions of workspace-package (one of which
    // we depend on directly).
    Package::new("workspace-package", "0.1.0").publish();
    Package::new("workspace-package", "0.0.1").publish();

    p.cargo("check")
        .overlay_registry(&reg.index_url(), &alt_path)
        .with_stderr_data(
            str![[r#"
[UPDATING] `sparse+http://127.0.0.1:[..]/index/` index
[LOCKING] 3 packages to latest compatible versions
[ADDING] workspace-package v0.0.1 (available: v0.1.1)
[DOWNLOADING] crates ...
[UNPACKING] workspace-package v0.1.1 (registry `[ROOT]/alternative-registry`)
[DOWNLOADED] registry-package v0.1.0 (registry `sparse+http://127.0.0.1:[..]/index/`)
[DOWNLOADED] workspace-package v0.0.1 (registry `sparse+http://127.0.0.1:[..]/index/`)
[CHECKING] workspace-package v0.1.1
[CHECKING] workspace-package v0.0.1
[CHECKING] registry-package v0.1.0
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]
            .unordered(),
        )
        .run();
}

// Test that we can overlay on top of alternate registries, not just crates-io.
// Since the test framework only supports a single alternate registry, we repurpose
// the dummy crates-io as the registry to overlay on top.
#[cargo_test]
fn alt_registry() {
    let alt = RegistryBuilder::new().http_index().alternative().build();
    let crates_io = RegistryBuilder::new().build();
    let crates_io_path = crates_io
        .index_url()
        .to_file_path()
        .unwrap()
        .into_os_string()
        .into_string()
        .unwrap();

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
                baz = { version = "0.1.0", registry = "alternative" }
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    // This package isn't used, but publishing it forces the creation of the registry index.
    Package::new("bar", "0.0.1").local(true).publish();
    Package::new("baz", "0.1.1").alternative(true).publish();

    p.cargo("check")
        .overlay_registry(&alt.index_url(), &crates_io_path)
        .run();
}
