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
fn build_sbom_using_cargo_config() {
    let p = project()
        .file(
            ".cargo/config.toml",
            r#"
                [build]
                sbom = true
            "#,
        )
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", r#"fn main() {}"#)
        .build();

    p.cargo("build").run();

    let file = p.bin("foo").with_extension("cargo-sbom.json");
    assert!(file.is_file());
}

#[cargo_test]
fn build_sbom_using_env_var() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", r#"fn main() {}"#)
        .build();

    p.cargo("build").env("CARGO_BUILD_SBOM", "true").run();

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

    p.cargo("build").stream().run();

    assert!(p.bin("foo").with_extension("cargo-sbom.json").is_file());
    assert_eq!(
        1,
        p.glob(p.target_debug_dir().join("libfoo.cargo-sbom.json"))
            .count()
    );
}
