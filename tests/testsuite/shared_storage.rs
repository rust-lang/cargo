use std::fs;
use std::path::{Path, PathBuf};

use crate::prelude::*;
use cargo_test_support::paths::ReadOnly;
use cargo_test_support::registry::Package;
use cargo_test_support::{paths, prelude::*, project, str, t};

#[cargo_test]
fn disabled_without_unstable_flag() {
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

    p.cargo("build").run();

    assert!(!paths::home().join(".cargo/shared-storage").exists());
}

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
    let hash = hash.as_str();
    let blob = paths::home()
        .join(".cargo/shared-storage/blobs")
        .join(&hash[..2])
        .join(&hash[2..]);
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

    let _readonly = ReadOnly::new(paths::cargo_home());
    p.cargo("build")
        .arg("-Zshared-blob-storage")
        .masquerade_as_nightly_cargo(&["shared-blob-storage"])
        .run();
}

#[cargo_test]
fn should_deduplicate_basic_rustc_artifacts() {
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

    let verify_files_deduplicated = || {
        for extension in ["rlib", "rmeta"] {
            let artifact = p
                .glob(&format!(
                    "target/debug/build/bar/*/out/libbar-*.{extension}"
                ))
                .next()
                .unwrap()
                .unwrap();
            let contents = t!(fs::read(&artifact));
            let hash = blake3::hash(&contents).to_hex();
            let blob = blob(hash.as_str());
            assert!(blob.is_file(), "missing blob: {}", blob.display());
            assert_eq!(t!(fs::read(&blob)), contents);
            assert_linked(&artifact, &blob);
        }
    };

    // Build and verify the files are deduplicated.
    // Then clean and reverify the files are deduplicated from the previous run.

    p.cargo("build")
        .env("__CARGO_TEST_SHARED_STORAGE_FORCE_HARDLINK", "1")
        .arg("-Zshared-blob-storage")
        .masquerade_as_nightly_cargo(&["shared-blob-storage"])
        .run();

    verify_files_deduplicated();

    p.cargo("clean").run();

    p.cargo("build")
        .env("__CARGO_TEST_SHARED_STORAGE_FORCE_HARDLINK", "1")
        .arg("-Zshared-blob-storage")
        .masquerade_as_nightly_cargo(&["shared-blob-storage"])
        .run();

    verify_files_deduplicated();
}

#[cargo_test]
fn should_save_build_snapshot() {
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
        .env("__CARGO_TEST_SHARED_STORAGE_FORCE_HARDLINK", "1")
        .arg("-Zshared-blob-storage")
        .masquerade_as_nightly_cargo(&["shared-blob-storage"])
        .run();

    shared_storage().assert_dir_layout(
        str![[r#"
[ROOT]/home/.cargo/shared-storage/blobs/[HASH]
[ROOT]/home/.cargo/shared-storage/blobs/[HASH]
[ROOT]/home/.cargo/shared-storage/blobs/[HASH]
[ROOT]/home/.cargo/shared-storage/snapshots/[HASH]
[ROOT]/home/.cargo/shared-storage/unit-output/[HASH]
[ROOT]/home/.cargo/shared-storage/usage/[HASH]

"#]],
        &[],
    );

    // Find the snapshot file and verify it matches its hash
    let snapshot_path = partitioned_file("snapshots");
    let contents = t!(fs::read_to_string(&snapshot_path));
    let snapshot_hash = partitioned_hash(&snapshot_path);
    assert_eq!(
        snapshot_hash,
        blake3::hash(contents.as_bytes()).to_hex().as_str()
    );

    // Find the unit output and verify its in the snapshot
    let snapshot: serde_json::Value = serde_json::from_str(&contents).unwrap();
    let unit_output = partitioned_hash(&partitioned_file("unit-output"));
    assert_eq!(snapshot, serde_json::json!({"unit_outputs": [unit_output]}));

    // Verify the snapshot usage exists that matches the snapshot hash
    let usage_path = partitioned_file("usage");
    assert_eq!(partitioned_hash(&usage_path), snapshot_hash);
}

fn shared_storage() -> PathBuf {
    paths::home().join(".cargo/shared-storage")
}

fn blob(hash: &str) -> PathBuf {
    shared_storage()
        .join("blobs")
        .join(&hash[..2])
        .join(&hash[2..])
}

/// Returns the first file in a shared storage dir partitioned by `hash[..2]/hash[2..]`.
fn partitioned_file(dir: &str) -> PathBuf {
    let pattern = shared_storage().join(dir).join("*").join("*");
    glob::glob(&pattern.to_string_lossy())
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
}

/// Rebuilds the hash from a path partitioned by `hash[..2]/hash[2..]`.
fn partitioned_hash(path: &Path) -> String {
    let prefix = path
        .parent()
        .unwrap()
        .file_name()
        .unwrap()
        .to_str()
        .unwrap();
    let suffix = path.file_name().unwrap().to_str().unwrap();
    format!("{prefix}{suffix}")
}

#[track_caller]
fn assert_linked(a: impl AsRef<Path>, b: impl AsRef<Path>) {
    let a = a.as_ref();
    let b = b.as_ref();
    assert!(
        same_file::is_same_file(a, b).expect("failed to compare file identities"),
        "{} and {} are not hardlinked",
        a.display(),
        b.display()
    );
}
