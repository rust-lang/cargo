//! Tests for `build.build-dir` config property.
//!
//! The testing strategy for build-dir functionality is primarily checking if directories / files
//! are in the expected locations.
//! The rational is that other tests will verify each individual feature, while the tests in this
//! file verify the files saved to disk are in the correct locations according to the `build-dir`
//! configuration.
//!
//! Tests check if directories match some "layout" by using [`CargoPathExt::assert_file_layout`]

use std::path::PathBuf;

use crate::prelude::*;
use cargo_test_support::registry::{Package, RegistryBuilder};
use cargo_test_support::{paths, prelude::*, project, str};
use std::env::consts::{DLL_PREFIX, DLL_SUFFIX, EXE_SUFFIX};

#[cargo_test]
fn binary_with_debug() {
    let p = project()
        .file("src/main.rs", r#"fn main() { println!("Hello, World!") }"#)
        .file(
            ".cargo/config.toml",
            r#"
            [build]
            target-dir = "target-dir"
            build-dir = "build-dir"
            "#,
        )
        .build();

    p.cargo("-Zbuild-dir-new-layout build")
        .masquerade_as_nightly_cargo(&["new build-dir layout"])
        .enable_mac_dsym()
        .run();

    assert_not_exists(&p.root().join("target"));

    p.root().join("build-dir").assert_build_dir_layout(str![[r#"
[ROOT]/foo/build-dir/.rustc_info.json
[ROOT]/foo/build-dir/CACHEDIR.TAG
[ROOT]/foo/build-dir/debug/.cargo-lock
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/fingerprint/bin-foo
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/fingerprint/bin-foo.json
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/fingerprint/dep-bin-foo
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/fingerprint/invoked.timestamp
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/deps/foo[..][EXE]
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/deps/foo[..].d

"#]]);

    p.root()
        .join("target-dir")
        .assert_build_dir_layout(str![[r#"
[ROOT]/foo/target-dir/CACHEDIR.TAG
[ROOT]/foo/target-dir/debug/.cargo-lock
[ROOT]/foo/target-dir/debug/foo[EXE]
[ROOT]/foo/target-dir/debug/foo.d

"#]]);
}

#[cargo_test]
fn binary_with_release() {
    let p = project()
        .file("src/main.rs", r#"fn main() { println!("Hello, World!") }"#)
        .file(
            ".cargo/config.toml",
            r#"
            [build]
            target-dir = "target-dir"
            build-dir = "build-dir"
            "#,
        )
        .build();

    p.cargo("-Zbuild-dir-new-layout build --release")
        .masquerade_as_nightly_cargo(&["new build-dir layout"])
        .enable_mac_dsym()
        .run();

    assert_exists_patterns_with_base_dir(
        &p.root(),
        &[
            // Check the pre-uplifted binary in the build-dir
            &format!("build-dir/release/build/foo/*/deps/foo*{EXE_SUFFIX}"),
            "build-dir/release/build/foo/*/deps/foo*.d",
            // Verify the binary was copied to the target-dir
            &format!("target-dir/release/foo{EXE_SUFFIX}"),
            "target-dir/release/foo.d",
        ],
    );
    p.root().join("build-dir").assert_build_dir_layout(str![[r#"
[ROOT]/foo/build-dir/.rustc_info.json
[ROOT]/foo/build-dir/CACHEDIR.TAG
[ROOT]/foo/build-dir/release/.cargo-lock
[ROOT]/foo/build-dir/release/build/foo/[HASH]/fingerprint/bin-foo
[ROOT]/foo/build-dir/release/build/foo/[HASH]/fingerprint/bin-foo.json
[ROOT]/foo/build-dir/release/build/foo/[HASH]/fingerprint/dep-bin-foo
[ROOT]/foo/build-dir/release/build/foo/[HASH]/fingerprint/invoked.timestamp
[ROOT]/foo/build-dir/release/build/foo/[HASH]/deps/foo[..][EXE]
[ROOT]/foo/build-dir/release/build/foo/[HASH]/deps/foo[..].d

"#]]);

    p.root()
        .join("target-dir")
        .assert_build_dir_layout(str![[r#"
[ROOT]/foo/target-dir/CACHEDIR.TAG
[ROOT]/foo/target-dir/release/.cargo-lock
[ROOT]/foo/target-dir/release/foo[EXE]
[ROOT]/foo/target-dir/release/foo.d

"#]]);
}

#[cargo_test]
fn libs() {
    // https://doc.rust-lang.org/reference/linkage.html#r-link.staticlib
    let (staticlib_prefix, staticlib_suffix) =
        if cfg!(target_os = "windows") && cfg!(target_env = "msvc") {
            ("", ".lib")
        } else {
            ("lib", ".a")
        };

    // (crate-type, list of final artifacts)
    let lib_types = [
        ("lib", ["libfoo.rlib", "libfoo.d"]),
        (
            "dylib",
            [
                &format!("{DLL_PREFIX}foo{DLL_SUFFIX}"),
                &format!("{DLL_PREFIX}foo.d"),
            ],
        ),
        (
            "cdylib",
            [
                &format!("{DLL_PREFIX}foo{DLL_SUFFIX}"),
                &format!("{DLL_PREFIX}foo.d"),
            ],
        ),
        (
            "staticlib",
            [
                &format!("{staticlib_prefix}foo{staticlib_suffix}"),
                &format!("{staticlib_prefix}foo.d"),
            ],
        ),
    ];

    for (lib_type, expected_files) in lib_types {
        let p = project()
            .file("src/lib.rs", r#"fn foo() { println!("Hello, World!") }"#)
            .file(
                "Cargo.toml",
                &format!(
                    r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
            edition = "2015"

            [lib]
            crate-type = ["{lib_type}"]
            "#
                ),
            )
            .file(
                ".cargo/config.toml",
                r#"
            [build]
            target-dir = "target-dir"
            build-dir = "build-dir"
            "#,
            )
            .build();

        p.cargo("-Zbuild-dir-new-layout build")
            .masquerade_as_nightly_cargo(&["new build-dir layout"])
            .enable_mac_dsym()
            .run();

        // Verify lib artifacts were copied into the artifact dir
        assert_exists_patterns_with_base_dir(&p.root().join("target-dir/debug"), &expected_files);
    }
}

#[cargo_test]
fn should_default_to_target() {
    let p = project()
        .file("src/main.rs", r#"fn main() { println!("Hello, World!") }"#)
        .build();

    p.cargo("-Zbuild-dir-new-layout build")
        .masquerade_as_nightly_cargo(&["new build-dir layout"])
        .enable_mac_dsym()
        .run();

    p.root().join("target").assert_build_dir_layout(str![[r#"
[ROOT]/foo/target/.rustc_info.json
[ROOT]/foo/target/CACHEDIR.TAG
[ROOT]/foo/target/debug/.cargo-lock
[ROOT]/foo/target/debug/build/foo/[HASH]/fingerprint/bin-foo
[ROOT]/foo/target/debug/build/foo/[HASH]/fingerprint/bin-foo.json
[ROOT]/foo/target/debug/build/foo/[HASH]/fingerprint/dep-bin-foo
[ROOT]/foo/target/debug/build/foo/[HASH]/fingerprint/invoked.timestamp
[ROOT]/foo/target/debug/build/foo/[HASH]/deps/foo[..][EXE]
[ROOT]/foo/target/debug/build/foo/[HASH]/deps/foo[..].d
[ROOT]/foo/target/debug/foo[EXE]
[ROOT]/foo/target/debug/foo.d

"#]]);
}

#[cargo_test]
fn should_respect_env_var() {
    let p = project()
        .file("src/main.rs", r#"fn main() { println!("Hello, World!") }"#)
        .build();

    p.cargo("-Zbuild-dir-new-layout build")
        .masquerade_as_nightly_cargo(&["new build-dir layout"])
        .env("CARGO_BUILD_BUILD_DIR", "build-dir")
        .enable_mac_dsym()
        .run();

    p.root().join("build-dir").assert_build_dir_layout(str![[r#"
[ROOT]/foo/build-dir/.rustc_info.json
[ROOT]/foo/build-dir/CACHEDIR.TAG
[ROOT]/foo/build-dir/debug/.cargo-lock
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/fingerprint/bin-foo
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/fingerprint/bin-foo.json
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/fingerprint/dep-bin-foo
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/fingerprint/invoked.timestamp
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/deps/foo[..][EXE]
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/deps/foo[..].d

"#]]);
}

#[cargo_test]
fn build_script_should_output_to_build_dir() {
    let p = project()
        .file("src/main.rs", r#"fn main() { println!("Hello, World!") }"#)
        .file(
            "build.rs",
            r#"
            fn main() {
                std::fs::write(
                    format!("{}/foo.txt", std::env::var("OUT_DIR").unwrap()),
                    "Hello, world!",
                )
                .unwrap();
            }
            "#,
        )
        .file(
            ".cargo/config.toml",
            r#"
            [build]
            target-dir = "target-dir"
            build-dir = "build-dir"
            "#,
        )
        .build();

    p.cargo("-Zbuild-dir-new-layout build")
        .masquerade_as_nightly_cargo(&["new build-dir layout"])
        .enable_mac_dsym()
        .run();

    p.root().join("build-dir").assert_build_dir_layout(str![[r#"
[ROOT]/foo/build-dir/CACHEDIR.TAG
[ROOT]/foo/build-dir/debug/.cargo-lock
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/deps/foo[..][EXE]
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/deps/foo[..].d
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/fingerprint/invoked.timestamp
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/fingerprint/dep-bin-foo
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/fingerprint/bin-foo
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/fingerprint/bin-foo.json
[ROOT]/foo/build-dir/.rustc_info.json
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/fingerprint/run-build-script-build-script-build
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/fingerprint/run-build-script-build-script-build.json
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/fingerprint/invoked.timestamp
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/fingerprint/dep-build-script-build-script-build
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/fingerprint/build-script-build-script-build
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/fingerprint/build-script-build-script-build.json
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/build-script/build_script_build[..].d
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/build-script/build_script_build[..][EXE]
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/build-script/build-script-build[EXE]
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/build-script-execution/out/foo.txt
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/build-script-execution/invoked.timestamp
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/build-script-execution/output
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/build-script-execution/root-output
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/build-script-execution/stderr

"#]]);
}

#[cargo_test]
fn cargo_tmpdir_should_output_to_build_dir() {
    let p = project()
        .file("src/main.rs", r#"fn main() { println!("Hello, World!") }"#)
        .file(
            "tests/foo.rs",
            r#"
            #[test]
            fn test() {
                std::fs::write(
                    format!("{}/foo.txt", env!("CARGO_TARGET_TMPDIR")),
                    "Hello, world!",
                )
                .unwrap();
            }
            "#,
        )
        .file(
            ".cargo/config.toml",
            r#"
            [build]
            target-dir = "target-dir"
            build-dir = "build-dir"
            "#,
        )
        .build();

    p.cargo("-Zbuild-dir-new-layout test")
        .masquerade_as_nightly_cargo(&["new build-dir layout"])
        .enable_mac_dsym()
        .run();

    p.root().join("build-dir").assert_build_dir_layout(str![[r#"
[ROOT]/foo/build-dir/CACHEDIR.TAG
[ROOT]/foo/build-dir/debug/.cargo-lock
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/deps/foo-[HASH].d
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/deps/foo-[HASH].d
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/deps/foo[..].d
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/deps/foo-[HASH][EXE]
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/deps/foo-[HASH][EXE]
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/deps/foo[..][EXE]
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/fingerprint/invoked.timestamp
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/fingerprint/dep-test-bin-foo
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/fingerprint/test-bin-foo
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/fingerprint/test-bin-foo.json
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/fingerprint/invoked.timestamp
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/fingerprint/dep-test-integration-test-foo
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/fingerprint/test-integration-test-foo
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/fingerprint/test-integration-test-foo.json
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/fingerprint/invoked.timestamp
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/fingerprint/dep-bin-foo
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/fingerprint/bin-foo
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/fingerprint/bin-foo.json
[ROOT]/foo/build-dir/tmp/foo.txt
[ROOT]/foo/build-dir/.rustc_info.json

"#]]);

    p.root()
        .join("target-dir")
        .assert_build_dir_layout(str![[r#"
[ROOT]/foo/target-dir/CACHEDIR.TAG
[ROOT]/foo/target-dir/debug/.cargo-lock
[ROOT]/foo/target-dir/debug/foo[EXE]

"#]]);
}

#[cargo_test]
fn examples_should_output_to_build_dir_and_uplift_to_target_dir() {
    let p = project()
        .file("src/main.rs", r#"fn main() { println!("Hello, World!") }"#)
        .file("examples/foo.rs", r#"fn main() { }"#)
        .file(
            ".cargo/config.toml",
            r#"
            [build]
            target-dir = "target-dir"
            build-dir = "build-dir"
            "#,
        )
        .build();

    p.cargo("-Zbuild-dir-new-layout build --examples")
        .masquerade_as_nightly_cargo(&["new build-dir layout"])
        .enable_mac_dsym()
        .run();

    p.root().join("build-dir").assert_build_dir_layout(str![[r#"
[ROOT]/foo/build-dir/.rustc_info.json
[ROOT]/foo/build-dir/CACHEDIR.TAG
[ROOT]/foo/build-dir/debug/.cargo-lock
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/fingerprint/dep-example-foo
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/fingerprint/example-foo
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/fingerprint/example-foo.json
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/fingerprint/invoked.timestamp
[ROOT]/foo/build-dir/debug/examples/foo[..][EXE]
[ROOT]/foo/build-dir/debug/examples/foo[..].d

"#]]);

    p.root()
        .join("target-dir")
        .assert_build_dir_layout(str![[r#"
[ROOT]/foo/target-dir/CACHEDIR.TAG
[ROOT]/foo/target-dir/debug/.cargo-lock
[ROOT]/foo/target-dir/debug/examples/foo[EXE]
[ROOT]/foo/target-dir/debug/examples/foo.d

"#]]);
}

#[cargo_test]
fn benches_should_output_to_build_dir() {
    let p = project()
        .file("src/main.rs", r#"fn main() { println!("Hello, World!") }"#)
        .file("benches/foo.rs", r#"fn main() { }"#)
        .file(
            ".cargo/config.toml",
            r#"
            [build]
            target-dir = "target-dir"
            build-dir = "build-dir"
            "#,
        )
        .build();

    p.cargo("-Zbuild-dir-new-layout build --bench=foo")
        .masquerade_as_nightly_cargo(&["new build-dir layout"])
        .enable_mac_dsym()
        .run();

    p.root().join("build-dir").assert_build_dir_layout(str![[r#"
[ROOT]/foo/build-dir/CACHEDIR.TAG
[ROOT]/foo/build-dir/debug/.cargo-lock
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/deps/foo-[HASH].d
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/deps/foo[..].d
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/deps/foo-[HASH][EXE]
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/deps/foo[..][EXE]
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/fingerprint/invoked.timestamp
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/fingerprint/dep-test-bench-foo
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/fingerprint/test-bench-foo
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/fingerprint/test-bench-foo.json
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/fingerprint/invoked.timestamp
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/fingerprint/dep-bin-foo
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/fingerprint/bin-foo
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/fingerprint/bin-foo.json
[ROOT]/foo/build-dir/.rustc_info.json

"#]]);

    p.root()
        .join("target-dir")
        .assert_build_dir_layout(str![[r#"
[ROOT]/foo/target-dir/CACHEDIR.TAG
[ROOT]/foo/target-dir/debug/.cargo-lock
[ROOT]/foo/target-dir/debug/foo[EXE]

"#]]);
}

#[cargo_test]
fn cargo_doc_should_output_to_target_dir() {
    let p = project()
        .file("src/main.rs", r#"fn main() { println!("Hello, World!") }"#)
        .file(
            ".cargo/config.toml",
            r#"
            [build]
            target-dir = "target-dir"
            build-dir = "build-dir"
            "#,
        )
        .build();

    p.cargo("-Zbuild-dir-new-layout doc")
        .masquerade_as_nightly_cargo(&["new build-dir layout"])
        .enable_mac_dsym()
        .run();

    let docs_dir = p.root().join("target-dir/doc");

    assert_exists(&docs_dir);
    assert_exists(&docs_dir.join("foo/index.html"));
}

#[cargo_test]
fn cargo_package_should_build_in_build_dir_and_output_to_target_dir() {
    let p = project()
        .file("src/main.rs", r#"fn main() { println!("Hello, World!") }"#)
        .file(
            ".cargo/config.toml",
            r#"
            [build]
            target-dir = "target-dir"
            build-dir = "build-dir"
            "#,
        )
        .build();

    p.cargo("-Zbuild-dir-new-layout package")
        .masquerade_as_nightly_cargo(&["new build-dir layout"])
        .enable_mac_dsym()
        .run();

    let package_artifact_dir = p.root().join("target-dir/package");
    assert_exists(&package_artifact_dir);
    assert_exists(&package_artifact_dir.join("foo-0.0.1.crate"));
    assert!(package_artifact_dir.join("foo-0.0.1.crate").is_file());
    p.root().join("build-dir").assert_build_dir_layout(str![[r#"
[ROOT]/foo/build-dir/.rustc_info.json
[ROOT]/foo/build-dir/debug/.cargo-lock
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/fingerprint/bin-foo
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/fingerprint/bin-foo.json
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/fingerprint/dep-bin-foo
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/fingerprint/invoked.timestamp
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/deps/foo[..][EXE]
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/deps/foo[..].d
[ROOT]/foo/build-dir/debug/foo[EXE]
[ROOT]/foo/build-dir/debug/foo.d
[ROOT]/foo/build-dir/package/foo-0.0.1/Cargo.lock
[ROOT]/foo/build-dir/package/foo-0.0.1/Cargo.toml
[ROOT]/foo/build-dir/package/foo-0.0.1/Cargo.toml.orig
[ROOT]/foo/build-dir/package/foo-0.0.1/src/main.rs
[ROOT]/foo/build-dir/package/foo-0.0.1.crate

"#]]);

    p.root()
        .join("target-dir")
        .assert_build_dir_layout(str![[r#"
[ROOT]/foo/target-dir/package/foo-0.0.1.crate

"#]]);
}

#[cargo_test]
fn cargo_publish_should_only_touch_build_dir() {
    let registry = RegistryBuilder::new().http_api().http_index().build();

    let p = project()
        .file("src/main.rs", r#"fn main() { println!("Hello, World!") }"#)
        .file(
            ".cargo/config.toml",
            r#"
            [build]
            target-dir = "target-dir"
            build-dir = "build-dir"
            "#,
        )
        .build();

    p.cargo("-Zbuild-dir-new-layout publish")
        .masquerade_as_nightly_cargo(&["new build-dir layout"])
        .replace_crates_io(registry.index_url())
        .enable_mac_dsym()
        .run();

    let package_artifact_dir = p.root().join("target-dir/package");
    assert!(!package_artifact_dir.exists());

    let package_build_dir = p.root().join("build-dir/package");
    assert_exists(&package_build_dir);
    assert_exists(&package_build_dir.join("foo-0.0.1"));
    assert!(package_build_dir.join("foo-0.0.1").is_dir());
}

#[cargo_test]
fn cargo_clean_should_clean_the_target_dir_and_build_dir() {
    let p = project()
        .file("src/main.rs", r#"fn main() { println!("Hello, World!") }"#)
        .file(
            ".cargo/config.toml",
            r#"
            [build]
            target-dir = "target-dir"
            build-dir = "build-dir"
            "#,
        )
        .build();

    p.cargo("-Zbuild-dir-new-layout build")
        .masquerade_as_nightly_cargo(&["new build-dir layout"])
        .enable_mac_dsym()
        .run();

    p.root().join("build-dir").assert_build_dir_layout(str![[r#"
[ROOT]/foo/build-dir/.rustc_info.json
[ROOT]/foo/build-dir/CACHEDIR.TAG
[ROOT]/foo/build-dir/debug/.cargo-lock
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/fingerprint/bin-foo
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/fingerprint/bin-foo.json
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/fingerprint/dep-bin-foo
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/fingerprint/invoked.timestamp
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/deps/foo[..][EXE]
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/deps/foo[..].d

"#]]);

    p.root()
        .join("target-dir")
        .assert_build_dir_layout(str![[r#"
[ROOT]/foo/target-dir/CACHEDIR.TAG
[ROOT]/foo/target-dir/debug/.cargo-lock
[ROOT]/foo/target-dir/debug/foo[EXE]
[ROOT]/foo/target-dir/debug/foo.d

"#]]);

    p.cargo("-Zbuild-dir-new-layout clean")
        .masquerade_as_nightly_cargo(&["new build-dir layout"])
        .enable_mac_dsym()
        .run();

    assert_not_exists(&p.root().join("build-dir"));
    assert_not_exists(&p.root().join("target-dir"));
}

#[cargo_test]
fn cargo_clean_should_remove_correct_files() {
    Package::new("bar", "0.1.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []
                exclude = ["*.txt"]
                license = "MIT"
                description = "foo"

                [dependencies]
                bar = "0.1"
            "#,
        )
        .file("src/main.rs", r#"fn main() { println!("Hello, World!"); }"#)
        .file(
            ".cargo/config.toml",
            r#"
            [build]
            target-dir = "target-dir"
            build-dir = "build-dir"
            "#,
        )
        .build();

    p.cargo("-Zbuild-dir-new-layout build")
        .masquerade_as_nightly_cargo(&["new build-dir layout"])
        .enable_mac_dsym()
        .run();

    p.root().join("build-dir").assert_build_dir_layout(str![[r#"
[ROOT]/foo/build-dir/.rustc_info.json
[ROOT]/foo/build-dir/CACHEDIR.TAG
[ROOT]/foo/build-dir/debug/.cargo-lock
[ROOT]/foo/build-dir/debug/build/bar/[HASH]/deps/bar-[HASH].d
[ROOT]/foo/build-dir/debug/build/bar/[HASH]/deps/libbar-[HASH].rlib
[ROOT]/foo/build-dir/debug/build/bar/[HASH]/deps/libbar-[HASH].rmeta
[ROOT]/foo/build-dir/debug/build/bar/[HASH]/fingerprint/dep-lib-bar
[ROOT]/foo/build-dir/debug/build/bar/[HASH]/fingerprint/invoked.timestamp
[ROOT]/foo/build-dir/debug/build/bar/[HASH]/fingerprint/lib-bar
[ROOT]/foo/build-dir/debug/build/bar/[HASH]/fingerprint/lib-bar.json
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/deps/foo[..][EXE]
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/deps/foo[..].d
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/fingerprint/bin-foo
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/fingerprint/bin-foo.json
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/fingerprint/dep-bin-foo
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/fingerprint/invoked.timestamp

"#]]);

    p.cargo("-Zbuild-dir-new-layout clean -p bar")
        .masquerade_as_nightly_cargo(&["new build-dir layout"])
        .enable_mac_dsym()
        .run();

    p.root().join("build-dir").assert_build_dir_layout(str![[r#"
[ROOT]/foo/build-dir/.rustc_info.json
[ROOT]/foo/build-dir/CACHEDIR.TAG
[ROOT]/foo/build-dir/debug/.cargo-lock
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/deps/foo[..][EXE]
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/deps/foo[..].d
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/fingerprint/bin-foo
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/fingerprint/bin-foo.json
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/fingerprint/dep-bin-foo
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/fingerprint/invoked.timestamp

"#]]);
}

#[cargo_test]
fn timings_report_should_output_to_target_dir() {
    let p = project()
        .file("src/main.rs", r#"fn main() { println!("Hello, World!") }"#)
        .file(
            ".cargo/config.toml",
            r#"
            [build]
            target-dir = "target-dir"
            build-dir = "build-dir"
            "#,
        )
        .build();

    p.cargo("-Zbuild-dir-new-layout build --timings")
        .masquerade_as_nightly_cargo(&["new build-dir layout"])
        .enable_mac_dsym()
        .run();

    assert_exists(&p.root().join("target-dir/cargo-timings/cargo-timing.html"));
}

#[cargo_test(
    nightly,
    reason = "-Zfuture-incompat-test requires nightly (permanently)"
)]
fn future_incompat_should_output_to_build_dir() {
    let p = project()
        .file("src/main.rs", r#"fn main() { let x = 1; }"#)
        .file(
            ".cargo/config.toml",
            r#"
            [build]
            target-dir = "target-dir"
            build-dir = "build-dir"
            "#,
        )
        .build();

    p.cargo("-Zbuild-dir-new-layout build")
        .masquerade_as_nightly_cargo(&["new build-dir layout"])
        .arg("--future-incompat-report")
        .env("RUSTFLAGS", "-Zfuture-incompat-test")
        .run();

    assert_exists(&p.root().join("build-dir/.future-incompat-report.json"));
}

#[cargo_test]
fn template_should_error_for_invalid_variables() {
    let p = project()
        .file("src/main.rs", r#"fn main() { println!("Hello, World!") }"#)
        .file(
            ".cargo/config.toml",
            r#"
            [build]
            build-dir = "{fake}/build-dir"
            target-dir = "target-dir"
            "#,
        )
        .build();

    p.cargo("-Zbuild-dir-new-layout build")
        .masquerade_as_nightly_cargo(&["new build-dir layout"])
        .enable_mac_dsym()
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] unexpected variable `fake` in build.build-dir path `{fake}/build-dir`

[HELP] available template variables are `{workspace-root}`, `{cargo-cache-home}`, `{workspace-path-hash}`

"#]])
        .run();
}

#[cargo_test]
fn template_should_suggest_nearest_variable() {
    let p = project()
        .file("src/lib.rs", "")
        .file(
            ".cargo/config.toml",
            r#"
            [build]
            build-dir = "{workspace-ro}/build-dir"
            "#,
        )
        .build();

    p.cargo("-Zbuild-dir-new-layout build")
        .masquerade_as_nightly_cargo(&["new build-dir layout"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] unexpected variable `workspace-ro` in build.build-dir path `{workspace-ro}/build-dir`

[HELP] a template variable with a similar name exists: `workspace-root`

"#]])
        .run();
}

#[cargo_test]
fn template_workspace_root() {
    let p = project()
        .file("src/main.rs", r#"fn main() { println!("Hello, World!") }"#)
        .file(
            ".cargo/config.toml",
            r#"
            [build]
            build-dir = "{workspace-root}/build-dir"
            target-dir = "target-dir"
            "#,
        )
        .build();

    p.cargo("-Zbuild-dir-new-layout build")
        .masquerade_as_nightly_cargo(&["new build-dir layout"])
        .enable_mac_dsym()
        .run();

    // Verify the binary was uplifted to the target-dir
    assert_exists(&p.root().join(&format!("target-dir/debug/foo{EXE_SUFFIX}")));
    p.root().join("build-dir").assert_build_dir_layout(str![[r#"
[ROOT]/foo/build-dir/.rustc_info.json
[ROOT]/foo/build-dir/CACHEDIR.TAG
[ROOT]/foo/build-dir/debug/.cargo-lock
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/fingerprint/bin-foo
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/fingerprint/bin-foo.json
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/fingerprint/dep-bin-foo
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/fingerprint/invoked.timestamp
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/deps/foo[..][EXE]
[ROOT]/foo/build-dir/debug/build/foo/[HASH]/deps/foo[..].d

"#]]);

    p.root()
        .join("target-dir")
        .assert_build_dir_layout(str![[r#"
[ROOT]/foo/target-dir/CACHEDIR.TAG
[ROOT]/foo/target-dir/debug/.cargo-lock
[ROOT]/foo/target-dir/debug/foo[EXE]
[ROOT]/foo/target-dir/debug/foo.d

"#]]);
}

#[cargo_test]
fn template_cargo_cache_home() {
    let p = project()
        .file("src/main.rs", r#"fn main() { println!("Hello, World!") }"#)
        .file(
            ".cargo/config.toml",
            r#"
            [build]
            build-dir = "{cargo-cache-home}/build-dir"
            target-dir = "target-dir"
            "#,
        )
        .build();

    p.cargo("-Zbuild-dir-new-layout build")
        .masquerade_as_nightly_cargo(&["new build-dir layout"])
        .enable_mac_dsym()
        .run();

    // Verify the binary was uplifted to the target-dir
    assert_exists(&p.root().join(&format!("target-dir/debug/foo{EXE_SUFFIX}")));
    paths::cargo_home()
        .join("build-dir")
        .assert_build_dir_layout(str![[r#"
[ROOT]/home/.cargo/build-dir/.rustc_info.json
[ROOT]/home/.cargo/build-dir/CACHEDIR.TAG
[ROOT]/home/.cargo/build-dir/debug/.cargo-lock
[ROOT]/home/.cargo/build-dir/debug/build/foo/[HASH]/fingerprint/bin-foo
[ROOT]/home/.cargo/build-dir/debug/build/foo/[HASH]/fingerprint/bin-foo.json
[ROOT]/home/.cargo/build-dir/debug/build/foo/[HASH]/fingerprint/dep-bin-foo
[ROOT]/home/.cargo/build-dir/debug/build/foo/[HASH]/fingerprint/invoked.timestamp
[ROOT]/home/.cargo/build-dir/debug/build/foo/[HASH]/deps/foo[..][EXE]
[ROOT]/home/.cargo/build-dir/debug/build/foo/[HASH]/deps/foo[..].d

"#]]);

    p.root()
        .join("target-dir")
        .assert_build_dir_layout(str![[r#"
[ROOT]/foo/target-dir/CACHEDIR.TAG
[ROOT]/foo/target-dir/debug/.cargo-lock
[ROOT]/foo/target-dir/debug/foo[EXE]
[ROOT]/foo/target-dir/debug/foo.d

"#]]);
}

#[cargo_test]
fn template_workspace_path_hash() {
    let p = project()
        .file("src/main.rs", r#"fn main() { println!("Hello, World!") }"#)
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "1.0.0"
            authors = []
            edition = "2015"
            "#,
        )
        .file(
            ".cargo/config.toml",
            r#"
            [build]
            build-dir = "foo/{workspace-path-hash}/build-dir"
            target-dir = "target-dir"
            "#,
        )
        .build();

    p.cargo("-Zbuild-dir-new-layout build")
        .masquerade_as_nightly_cargo(&["new build-dir layout"])
        .enable_mac_dsym()
        .run();

    let foo_dir = p.root().join("foo");
    assert_exists(&foo_dir);
    let hash_dir = parse_workspace_manifest_path_hash(&foo_dir);

    let build_dir = hash_dir.as_path().join("build-dir");

    // Verify the binary was uplifted to the target-dir
    assert_exists(&p.root().join(&format!("target-dir/debug/foo{EXE_SUFFIX}")));
    build_dir.assert_build_dir_layout(str![[r#"
[ROOT]/foo/foo/[HASH]/build-dir/.rustc_info.json
[ROOT]/foo/foo/[HASH]/build-dir/CACHEDIR.TAG
[ROOT]/foo/foo/[HASH]/build-dir/debug/.cargo-lock
[ROOT]/foo/foo/[HASH]/build-dir/debug/build/foo/[HASH]/fingerprint/bin-foo
[ROOT]/foo/foo/[HASH]/build-dir/debug/build/foo/[HASH]/fingerprint/bin-foo.json
[ROOT]/foo/foo/[HASH]/build-dir/debug/build/foo/[HASH]/fingerprint/dep-bin-foo
[ROOT]/foo/foo/[HASH]/build-dir/debug/build/foo/[HASH]/fingerprint/invoked.timestamp
[ROOT]/foo/foo/[HASH]/build-dir/debug/build/foo/[HASH]/deps/foo[..][EXE]
[ROOT]/foo/foo/[HASH]/build-dir/debug/build/foo/[HASH]/deps/foo[..].d

"#]]);

    p.root()
        .join("target-dir")
        .assert_build_dir_layout(str![[r#"
[ROOT]/foo/target-dir/CACHEDIR.TAG
[ROOT]/foo/target-dir/debug/.cargo-lock
[ROOT]/foo/target-dir/debug/foo[EXE]
[ROOT]/foo/target-dir/debug/foo.d

"#]]);
}

/// Verify that the {workspace-path-hash} does not changes if cargo is run from inside of
/// a symlinked directory.
/// The test approach is to build a project twice from the non-symlinked directory and a symlinked
/// directory and then compare the build-dir paths.
#[cargo_test]
fn template_workspace_path_hash_should_handle_symlink() {
    #[cfg(unix)]
    use std::os::unix::fs::symlink;
    #[cfg(windows)]
    use std::os::windows::fs::symlink_dir as symlink;

    let p = project()
        .file("src/lib.rs", "")
        .file(
            "Cargo.toml",
            r#"
             [package]
             name = "foo"
             version = "1.0.0"
             authors = []
             edition = "2015"
             "#,
        )
        .file(
            ".cargo/config.toml",
            r#"
             [build]
             build-dir = "foo/{workspace-path-hash}/build-dir"
             "#,
        )
        .build();

    // Build from the non-symlinked directory
    p.cargo("-Zbuild-dir-new-layout check")
        .masquerade_as_nightly_cargo(&["new build-dir layout"])
        .enable_mac_dsym()
        .run();

    // Parse and verify the hash dir created from the non-symlinked dir
    let foo_dir = p.root().join("foo");
    assert_exists(&foo_dir);
    let original_hash_dir = parse_workspace_manifest_path_hash(&foo_dir);

    original_hash_dir.assert_build_dir_layout(str![[r#"
[ROOT]/foo/foo/[HASH]/build-dir/.rustc_info.json
[ROOT]/foo/foo/[HASH]/build-dir/CACHEDIR.TAG
[ROOT]/foo/foo/[HASH]/build-dir/debug/.cargo-lock
[ROOT]/foo/foo/[HASH]/build-dir/debug/build/foo/[HASH]/fingerprint/dep-lib-foo
[ROOT]/foo/foo/[HASH]/build-dir/debug/build/foo/[HASH]/fingerprint/invoked.timestamp
[ROOT]/foo/foo/[HASH]/build-dir/debug/build/foo/[HASH]/fingerprint/lib-foo
[ROOT]/foo/foo/[HASH]/build-dir/debug/build/foo/[HASH]/fingerprint/lib-foo.json
[ROOT]/foo/foo/[HASH]/build-dir/debug/build/foo/[HASH]/deps/foo-[HASH].d
[ROOT]/foo/foo/[HASH]/build-dir/debug/build/foo/[HASH]/deps/libfoo-[HASH].rmeta

"#]]);

    p.root().join("target").assert_build_dir_layout(str![[r#"
[ROOT]/foo/target/CACHEDIR.TAG
[ROOT]/foo/target/debug/.cargo-lock

"#]]);

    // Create a symlink of the project root.
    let mut symlinked_dir = p.root().clone();
    symlinked_dir.pop();
    symlinked_dir = symlinked_dir.join("symlink-dir");
    symlink(p.root(), &symlinked_dir).unwrap();

    // Remove the foo dir (which contains the build-dir) before we rebuild from a symlinked dir.
    foo_dir.rm_rf();

    // Run cargo from the symlinked dir
    p.cargo("-Zbuild-dir-new-layout check")
        .masquerade_as_nightly_cargo(&["new build-dir layout"])
        .enable_mac_dsym()
        .cwd(&symlinked_dir)
        .run();

    // Parse and verify the hash created from the symlinked dir
    assert_exists(&foo_dir);
    let symlink_hash_dir = parse_workspace_manifest_path_hash(&foo_dir);

    symlink_hash_dir.assert_build_dir_layout(str![[r#"
[ROOT]/foo/foo/[HASH]/build-dir/.rustc_info.json
[ROOT]/foo/foo/[HASH]/build-dir/CACHEDIR.TAG
[ROOT]/foo/foo/[HASH]/build-dir/debug/.cargo-lock
[ROOT]/foo/foo/[HASH]/build-dir/debug/build/foo/[HASH]/fingerprint/dep-lib-foo
[ROOT]/foo/foo/[HASH]/build-dir/debug/build/foo/[HASH]/fingerprint/invoked.timestamp
[ROOT]/foo/foo/[HASH]/build-dir/debug/build/foo/[HASH]/fingerprint/lib-foo
[ROOT]/foo/foo/[HASH]/build-dir/debug/build/foo/[HASH]/fingerprint/lib-foo.json
[ROOT]/foo/foo/[HASH]/build-dir/debug/build/foo/[HASH]/deps/foo-[HASH].d
[ROOT]/foo/foo/[HASH]/build-dir/debug/build/foo/[HASH]/deps/libfoo-[HASH].rmeta

"#]]);

    p.root().join("target").assert_build_dir_layout(str![[r#"
[ROOT]/foo/target/CACHEDIR.TAG
[ROOT]/foo/target/debug/.cargo-lock

"#]]);

    // Verify the hash dir created from the symlinked and non-symlinked dirs are the same.
    assert_eq!(original_hash_dir, symlink_hash_dir);
}

#[cargo_test]
fn template_should_handle_reject_unmatched_brackets() {
    let p = project()
        .file("src/lib.rs", "")
        .file(
            ".cargo/config.toml",
            r#"
            [build]
            build-dir = "foo/{bar"
            "#,
        )
        .build();

    p.cargo("-Zbuild-dir-new-layout build")
        .masquerade_as_nightly_cargo(&["new build-dir layout"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] unexpected opening bracket `{` in build.build-dir path `foo/{bar`

"#]])
        .run();

    let p = project()
        .file("src/lib.rs", "")
        .file(
            ".cargo/config.toml",
            r#"
            [build]
            build-dir = "foo/}bar"
            "#,
        )
        .build();

    p.cargo("build")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] unexpected closing bracket `}` in build.build-dir path `foo/}bar`

"#]])
        .run();
}

fn parse_workspace_manifest_path_hash(hash_dir: &PathBuf) -> PathBuf {
    // Since the hash will change between test runs simply find the first directories and assume
    // that is the hash dir. The format is a 2 char directory followed by the remaining hash in the
    // inner directory (ie. `34/f9d02eb8411c05`)
    let mut dirs = std::fs::read_dir(hash_dir).unwrap().into_iter();
    let outer_hash_dir = dirs.next().unwrap().unwrap();
    // Validate there are no other directories in `hash_dir`
    assert!(
        dirs.next().is_none(),
        "Found multiple dir entries in {hash_dir:?}"
    );
    // Validate the outer hash dir hash is a directory and has the correct hash length
    assert!(
        outer_hash_dir.path().is_dir(),
        "{outer_hash_dir:?} was not a directory"
    );
    assert_eq!(
        outer_hash_dir.path().file_name().unwrap().len(),
        2,
        "Path {:?} should have been 2 chars",
        outer_hash_dir.path().file_name()
    );

    let mut dirs = std::fs::read_dir(outer_hash_dir.path())
        .unwrap()
        .into_iter();
    let inner_hash_dir = dirs.next().unwrap().unwrap();
    // Validate there are no other directories in first hash dir
    assert!(
        dirs.next().is_none(),
        "Found multiple dir entries in {outer_hash_dir:?}"
    );
    // Validate the outer hash dir hash is a directory and has the correct hash length
    assert!(
        inner_hash_dir.path().is_dir(),
        "{inner_hash_dir:?} was not a directory"
    );
    assert_eq!(
        inner_hash_dir.path().file_name().unwrap().len(),
        14,
        "Path {:?} should have been 2 chars",
        inner_hash_dir.path().file_name()
    );
    return inner_hash_dir.path();
}

#[track_caller]
fn assert_exists(path: &PathBuf) {
    assert!(
        path.exists(),
        "Expected `{}` to exist but was not found.",
        path.display()
    );
}

#[track_caller]
fn assert_not_exists(path: &PathBuf) {
    assert!(
        !path.exists(),
        "Expected `{}` to NOT exist but was found.",
        path.display()
    );
}

#[track_caller]
fn assert_exists_patterns_with_base_dir(base: &PathBuf, patterns: &[&str]) {
    let root = base.to_str().unwrap();
    let p: Vec<_> = patterns.iter().map(|p| format!("{root}/{p}")).collect();
    let p: Vec<&str> = p.iter().map(|v| v.as_str()).collect();
    assert_exists_patterns(&p);
}

#[track_caller]
fn assert_exists_patterns(patterns: &[&str]) {
    for p in patterns {
        assert_exists_pattern(p);
    }
}

#[track_caller]
fn assert_exists_pattern(pattern: &str) {
    use glob::glob;

    let mut z = glob(pattern).unwrap();

    assert!(
        z.next().is_some(),
        "Expected `{pattern}` to match existing file but was not found.",
    )
}
