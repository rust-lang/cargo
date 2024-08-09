//! Tests for cargo-sbom precursor files.

use std::path::PathBuf;

use cargo_test_support::basic_bin_manifest;
use cargo_test_support::cargo_test;
use cargo_test_support::compare::match_json;
use cargo_test_support::project;
use cargo_test_support::registry::Package;
use cargo_test_support::ProjectBuilder;

/// Helper function to compare expected JSON output against actual.
#[track_caller]
fn assert_json_output(actual_json_file: PathBuf, expected_json: &str) {
    assert!(actual_json_file.is_file());
    let actual_json = std::fs::read_to_string(actual_json_file).expect("Failed to read file");
    let actual_json: serde_json::Value =
        serde_json::from_str(actual_json.as_str()).expect("Failed to parse JSON");
    let actual_json = serde_json::to_string(&actual_json).expect("Failed to convert JSON");

    if let Err(error) = match_json(expected_json, &actual_json, None) {
        panic!("{}", error.to_string());
    }
}

const SBOM_FILE_EXTENSION: &str = ".cargo-sbom.json";

fn with_sbom_suffix(link: &PathBuf) -> PathBuf {
    let mut link_buf = link.clone().into_os_string();
    link_buf.push(SBOM_FILE_EXTENSION);
    PathBuf::from(link_buf)
}

fn configured_project() -> ProjectBuilder {
    project().file(
        ".cargo/config.toml",
        r#"
            [build]
            sbom = true
        "#,
    )
}

#[cargo_test]
fn build_sbom_without_passing_unstable_flag() {
    let p = configured_project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", r#"fn main() {}"#)
        .build();

    p.cargo("build")
        .masquerade_as_nightly_cargo(&["sbom"])
        .with_stderr_data(
            "\
            [WARNING] ignoring 'sbom' config, pass `-Zsbom` to enable it\n\
            [COMPILING] foo v0.5.0 ([..])\n\
            [FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [..]\n",
        )
        .run();

    let file = with_sbom_suffix(&p.bin("foo"));
    assert!(!file.exists());
}

#[cargo_test]
fn build_sbom_using_cargo_config() {
    let p = configured_project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", r#"fn main() {}"#)
        .build();

    p.cargo("build -Zsbom")
        .masquerade_as_nightly_cargo(&["sbom"])
        .run();

    let file = with_sbom_suffix(&p.bin("foo"));
    assert_json_output(
        file,
        r#"
        {
            "format_version": 1,
            "package_id": "path+file:///[..]/foo#0.5.0",
            "name": "foo",
            "version": "0.5.0",
            "source": "[ROOT]/foo",
            "target": {
                "kind": [
                    "bin"
                ],
                "crate_types": [
                    "bin"
                ],
                "name": "foo",
                "edition": "2015"
            },
            "profile": {
                "name": "dev",
                "opt_level": "0",
                "lto": "false",
                "codegen_backend": null,
                "codegen_units": null,
                "debuginfo": 2,
                "split_debuginfo": "{...}",
                "debug_assertions": true,
                "overflow_checks": true,
                "rpath": false,
                "incremental": false,
                "panic": "unwind"
            },
            "packages": [],
            "features": [],
            "rustc": {
                "version": "[..]",
                "wrapper": null,
                "workspace_wrapper": null,
                "commit_hash": "[..]",
                "host": "[..]",
                "verbose_version": "{...}"
            }
        }
        "#,
    );
}

#[cargo_test]
fn build_sbom_using_env_var() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", r#"fn main() {}"#)
        .build();

    p.cargo("build -Zsbom")
        .env("CARGO_BUILD_SBOM", "true")
        .masquerade_as_nightly_cargo(&["sbom"])
        .run();

    let file = with_sbom_suffix(&p.bin("foo"));
    assert!(file.is_file());
}

#[cargo_test]
fn build_sbom_project_bin_and_lib() {
    let p = configured_project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "1.2.3"
                authors = []

                [lib]
                crate-type = ["rlib"]
            "#,
        )
        .file("src/main.rs", r#"fn main() { let _i = foo::give_five(); }"#)
        .file("src/lib.rs", r#"pub fn give_five() -> i32 { 5 }"#)
        .build();

    p.cargo("build -Zsbom")
        .masquerade_as_nightly_cargo(&["sbom"])
        .run();

    assert!(with_sbom_suffix(&p.bin("foo")).is_file());
    assert_eq!(
        2,
        p.glob(p.target_debug_dir().join("*.cargo-sbom.json"))
            .count()
    );
}

#[cargo_test]
fn build_sbom_with_artifact_name_conflict() {
    Package::new("deps", "0.1.0")
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "deps" # name conflict
                version = "0.1.0"
                authors = []
            "#,
        )
        .file("src/lib.rs", "pub fn bar() -> i32 { 2 }")
        .publish();

    let p = configured_project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies]
                deps = "0.1.0"
            "#,
        )
        .file("src/main.rs", "fn main() { let _i = deps::bar(); }")
        .build();

    p.cargo("build -Zsbom")
        .masquerade_as_nightly_cargo(&["sbom"])
        .run();
}

#[cargo_test]
fn build_sbom_with_multiple_crate_types() {
    let p = configured_project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "1.2.3"
                authors = []

                [lib]
                crate-type = ["dylib", "rlib"]
            "#,
        )
        .file("src/main.rs", r#"fn main() { let _i = foo::give_five(); }"#)
        .file("src/lib.rs", r#"pub fn give_five() -> i32 { 5 }"#)
        .build();

    p.cargo("build -Zsbom")
        .masquerade_as_nightly_cargo(&["sbom"])
        .run();

    assert_eq!(
        3,
        p.glob(p.target_debug_dir().join("*.cargo-sbom.json"))
            .count()
    );

    let sbom_path = with_sbom_suffix(&p.dylib("foo"));
    assert!(sbom_path.is_file());

    assert_json_output(
        sbom_path,
        r#"
        {
            "format_version": 1,
            "package_id": "path+file://[..]/foo#1.2.3",
            "name": "foo",
            "version": "1.2.3",
            "source": "[ROOT]/foo",
            "target": {
               "kind": [
                    "dylib",
                    "rlib"
                ],
                "crate_types": [
                    "dylib",
                    "rlib"
                ],
                "name": "foo",
                "edition": "2015"
            },
            "profile": {
                "name": "dev",
                "opt_level": "0",
                "lto": "false",
                "codegen_backend": null,
                "codegen_units": null,
                "debuginfo": 2,
                "split_debuginfo": "{...}",
                "debug_assertions": true,
                "overflow_checks": true,
                "rpath": false,
                "incremental": false,
                "panic": "unwind"
            },
            "packages": [],
            "features": [],
            "rustc": {
                "version": "[..]",
                "wrapper": null,
                "workspace_wrapper": null,
                "commit_hash": "[..]",
                "host": "[..]",
                "verbose_version": "{...}"
            }
        }
        "#,
    )
}

#[cargo_test]
fn build_sbom_with_simple_build_script() {
    let p = configured_project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []
                build = "build.rs"
            "#,
        )
        .file("src/main.rs", "#[cfg(foo)] fn main() {}")
        .file(
            "build.rs",
            r#"fn main() {
                println!("cargo::rustc-check-cfg=cfg(foo)");
                println!("cargo::rustc-cfg=foo");
            }"#,
        )
        .build();

    p.cargo("build -Zsbom")
        .masquerade_as_nightly_cargo(&["sbom"])
        .run();

    let path = with_sbom_suffix(&p.bin("foo"));
    assert!(path.is_file());

    assert_json_output(
        path,
        r#"
        {
            "format_version": 1,
            "package_id": "path+file://[..]/foo#0.0.1",
            "name": "foo",
            "version": "0.0.1",
            "source": "[ROOT]/foo",
            "target": {
                "kind": [
                    "bin"
                ],
                "crate_types": [
                    "bin"
                ],
                "name": "foo",
                "edition": "2015"
            },
            "profile": {
                "name": "dev",
                "opt_level": "0",
                "lto": "false",
                "codegen_backend": null,
                "codegen_units": null,
                "debuginfo": 2,
                "split_debuginfo": "{...}",
                "debug_assertions": true,
                "overflow_checks": true,
                "rpath": false,
                "incremental": false,
                "panic": "unwind"
            },
            "packages": [
                {
                    "build_type": "build",
                    "dependencies": [],
                    "extern_crate_name": "build_script_build",
                    "features": [],
                    "package": "foo",
                    "package_id": "path+file://[..]/foo#0.0.1",
                    "profile": {
                        "codegen_backend": null,
                        "codegen_units": null,
                        "debug_assertions": false,
                        "debuginfo": 2,
                        "incremental": false,
                        "lto": "false",
                        "name": "dev",
                        "opt_level": "0",
                        "overflow_checks": false,
                        "panic": "unwind",
                        "rpath": false,
                        "split_debuginfo": null
                    },
                    "version": "0.0.1"
                },
                {
                    "package_id": "path+file://[..]/foo#0.0.1",
                    "package": "foo",
                    "version": "0.0.1",
                    "features": [],
                    "build_type": "normal",
                    "extern_crate_name": "build_script_build",
                    "dependencies": [0],
                    "profile": {
                        "codegen_backend": null,
                        "codegen_units": null,
                        "debug_assertions": true,
                        "debuginfo": 0,
                        "incremental": false,
                        "lto": "false",
                        "name": "dev",
                        "opt_level": "0",
                        "overflow_checks": true,
                        "panic": "unwind",
                        "rpath": false,
                        "split_debuginfo": "{...}"
                    }
                }
            ],
            "features": [],
            "rustc": {
                "version": "[..]",
                "wrapper": null,
                "workspace_wrapper": null,
                "commit_hash": "[..]",
                "host": "[..]",
                "verbose_version": "{...}"
            }
        }"#,
    );
}

#[cargo_test]
fn build_sbom_with_build_dependencies() {
    Package::new("baz", "0.1.0").publish();
    Package::new("bar", "0.1.0")
        .build_dep("baz", "0.1.0")
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.1.0"
                build = "build.rs"

                [build-dependencies]
                baz = "0.1.0"
            "#,
        )
        .file("src/lib.rs", "pub fn bar() -> i32 { 2 }")
        .file(
            "build.rs",
            r#"fn main() {
                println!("cargo::rustc-check-cfg=cfg(foo)");
                println!("cargo::rustc-cfg=foo");
            }"#,
        )
        .publish();

    let p = configured_project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies]
                bar = "0.1.0"
            "#,
        )
        .file("src/main.rs", "fn main() { let _i = bar::bar(); }")
        .build();

    p.cargo("build -Zsbom")
        .masquerade_as_nightly_cargo(&["sbom"])
        .run();

    let path = with_sbom_suffix(&p.bin("foo"));
    assert_json_output(
        path,
        r#"
        {
            "format_version": 1,
            "package_id": "path+file:///[..]/foo#0.0.1",
            "name": "foo",
            "version": "0.0.1",
            "source": "[ROOT]/foo",
            "target": {
                "kind": [
                    "bin"
                ],
                "crate_types": [
                    "bin"
                ],
                "name": "foo",
                "edition": "2015"
            },
            "profile": {
                "name": "dev",
                "opt_level": "0",
                "lto": "false",
                "codegen_backend": null,
                "codegen_units": null,
                "debuginfo": 2,
                "split_debuginfo": "{...}",
                "debug_assertions": true,
                "overflow_checks": true,
                "rpath": false,
                "incremental": false,
                "panic": "unwind"
            },
            "packages": [
                {
                    "package_id": "registry+[..]#bar@0.1.0",
                    "package": "bar",
                    "version": "0.1.0",
                    "features": [],
                    "build_type": "normal",
                    "extern_crate_name": "bar",
                    "dependencies": [2]
                },
                {
                    "package_id": "registry+[..]#bar@0.1.0",
                    "package": "bar",
                    "version": "0.1.0",
                    "features": [],
                    "build_type": "build",
                    "extern_crate_name": "build_script_build",
                    "dependencies": [3],
                    "profile": {
                        "codegen_backend": null,
                        "codegen_units": null,
                        "debug_assertions": false,
                        "debuginfo": 2,
                        "incremental": false,
                        "lto": "false",
                        "name": "dev",
                        "opt_level": "0",
                        "overflow_checks": false,
                        "panic": "unwind",
                        "rpath": false,
                        "split_debuginfo": "{...}"
                    }
                },
                {
                    "package_id": "registry+[..]#bar@0.1.0",
                    "package": "bar",
                    "version": "0.1.0",
                    "features": [],
                    "build_type": "normal",
                    "extern_crate_name": "build_script_build",
                    "dependencies": [1],
                    "profile": {
                        "codegen_backend": null,
                        "codegen_units": null,
                        "debug_assertions": true,
                        "debuginfo": 0,
                        "incremental": false,
                        "lto": "false",
                        "name": "dev",
                        "opt_level": "0",
                        "overflow_checks": true,
                        "panic": "unwind",
                        "rpath": false,
                        "split_debuginfo": "{...}"
                    }
                },
                {
                    "package_id": "registry+[..]#baz@0.1.0",
                    "package": "baz",
                    "version": "0.1.0",
                    "features": [],
                    "build_type": "normal",
                    "extern_crate_name": "baz",
                    "dependencies": [],
                    "profile": {
                        "codegen_backend": null,
                        "codegen_units": null,
                        "debug_assertions": true,
                        "debuginfo": 0,
                        "incremental": false,
                        "lto": "false",
                        "name": "dev",
                        "opt_level": "0",
                        "overflow_checks": true,
                        "panic": "unwind",
                        "rpath": false,
                        "split_debuginfo": "{...}"
                    }
                }
            ],
            "features": [],
            "rustc": {
                "version": "[..]",
                "wrapper": null,
                "workspace_wrapper": null,
                "commit_hash": "[..]",
                "host": "[..]",
                "verbose_version": "{...}"
            }
        }"#,
    );
}

#[cargo_test]
fn build_sbom_crate_uses_different_features_for_build_and_normal_dependencies() {
    let p = configured_project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "a"
                version = "0.1.0"
                edition = "2021"
                authors = []

                [dependencies]
                b = { path = "b/", features = ["f1"] }

                [build-dependencies]
                b = { path = "b/", features = ["f2"] }
            "#,
        )
        .file(
            "src/main.rs",
            r#"
                fn main() { b::f1(); }
            "#,
        )
        .file(
            "build.rs",
            r#"
                fn main() { b::f2(); }
            "#,
        )
        .file(
            "b/Cargo.toml",
            r#"
                [package]
                name = "b"
                version = "0.0.1"
                edition = "2021"

                [features]
                f1 = []
                f2 = []
            "#,
        )
        .file(
            "b/src/lib.rs",
            r#"
                #[cfg(feature = "f1")]
                pub fn f1() {}

                #[cfg(feature = "f2")]
                pub fn f2() {}
            "#,
        )
        .build();

    p.cargo("build -Zsbom")
        .masquerade_as_nightly_cargo(&["sbom"])
        .run();

    let path = with_sbom_suffix(&p.bin("a"));
    assert!(path.is_file());

    assert_json_output(
        path,
        r#"
        {
            "features": [],
            "format_version": 1,
            "package_id": "path+file:///[..]#a@0.1.0",
            "name": "a",
            "version": "0.1.0",
            "source": "[ROOT]/foo",
            "target": {
                "crate_types": [
                    "bin"
                ],
                "edition": "2021",
                "kind": [
                    "bin"
                ],
                "name": "a"
            },
            "packages": [
                {
                    "build_type": "build",
                    "dependencies": [2],
                    "extern_crate_name": "build_script_build",
                    "features": [],
                    "package": "a",
                    "package_id": "path+file:///[..]#a@0.1.0",
                    "profile": {
                        "codegen_backend": null,
                        "codegen_units": null,
                        "debug_assertions": false,
                        "debuginfo": 2,
                        "incremental": false,
                        "lto": "false",
                        "name": "dev",
                        "opt_level": "0",
                        "overflow_checks": false,
                        "panic": "unwind",
                        "rpath": false,
                        "split_debuginfo": null
                    },
                    "version": "0.1.0"
                },
                {
                    "build_type": "normal",
                    "dependencies": [0],
                    "extern_crate_name": "build_script_build",
                    "features": [],
                    "package": "a",
                    "package_id": "path+file:///[..]#a@0.1.0",
                    "profile": {
                        "codegen_backend": null,
                        "codegen_units": null,
                        "debug_assertions": true,
                        "debuginfo": 0,
                        "incremental": false,
                        "lto": "false",
                        "name": "dev",
                        "opt_level": "0",
                        "overflow_checks": true,
                        "panic": "unwind",
                        "rpath": false,
                        "split_debuginfo": "{...}"
                    },
                    "version": "0.1.0"
                },
                {
                    "build_type": "normal",
                    "dependencies": [],
                    "extern_crate_name": "b",
                    "features": [
                        "f2"
                    ],
                    "package": "b",
                    "package_id": "path+file:///[..]b#0.0.1",
                    "profile": {
                        "codegen_backend": null,
                        "codegen_units": null,
                        "debug_assertions": true,
                        "debuginfo": 0,
                        "incremental": false,
                        "lto": "false",
                        "name": "dev",
                        "opt_level": "0",
                        "overflow_checks": true,
                        "panic": "unwind",
                        "rpath": false,
                        "split_debuginfo": "{...}"
                    },
                    "version": "0.0.1"
                },
                {
                    "build_type": "normal",
                    "dependencies": [],
                    "extern_crate_name": "b",
                    "features": [
                        "f1"
                    ],
                    "package": "b",
                    "package_id": "path+file:///[..]b#0.0.1",
                    "version": "0.0.1"
                }
            ],
            "profile": {
                "codegen_backend": null,
                "codegen_units": null,
                "debug_assertions": true,
                "debuginfo": 2,
                "incremental": false,
                "lto": "false",
                "name": "dev",
                "opt_level": "0",
                "overflow_checks": true,
                "panic": "unwind",
                "rpath": false,
                "split_debuginfo": "{...}"
            },
            "rustc": {
              "commit_hash": "[..]",
              "host": "[..]",
              "verbose_version": "{...}",
              "version": "[..]",
              "workspace_wrapper": null,
              "wrapper": null
            }
        }"#,
    );
}
