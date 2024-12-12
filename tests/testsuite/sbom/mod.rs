//! Tests for cargo-sbom precursor files.

use std::path::PathBuf;

use cargo_test_support::basic_bin_manifest;
use cargo_test_support::cargo_test;
use cargo_test_support::compare::assert_e2e;
use cargo_test_support::project;
use cargo_test_support::registry::Package;
use cargo_test_support::ProjectBuilder;

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
    let output = std::fs::read_to_string(file).unwrap();
    assert_e2e().eq(output, snapbox::file!["build_sbom_using_cargo_config.json"]);
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

    let output = std::fs::read_to_string(sbom_path).unwrap();
    assert_e2e().eq(output, snapbox::file!["build_sbom_with_multiple_crate_types.json"]);
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

    let output = std::fs::read_to_string(path).unwrap();
    assert_e2e().eq(output, snapbox::file!["build_sbom_with_simple_build_script.json"]);
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
    let output = std::fs::read_to_string(path).unwrap();
    assert_e2e().eq(output, snapbox::file!["build_sbom_with_build_dependencies.json"]);
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
    let output = std::fs::read_to_string(path).unwrap();
    assert_e2e().eq(output, snapbox::file!["build_sbom_crate_uses_different_features_for_build_and_normal_dependencies.json"]);
}
