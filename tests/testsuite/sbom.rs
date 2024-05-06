//! Tests for cargo-sbom precursor files.

use cargo_test_support::{basic_bin_manifest, project, ProjectBuilder};

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
    assert!(file.is_file());
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
        1,
        p.glob(p.target_debug_dir().join("libfoo.cargo-sbom.json"))
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
    let p = configured_project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies]
                bar = { path = "./bar" }
            "#,
        )
        .file("src/main.rs", "fn main() { let _i = bar::bar(); }")
        .file("bar/src/lib.rs", "pub fn bar() -> i32 { 2 }")
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.1.0"
                build = "build.rs"

                [build-dependencies]
                cc = "1.0.46"
            "#,
        )
        .file(
            "bar/build.rs",
            r#"fn main() { println!("cargo::rustc-cfg=foo"); }"#,
        )
        .build();

    p.cargo("build -Zsbom")
        .masquerade_as_nightly_cargo(&["sbom"])
        .run();

    let path = p.bin("foo").with_extension("cargo-sbom.json");
    assert!(path.is_file());
}
