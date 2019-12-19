//! -Zadvanced-env tests

use cargo_test_support::{paths, project, registry::Package};

#[cargo_test]
// I don't know why, but `Command` forces all env keys to be upper case on
// Windows. Seems questionable, since I think Windows is case-preserving.
#[cfg_attr(windows, ignore)]
fn source_config_env() {
    // Try to define [source] with environment variables.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"

            [dependencies]
            somedep = "1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    Package::new("somedep", "1.0.0")
        .local(true)
        .file("src/lib.rs", "")
        .publish();

    let path = paths::root().join("registry");

    p.cargo("check -Zadvanced-env")
        .masquerade_as_nightly_cargo()
        .env("CARGO_SOURCE_crates-io_REPLACE_WITH", "my-local-source")
        .env("CARGO_SOURCE_my-local-source_LOCAL_REGISTRY", path)
        .run();
}
