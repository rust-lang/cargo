//! Tests for `build.build-dir` config property.
//!
//! The testing strategy for build-dir functionality is primarily checking if directories / files
//! are in the expected locations.
//! The rational is that other tests will verify each individual feature, while the tests in this
//! file verify the files saved to disk are in the correct locations according to the `build-dir`
//! configuration.
//!
//! Tests check if directories match some "layout" by using [`assert_build_dir_layout`] and
//! [`assert_artifact_dir_layout`].

use std::path::PathBuf;

use cargo_test_support::prelude::*;
use cargo_test_support::project;
use std::env::consts::{DLL_PREFIX, DLL_SUFFIX, EXE_SUFFIX};

#[cargo_test]
fn verify_build_dir_is_disabled_by_feature_flag() {
    let p = project()
        .file("src/main.rs", r#"fn main() { println!("Hello, World!") }"#)
        .file(
            ".cargo/config.toml",
            r#"
            [build]
            build-dir = "build-dir"
            "#,
        )
        .build();

    p.cargo("build")
        .masquerade_as_nightly_cargo(&["build-dir"])
        .enable_mac_dsym()
        .run();

    assert_build_dir_layout(p.root().join("target"), "debug");
    assert_exists(&p.root().join(format!("target/debug/foo{EXE_SUFFIX}")));
    assert_exists(&p.root().join("target/debug/foo.d"));
    assert_not_exists(&p.root().join("build-dir"));
}

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

    p.cargo("build -Z build-dir")
        .masquerade_as_nightly_cargo(&["build-dir"])
        .enable_mac_dsym()
        .run();

    assert_build_dir_layout(p.root().join("build-dir"), "debug");
    assert_artifact_dir_layout(p.root().join("target-dir"), "debug");
    assert_exists_patterns_with_base_dir(
        &p.root(),
        &[
            // Check the pre-uplifted binary in the build-dir
            &format!("build-dir/debug/deps/foo*{EXE_SUFFIX}"),
            "build-dir/debug/deps/foo*.d",
            // Verify the binary was copied to the target-dir
            &format!("target-dir/debug/foo{EXE_SUFFIX}"),
            "target-dir/debug/foo.d",
        ],
    );
    assert_not_exists(&p.root().join("target"));
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

    p.cargo("build --release -Z build-dir")
        .masquerade_as_nightly_cargo(&["build-dir"])
        .enable_mac_dsym()
        .run();

    assert_build_dir_layout(p.root().join("build-dir"), "release");
    assert_exists(&p.root().join(format!("target-dir/release/foo{EXE_SUFFIX}")));
    assert_exists_patterns_with_base_dir(
        &p.root(),
        &[
            // Check the pre-uplifted binary in the build-dir
            &format!("build-dir/release/deps/foo*{EXE_SUFFIX}"),
            "build-dir/release/deps/foo*.d",
            // Verify the binary was copied to the target-dir
            &format!("target-dir/release/foo{EXE_SUFFIX}"),
            "target-dir/release/foo.d",
        ],
    );
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

        p.cargo("build -Z build-dir")
            .masquerade_as_nightly_cargo(&["build-dir"])
            .enable_mac_dsym()
            .run();

        assert_build_dir_layout(p.root().join("build-dir"), "debug");

        // Verify lib artifacts were copied into the artifact dir
        assert_exists_patterns_with_base_dir(&p.root().join("target-dir/debug"), &expected_files);
    }
}

#[cargo_test]
fn should_default_to_target() {
    let p = project()
        .file("src/main.rs", r#"fn main() { println!("Hello, World!") }"#)
        .build();

    p.cargo("build -Z build-dir")
        .masquerade_as_nightly_cargo(&["build-dir"])
        .enable_mac_dsym()
        .run();

    assert_build_dir_layout(p.root().join("target"), "debug");
    assert_exists(&p.root().join(format!("target/debug/foo{EXE_SUFFIX}")));
}

#[cargo_test]
fn should_respect_env_var() {
    let p = project()
        .file("src/main.rs", r#"fn main() { println!("Hello, World!") }"#)
        .build();

    p.cargo("build -Z build-dir")
        .masquerade_as_nightly_cargo(&["build-dir"])
        .env("CARGO_BUILD_BUILD_DIR", "build-dir")
        .enable_mac_dsym()
        .run();

    assert_build_dir_layout(p.root().join("build-dir"), "debug");
    assert_exists(&p.root().join(format!("target/debug/foo{EXE_SUFFIX}")));
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

    p.cargo("build -Z build-dir")
        .masquerade_as_nightly_cargo(&["build-dir"])
        .enable_mac_dsym()
        .run();

    assert_build_dir_layout(p.root().join("build-dir"), "debug");
    assert_exists_patterns_with_base_dir(
        &p.root(),
        &[
            &format!("build-dir/debug/build/foo-*/build-script-build{EXE_SUFFIX}"),
            "build-dir/debug/build/foo-*/out/foo.txt", // Verify OUT_DIR
        ],
    );
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

    p.cargo("test -Z build-dir")
        .masquerade_as_nightly_cargo(&["build-dir"])
        .enable_mac_dsym()
        .run();

    assert_build_dir_layout(p.root().join("build-dir"), "debug");
    assert_exists(&p.root().join(format!("build-dir/tmp/foo.txt")));
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

    p.cargo("build --examples -Z build-dir")
        .masquerade_as_nightly_cargo(&["build-dir"])
        .enable_mac_dsym()
        .run();

    assert_build_dir_layout(p.root().join("build-dir"), "debug");
    assert_exists_patterns_with_base_dir(
        &p.root(),
        &[
            // uplifted (target-dir)
            &format!("target-dir/debug/examples/foo{EXE_SUFFIX}"),
            "target-dir/debug/examples/foo.d",
            // pre-uplifted (build-dir)
            &format!("build-dir/debug/examples/foo*{EXE_SUFFIX}"),
            "build-dir/debug/examples/foo*.d",
        ],
    );
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

    p.cargo("build --bench=foo -Z build-dir")
        .masquerade_as_nightly_cargo(&["build-dir"])
        .enable_mac_dsym()
        .run();

    assert_build_dir_layout(p.root().join("build-dir"), "debug");
    assert_exists_patterns_with_base_dir(
        &p.root(),
        &[
            &format!("build-dir/debug/deps/foo*{EXE_SUFFIX}"),
            "build-dir/debug/deps/foo*.d",
        ],
    );
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

    p.cargo("doc -Z build-dir")
        .masquerade_as_nightly_cargo(&["build-dir"])
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

    p.cargo("package -Z build-dir")
        .masquerade_as_nightly_cargo(&["build-dir"])
        .enable_mac_dsym()
        .run();

    assert_build_dir_layout(p.root().join("build-dir"), "debug");

    let package_artifact_dir = p.root().join("target-dir/package");
    assert_exists(&package_artifact_dir);
    assert_exists(&package_artifact_dir.join("foo-0.0.1.crate"));
    assert!(package_artifact_dir.join("foo-0.0.1.crate").is_file());

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

    p.cargo("build -Z build-dir")
        .masquerade_as_nightly_cargo(&["build-dir"])
        .enable_mac_dsym()
        .run();

    assert_build_dir_layout(p.root().join("build-dir"), "debug");

    p.cargo("clean -Z build-dir")
        .masquerade_as_nightly_cargo(&["build-dir"])
        .enable_mac_dsym()
        .run();

    assert_not_exists(&p.root().join("build-dir"));
    assert_not_exists(&p.root().join("target-dir"));
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

    p.cargo("build --timings -Z build-dir")
        .masquerade_as_nightly_cargo(&["build-dir"])
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

    p.cargo("build -Z build-dir")
        .masquerade_as_nightly_cargo(&["build-dir"])
        .arg("--future-incompat-report")
        .env("RUSTFLAGS", "-Zfuture-incompat-test")
        .run();

    assert_exists(&p.root().join("build-dir/.future-incompat-report.json"));
}

#[track_caller]
fn assert_build_dir_layout(path: PathBuf, profile: &str) {
    assert_dir_layout(path, profile, true);
}

#[allow(dead_code)]
#[track_caller]
fn assert_artifact_dir_layout(path: PathBuf, profile: &str) {
    assert_dir_layout(path, profile, false);
}

#[track_caller]
fn assert_dir_layout(path: PathBuf, profile: &str, is_build_dir: bool) {
    println!("checking if {path:?} is a build directory ({is_build_dir})");
    // For things that are in both `target` and the build directory we only check if they are
    // present if `is_build_dir` is true.
    if is_build_dir {
        assert_eq!(
            is_build_dir,
            path.join(profile).is_dir(),
            "Expected {:?} to exist and be a directory",
            path.join(profile)
        );
    }

    let error_message = |dir: &str| {
        if is_build_dir {
            format!("`{dir}` dir was expected but not found")
        } else {
            format!("`{dir}` dir was not expected but was found")
        }
    };

    if is_build_dir {
        assert_exists(&path.join(".rustc_info.json"));
    } else {
        assert_not_exists(&path.join(".rustc_info.json"));
    }

    assert_eq!(
        is_build_dir,
        path.join(profile).join("deps").is_dir(),
        "{}",
        error_message("deps")
    );
    assert_eq!(
        is_build_dir,
        path.join(profile).join("build").is_dir(),
        "{}",
        error_message("build")
    );
    assert_eq!(
        is_build_dir,
        path.join(profile).join("incremental").is_dir(),
        "{}",
        error_message("incremental")
    );
    assert_eq!(
        is_build_dir,
        path.join(profile).join(".fingerprint").is_dir(),
        "{}",
        error_message(".fingerprint")
    );
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
