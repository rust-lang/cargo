use std::fs;
use std::path::PathBuf;

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

    p.cargo("build")
        .arg("-Zshared-object-storage")
        .masquerade_as_nightly_cargo(&["shared-object-storage"])
        .run();

    assert!(!shared_storage().exists());
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
    assert!(!object.exists());
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

    assert!(!shared_storage().exists());
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
        assert!(!shared_storage().exists());
    }
}

fn shared_storage() -> PathBuf {
    paths::home().join(".cargo/storage/v1")
}
