//! Tests for selecting pre-release versions with `update --precise`.

use cargo_test_support::project;

#[cargo_test]
fn requires_nightly_cargo() {
    cargo_test_support::registry::init();

    for version in ["0.1.1", "0.1.2-pre.0"] {
        cargo_test_support::registry::Package::new("my-dependency", version).publish();
    }

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "package"
            [dependencies]
            my-dependency = "0.1.1"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("update -p my-dependency --precise 0.1.2-pre.0")
        .with_status(101)
        // This error is suffering from #12579 but still demonstrates that updating to
        // a pre-release does not work on stable
        .with_stderr(
            r#"[UPDATING] `dummy-registry` index
[ERROR] failed to select a version for the requirement `my-dependency = "^0.1.1"`
candidate versions found which didn't match: 0.1.2-pre.0
location searched: `dummy-registry` index (which is replacing registry `crates-io`)
required by package `package v0.0.0 ([ROOT]/foo)`
if you are looking for the prerelease package it needs to be specified explicitly
    my-dependency = { version = "0.1.2-pre.0" }
perhaps a crate was updated and forgotten to be re-vendored?"#,
        )
        .run()
}

#[cargo_test]
fn feature_exists() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "package"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("-Zprecise-pre-release update")
        .masquerade_as_nightly_cargo(&["precise-pre-release"])
        .with_stderr("")
        .run()
}
