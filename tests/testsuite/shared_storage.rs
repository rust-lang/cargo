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

    assert!(!paths::home().join(".cargo/storage").exists());
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
        .arg("-Zshared-object-storage")
        .masquerade_as_nightly_cargo(&["shared-object-storage"])
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
            let object = object(hash.as_str());
            assert!(object.is_file(), "missing object: {}", object.display());
            assert_eq!(t!(fs::read(&object)), contents);
            assert_linked(&artifact, &object);
        }
    };

    // Build and verify the files are deduplicated.
    // Then clean and reverify the files are deduplicated from the previous run.

    p.cargo("build")
        .env("__CARGO_TEST_SHARED_STORAGE_FORCE_HARDLINK", "1")
        .arg("-Zshared-object-storage")
        .masquerade_as_nightly_cargo(&["shared-object-storage"])
        .run();

    verify_files_deduplicated();

    p.cargo("clean").run();

    p.cargo("build")
        .env("__CARGO_TEST_SHARED_STORAGE_FORCE_HARDLINK", "1")
        .arg("-Zshared-object-storage")
        .masquerade_as_nightly_cargo(&["shared-object-storage"])
        .run();

    verify_files_deduplicated();
}

#[cargo_test]
fn should_save_build_manifest() {
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
        .arg("-Zshared-object-storage")
        .masquerade_as_nightly_cargo(&["shared-object-storage"])
        .enable_mac_dsym()
        .run();

    shared_storage().assert_dir_layout(
        str![[r#"
[ROOT]/home/.cargo/storage/v1/build-index/[HASH]
[ROOT]/home/.cargo/storage/v1/objects/[HASH]
[ROOT]/home/.cargo/storage/v1/objects/[HASH]
[ROOT]/home/.cargo/storage/v1/objects/[HASH]
[ROOT]/home/.cargo/storage/v1/objects/[HASH]
[ROOT]/home/.cargo/storage/v1/objects/[HASH]

"#]],
        &[],
    );

    // Find the build manifest through the index and verify it matches its hash
    let manifest_hash = partitioned_hash(&partitioned_file("build-index"));
    let contents = t!(fs::read_to_string(object(&manifest_hash)));
    assert_eq!(
        manifest_hash,
        blake3::hash(contents.as_bytes()).to_hex().as_str()
    );

    // Find the unit output from the build manifest and verify it matches its hash
    let manifest: serde_json::Value = serde_json::from_str(&contents).unwrap();
    let unit_output = manifest["unit_outputs"][0].as_str().unwrap();
    assert_eq!(manifest, serde_json::json!({"unit_outputs": [unit_output]}));
    let unit_contents = t!(fs::read(object(unit_output)));
    assert_eq!(unit_output, blake3::hash(&unit_contents).to_hex().as_str());

    // Verify every object referenced by the unit output exists
    let unit: serde_json::Value = serde_json::from_slice(&unit_contents).unwrap();
    for hash in unit["objects"].as_object().unwrap().values() {
        assert!(object(hash.as_str().unwrap()).is_file());
    }
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
        .arg("-Zshared-object-storage")
        .masquerade_as_nightly_cargo(&["shared-object-storage"])
        .run();

    let artifact = p
        .glob("target/debug/build/bar/*/out/libbar-*.rlib")
        .next()
        .unwrap()
        .unwrap();
    let contents = t!(fs::read(artifact));
    let hash = blake3::hash(&contents).to_hex();
    let hash = hash.as_str();
    let object = shared_storage()
        .join("objects")
        .join(&hash[..2])
        .join(&hash[2..]);
    assert_eq!(t!(fs::read(object)), contents);
}

#[cargo_test]
fn gc_preserves_artifacts_shared_with_live_manifest() {
    Package::new("bar", "0.1.0").publish();
    Package::new("baz", "0.1.0").publish();

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
                baz = { version = "0.1.0", optional = true }
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    let storage_files = || {
        shared_storage()
            .ls_r()
            .into_iter()
            .filter(|path| path.is_file())
            .collect::<Vec<_>>()
    };

    p.cargo("build")
        .arg("-Zshared-object-storage")
        .masquerade_as_nightly_cargo(&["shared-object-storage"])
        .enable_mac_dsym()
        .run();

    let live_files = storage_files();
    let live_usage = partitioned_file("build-index");

    // Both manifests reference bar, but only this manifest references baz.
    p.cargo("build --features baz")
        .arg("-Zshared-object-storage")
        .masquerade_as_nightly_cargo(&["shared-object-storage"])
        .enable_mac_dsym()
        .run();

    shared_storage().assert_dir_layout(
        str![[r#"
[ROOT]/home/.cargo/storage/v1/build-index/[HASH]
[ROOT]/home/.cargo/storage/v1/build-index/[HASH]
[ROOT]/home/.cargo/storage/v1/objects/[HASH]
[ROOT]/home/.cargo/storage/v1/objects/[HASH]
[ROOT]/home/.cargo/storage/v1/objects/[HASH]
[ROOT]/home/.cargo/storage/v1/objects/[HASH]
[ROOT]/home/.cargo/storage/v1/objects/[HASH]
[ROOT]/home/.cargo/storage/v1/objects/[HASH]
[ROOT]/home/.cargo/storage/v1/objects/[HASH]
[ROOT]/home/.cargo/storage/v1/objects/[HASH]
[ROOT]/home/.cargo/storage/v1/objects/[HASH]
[ROOT]/home/.cargo/storage/v1/objects/[HASH]

"#]],
        &[],
    );

    // Set the time of the usage to be in the past for the manifest that references baz
    for usage in shared_storage().join("build-index").ls_r() {
        if usage.is_file() && usage != live_usage {
            t!(fs::write(usage, "0"));
        }
    }

    p.cargo("clean gc --max-shared-storage-age=1day")
        .args(&["-Zgc", "-Zshared-object-storage"])
        .masquerade_as_nightly_cargo(&["gc", "shared-object-storage"])
        .run();

    shared_storage().assert_dir_layout(
        str![[r#"
[ROOT]/home/.cargo/storage/v1/build-index/[HASH]
[ROOT]/home/.cargo/storage/v1/objects/[HASH]
[ROOT]/home/.cargo/storage/v1/objects/[HASH]
[ROOT]/home/.cargo/storage/v1/objects/[HASH]
[ROOT]/home/.cargo/storage/v1/objects/[HASH]
[ROOT]/home/.cargo/storage/v1/objects/[HASH]

"#]],
        &[],
    );
    assert_eq!(storage_files(), live_files);
}

#[cargo_test]
fn should_not_rebuild_when_output_mtimes_are_older_than_dependencies() {
    Package::new("bar", "0.1.0")
        .file("src/lib.rs", "pub fn foo() {}")
        .file(
            "build.rs",
            r#"fn main() { println!("cargo::rerun-if-env-changed=FOO") }"#,
        )
        .publish();

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
        .file("src/main.rs", "extern crate bar; fn main() { bar::foo(); }")
        .build();

    let check = |foo: &str, stderr_data: Option<snapbox::data::Inline>| {
        let mut exec = p.cargo("check -v");
        exec.env("FOO", foo)
            .env("__CARGO_TEST_SHARED_STORAGE_FORCE_HARDLINK", "1")
            .arg("-Zshared-object-storage")
            .masquerade_as_nightly_cargo(&["shared-object-storage"]);

        if let Some(stderr_data) = stderr_data {
            exec.with_stderr_data(stderr_data);
        }

        exec.run();
    };

    // Check causing the artifacts to be linked into object storage
    check("1", None);

    let past = filetime::FileTime::now().unix_seconds() - 3600;
    p.root().move_in_time(|_, _| (past, 0));
    p.build_dir().move_in_time(|_, _| (past, 0));

    // Check again and verify that we are fresh
    check(
        "1",
        Some(str![[r#"
[FRESH] bar v0.1.0
[FRESH] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]),
    );

    // Check again, this time changing FOO which will cause an expected rebuild
    check("2", None);

    // Verify that we are fresh, even when outputs deduplicated from object storage
    check(
        "2",
        Some(str![[r#"
[FRESH] bar v0.1.0
[FRESH] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]),
    );
}

#[cargo_test]
fn skips_shared_storage_on_different_filesystems() {
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
        .arg("-Zshared-object-storage")
        .env("__CARGO_TEST_SHARED_STORAGE_DIFFERENT_FILESYSTEM", "1")
        .masquerade_as_nightly_cargo(&["shared-object-storage"])
        .run();

    assert!(
        shared_storage().read_dir().unwrap().next().is_none(),
        "expected empty shared storage"
    );
}

#[cargo_test]
fn should_not_deduplicate_build_script_outputs() {
    Package::new("bar", "0.1.0")
        .file(
            "build.rs",
            r#"
                use std::io::Write;

                fn main() {
                    println!("cargo::rerun-if-env-changed=BUILD_SCRIPT_OUTPUT");
                    let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
                    let mut file = std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(out.join("generated.txt"))
                        .unwrap();
                    writeln!(file, "{}", std::env::var("BUILD_SCRIPT_OUTPUT").unwrap()).unwrap();
                }
            "#,
        )
        .file(
            "src/lib.rs",
            r#"pub const GENERATED: &str = include_str!(concat!(env!("OUT_DIR"), "/generated.txt"));"#,
        )
        .publish();

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

    // OUT_DIR is not wiped before reruns so build scripts can mutate previously written files.
    for (value, expected) in [("first", "first\n"), ("second", "first\nsecond\n")] {
        p.cargo("build")
            .env("BUILD_SCRIPT_OUTPUT", value)
            .env("__CARGO_TEST_SHARED_STORAGE_FORCE_HARDLINK", "1")
            .arg("-Zshared-object-storage")
            .masquerade_as_nightly_cargo(&["shared-object-storage"])
            .run();

        let output = p
            .glob("target/debug/build/bar/*/out/generated.txt")
            .next()
            .unwrap()
            .unwrap();
        assert_eq!(std::fs::read_to_string(&output).unwrap(), expected);
        let hash = blake3::hash(expected.as_bytes()).to_hex();
        assert!(!object(hash.as_str()).exists());

        // Compiled artifacts are still eligible for object storage.
        let artifact = p
            .glob("target/debug/build/bar/*/out/libbar-*.rlib")
            .next()
            .unwrap()
            .unwrap();
        let contents = t!(fs::read(&artifact));
        let hash = blake3::hash(&contents).to_hex();
        assert_linked(&artifact, object(hash.as_str()));
    }
}

fn shared_storage() -> PathBuf {
    paths::home().join(".cargo/storage/v1")
}

fn object(hash: &str) -> PathBuf {
    shared_storage()
        .join("objects")
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
