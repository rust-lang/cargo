//! Tests for selecting pre-release versions with `update --precise`.

use crate::prelude::*;
use cargo_test_support::{project, str};

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

    p.cargo("update my-dependency --precise 0.1.2-pre.0")
        .with_status(101)
        // This error is suffering from #12579 but still demonstrates that updating to
        // a pre-release does not work on stable
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[ERROR] failed to select a version for the requirement `my-dependency = "^0.1.1"`
candidate versions found which didn't match: 0.1.2-pre.0
location searched: `dummy-registry` index (which is replacing registry `crates-io`)
required by package `package v0.0.0 ([ROOT]/foo)`
if you are looking for the prerelease package it needs to be specified explicitly
    my-dependency = { version = "0.1.2-pre.0" }
perhaps a crate was updated and forgotten to be re-vendored?

"#]])
        .run();
}

#[cargo_test]
fn update_pre_release() {
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

    p.cargo("update my-dependency --precise 0.1.2-pre.0 -Zunstable-options")
        .masquerade_as_nightly_cargo(&["precise-pre-release"])
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[UPDATING] my-dependency v0.1.1 -> v0.1.2-pre.0

"#]])
        .run();
    let lockfile = p.read_lockfile();
    assert!(lockfile.contains("\nname = \"my-dependency\"\nversion = \"0.1.2-pre.0\""));
}

#[cargo_test]
fn pre_release_should_unmatched() {
    cargo_test_support::registry::init();

    cargo_test_support::registry::Package::new("my-dependency", "0.1.2").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "package"
            [dependencies]
            my-dependency = "0.1.2"
            "#,
        )
        .file("src/lib.rs", "")
        .build();
    p.cargo("generate-lockfile").run();
    let lockfile = p.read_lockfile();
    assert!(lockfile.contains("\nname = \"my-dependency\"\nversion = \"0.1.2\""));

    // 0.1.2-pre.0 < 0.1.2 so it doesn't match
    cargo_test_support::registry::Package::new("my-dependency", "0.1.2-pre.0").publish();
    p.cargo("update -p my-dependency --precise 0.1.2-pre.0 -Zunstable-options")
        .masquerade_as_nightly_cargo(&["precise-pre-release"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[ERROR] failed to select a version for the requirement `my-dependency = "^0.1.2"`
candidate versions found which didn't match: 0.1.2-pre.0
location searched: `dummy-registry` index (which is replacing registry `crates-io`)
required by package `package v0.0.0 ([ROOT]/foo)`
if you are looking for the prerelease package it needs to be specified explicitly
    my-dependency = { version = "0.1.2-pre.0" }
perhaps a crate was updated and forgotten to be re-vendored?

"#]])
        .run();

    cargo_test_support::registry::Package::new("my-dependency", "0.2.0-0").publish();
    // 0.2.0-0 is the upper bound we exclude, so it doesn't match
    p.cargo("update -p my-dependency --precise 0.2.0-0 -Zunstable-options")
        .masquerade_as_nightly_cargo(&["precise-pre-release"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[ERROR] failed to select a version for the requirement `my-dependency = "^0.1.2"`
candidate versions found which didn't match: 0.2.0-0
location searched: `dummy-registry` index (which is replacing registry `crates-io`)
required by package `package v0.0.0 ([ROOT]/foo)`
if you are looking for the prerelease package it needs to be specified explicitly
    my-dependency = { version = "0.2.0-0" }
perhaps a crate was updated and forgotten to be re-vendored?

"#]])
        .run();

    let lockfile = p.read_lockfile();
    assert!(lockfile.contains("\nname = \"my-dependency\"\nversion = \"0.1.2\""));
}

#[cargo_test]
fn pre_release_should_matched() {
    cargo_test_support::registry::init();

    cargo_test_support::registry::Package::new("my-dependency", "0.1.2").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "package"
            [dependencies]
            my-dependency = "0.1.2"
            "#,
        )
        .file("src/lib.rs", "")
        .build();
    p.cargo("generate-lockfile").run();
    let lockfile = p.read_lockfile();
    assert!(lockfile.contains("\nname = \"my-dependency\"\nversion = \"0.1.2\""));

    // Test upgrade
    // 0.1.3 is in the range, so it match
    cargo_test_support::registry::Package::new("my-dependency", "0.1.3").publish();
    p.cargo("update -p my-dependency --precise 0.1.3 -Zunstable-options")
        .masquerade_as_nightly_cargo(&["precise-pre-release"])
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[UPDATING] my-dependency v0.1.2 -> v0.1.3

"#]])
        .run();

    let lockfile = p.read_lockfile();
    assert!(lockfile.contains("\nname = \"my-dependency\"\nversion = \"0.1.3\""));

    // Test downgrade
    // v0.1.3-pre.1 is in the range, so it match
    cargo_test_support::registry::Package::new("my-dependency", "0.1.3-pre.1").publish();
    p.cargo("update -p my-dependency --precise 0.1.3-pre.1 -Zunstable-options")
        .masquerade_as_nightly_cargo(&["precise-pre-release"])
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[DOWNGRADING] my-dependency v0.1.3 -> v0.1.3-pre.1

"#]])
        .run();

    let lockfile = p.read_lockfile();
    assert!(lockfile.contains("\nname = \"my-dependency\"\nversion = \"0.1.3-pre.1\""));
}
