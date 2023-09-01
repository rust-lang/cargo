//! Tests for `-Ztrim-paths`.

use cargo_test_support::project;

#[cargo_test]
fn gated_manifest() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"

                [profile.dev]
                trim-paths = "macro"
           "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check")
        .masquerade_as_nightly_cargo(&["-Ztrim-paths"])
        .with_status(101)
        .with_stderr_contains(
            "\
[ERROR] failed to parse manifest at `[CWD]/Cargo.toml`

Caused by:
  feature `trim-paths` is required",
        )
        .run();
}

#[cargo_test]
fn gated_config_toml() {
    let p = project()
        .file(
            ".cargo/config.toml",
            r#"
                [profile.dev]
                trim-paths = "macro"
           "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check")
        .masquerade_as_nightly_cargo(&["-Ztrim-paths"])
        .with_status(101)
        .with_stderr_contains(
            "\
[ERROR] config profile `dev` is not valid (defined in `[CWD]/.cargo/config.toml`)

Caused by:
  feature `trim-paths` is required",
        )
        .run();
}
