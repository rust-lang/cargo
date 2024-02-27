//! Tests for cargo-sbom precursor files.

use cargo_test_support::{basic_bin_manifest, project};

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
    dbg!(&file);
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
