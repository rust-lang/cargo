//! Tests for cargo-sbom precursor files.

use std::path::PathBuf;

use cargo_test_support::basic_bin_manifest;
use cargo_test_support::compare;
use cargo_test_support::project;
use cargo_test_support::registry::Package;
use cargo_test_support::ProjectBuilder;

/// Helper function to compare expected JSON output against actual.
#[track_caller]
fn assert_json_output(actual_json_file: PathBuf, expected_json: &str) {
    assert!(actual_json_file.is_file());
    let actual_json = std::fs::read_to_string(actual_json_file).expect("Failed to read file");
    if let Err(error) = compare::match_json(expected_json, &actual_json, None) {
        panic!("{}", error.to_string());
    }
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
        .with_stderr(
            "\
            warning: ignoring 'sbom' config, pass `-Zsbom` to enable it\n\
            [COMPILING] foo v0.5.0 ([..])\n\
            [FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [..]\n",
        )
        .run();

    let file = p.bin("foo").with_extension("cargo-sbom.json");
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

    let file = p.bin("foo").with_extension("cargo-sbom.json");
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
                "crate_type": "bin",
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
                "panic": "unwind",
                "strip": {
                    "deferred": "None"
                }
            },
            "packages": [],
            "features": [],
            "rustc": {
                "version": "[..]",
                "wrapper": null,
                "commit_hash": "[..]",
                "host": "[..]"
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

    let file = p.bin("foo").with_extension("cargo-sbom.json");
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

    assert!(p.bin("foo").with_extension("cargo-sbom.json").is_file());
    assert_eq!(
        2,
        p.glob(p.target_debug_dir().join("*.cargo-sbom.json"))
            .count()
    );
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
            r#"fn main() { println!("cargo::rustc-cfg=foo"); }"#,
        )
        .build();

    p.cargo("build -Zsbom")
        .masquerade_as_nightly_cargo(&["sbom"])
        .run();

    let path = p.bin("foo").with_extension("cargo-sbom.json");
    assert!(path.is_file());
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
            r#"fn main() { println!("cargo::rustc-cfg=foo"); }"#,
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

    let path = p.bin("foo").with_extension("cargo-sbom.json");
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
                "crate_type": "bin",
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
                "panic": "unwind",
                "strip": {
                    "deferred": "None"
                }
            },
            "packages": [
                {
                    "package_id": "bar 0.1.0 (registry+[..])",
                    "package": "bar",
                    "version": "0.1.0",
                    "features": [],
                    "build_type": "normal",
                    "extern_crate_name": "bar",
                    "dependencies": [
                        {
                            "name": "bar",
                            "package_id": "bar 0.1.0 (registry+[..])",
                            "version": "0.1.0",
                            "features": []
                        }
                    ]
                },
                {
                    "package_id": "bar 0.1.0 (registry+[..])",
                    "package": "bar",
                    "version": "0.1.0",
                    "features": [],
                    "build_type": "build",
                    "extern_crate_name": "build_script_build",
                    "dependencies": [
                        {
                            "name": "bar",
                            "package_id": "bar 0.1.0 (registry+[..])",
                            "version": "0.1.0",
                            "features": []
                        }
                    ]
                },
                {
                    "package_id": "bar 0.1.0 (registry+[..])",
                    "package": "bar",
                    "version": "0.1.0",
                    "features": [],
                    "build_type": "normal",
                    "extern_crate_name": "build_script_build",
                    "dependencies": [
                        {
                            "name": "baz",
                            "package_id": "baz 0.1.0 (registry+[..])",
                            "version": "0.1.0",
                            "features": []
                        }
                    ]
                },
                {
                    "package_id": "baz 0.1.0 (registry+[..])",
                    "package": "baz",
                    "version": "0.1.0",
                    "features": [],
                    "build_type": "normal",
                    "extern_crate_name": "baz",
                    "dependencies": []
                }
            ],
            "features": [],
            "rustc": {
                "version": "[..]",
                "wrapper": null,
                "commit_hash": "[..]",
                "host": "[..]"
            }
        }"#,
    );
}
