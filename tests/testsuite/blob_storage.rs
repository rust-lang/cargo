use std::fs;
use std::path::Path;

use crate::prelude::*;
use cargo_test_support::registry::Package;
use cargo_test_support::{paths, prelude::*, project, t};

#[cargo_test]
fn stores_dependency_artifact_by_content_hash() {
    Package::new("bar", "0.1.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

                [dependencies]
                bar = "0.1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build")
        .arg("-Zshared-blob-storage")
        .masquerade_as_nightly_cargo(&["shared-blob-storage"])
        .run();

    let artifact = p
        .glob("target/debug/build/bar/*/out/libbar-*.rlib")
        .next()
        .unwrap()
        .unwrap();
    let contents = t!(fs::read(artifact));
    let hash = blake3::hash(&contents).to_hex();
    let blob = paths::home().join(".cargo/blobs").join(hash.as_str());
    assert_eq!(t!(fs::read(blob)), contents);
}

#[cargo_test]
fn readonly_cargo_home_still_works() {
    Package::new("bar", "0.1.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

                [dependencies]
                bar = "0.1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("generate-lockfile").run();
    p.cargo("fetch --locked").run();

    let cargo_home = paths::home().join(".cargo");
    chmod_readonly(&cargo_home, true);
    p.cargo("build")
        .arg("-Zshared-blob-storage")
        .masquerade_as_nightly_cargo(&["shared-blob-storage"])
        .run();
    chmod_readonly(&cargo_home, false);
}

fn chmod_readonly(path: &Path, readonly: bool) {
    for entry in t!(path.read_dir()) {
        let entry = t!(entry);
        let path = entry.path();
        if t!(entry.file_type()).is_dir() {
            chmod_readonly(&path, readonly);
        } else {
            set_readonly(&path, readonly);
        }
    }
    set_readonly(path, readonly);
}

fn set_readonly(path: &Path, readonly: bool) {
    let mut perms = t!(path.metadata()).permissions();
    perms.set_readonly(readonly);
    t!(fs::set_permissions(path, perms));
}
