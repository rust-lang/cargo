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
[HELP] if you are looking for the prerelease package it needs to be specified explicitly
    my-dependency = { version = "0.1.2-pre.0" }
[NOTE] perhaps a crate was updated and forgotten to be re-vendored?

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

    p.cargo("update my-dependency --precise 0.1.2-pre.0")
        .arg("-Zprerelease")
        .masquerade_as_nightly_cargo(&["prerelease"])
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
    p.cargo("update -p my-dependency --precise 0.1.2-pre.0")
        .arg("-Zprerelease")
        .masquerade_as_nightly_cargo(&["prerelease"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[ERROR] failed to select a version for the requirement `my-dependency = "^0.1.2"`
candidate versions found which didn't match: 0.1.2-pre.0
location searched: `dummy-registry` index (which is replacing registry `crates-io`)
required by package `package v0.0.0 ([ROOT]/foo)`
[HELP] if you are looking for the prerelease package it needs to be specified explicitly
    my-dependency = { version = "0.1.2-pre.0" }
[NOTE] perhaps a crate was updated and forgotten to be re-vendored?

"#]])
        .run();

    cargo_test_support::registry::Package::new("my-dependency", "0.2.0-0").publish();
    // 0.2.0-0 is the upper bound we exclude, so it doesn't match
    p.cargo("update -p my-dependency --precise 0.2.0-0")
        .arg("-Zprerelease")
        .masquerade_as_nightly_cargo(&["prerelease"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[ERROR] failed to select a version for the requirement `my-dependency = "^0.1.2"`
candidate versions found which didn't match: 0.2.0-0
location searched: `dummy-registry` index (which is replacing registry `crates-io`)
required by package `package v0.0.0 ([ROOT]/foo)`
[HELP] if you are looking for the prerelease package it needs to be specified explicitly
    my-dependency = { version = "0.2.0-0" }
[NOTE] perhaps a crate was updated and forgotten to be re-vendored?

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
    p.cargo("update -p my-dependency --precise 0.1.3")
        .arg("-Zprerelease")
        .masquerade_as_nightly_cargo(&["prerelease"])
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
    p.cargo("update -p my-dependency --precise 0.1.3-pre.1")
        .arg("-Zprerelease")
        .masquerade_as_nightly_cargo(&["prerelease"])
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[DOWNGRADING] my-dependency v0.1.3 -> v0.1.3-pre.1

"#]])
        .run();

    let lockfile = p.read_lockfile();
    assert!(lockfile.contains("\nname = \"my-dependency\"\nversion = \"0.1.3-pre.1\""));
}

#[cargo_test]
fn pin_prereleases_in_separate_updates() {
    cargo_test_support::registry::init();

    for name in ["first-dep", "second-dep"] {
        for version in ["0.1.1", "0.1.2-pre.0"] {
            cargo_test_support::registry::Package::new(name, version).publish();
        }
    }

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "package"
            edition = "2018"
            [dependencies]
            first-dep = "0.1.1"
            second-dep = "0.1.1"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("generate-lockfile").run();

    p.cargo("update first-dep --precise 0.1.2-pre.0")
        .arg("-Zprerelease")
        .masquerade_as_nightly_cargo(&["prerelease"])
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[UPDATING] first-dep v0.1.1 -> v0.1.2-pre.0

"#]])
        .run();

    p.cargo("update second-dep --precise 0.1.2-pre.0")
        .arg("-Zprerelease")
        .masquerade_as_nightly_cargo(&["prerelease"])
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[UPDATING] second-dep v0.1.1 -> v0.1.2-pre.0

"#]])
        .run();

    let lockfile = p.read_lockfile();
    assert!(lockfile.contains("\nname = \"first-dep\"\nversion = \"0.1.2-pre.0\""));
    assert!(lockfile.contains("\nname = \"second-dep\"\nversion = \"0.1.2-pre.0\""));
}

/// Baseline to make sure that pre-release won't be selected if not explicitlyed pinned.
#[cargo_test]
fn dont_select_prerelease_if_not_explicitly_pinned() {
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
            edition = "2018"
            [dependencies]
            my-dependency = "0.1.1"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("generate-lockfile")
        .arg("-Zprerelease")
        .masquerade_as_nightly_cargo(&["prerelease"])
        .run();

    let lockfile = p.read_lockfile();
    assert!(lockfile.contains("\nname = \"my-dependency\"\nversion = \"0.1.1\""));
}

#[cargo_test]
fn pin_prerelease_and_check() {
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
            edition = "2018"
            [dependencies]
            my-dependency = "0.1.1"
            "#,
        )
        .file("src/lib.rs", "use my_dependency as _;")
        .build();

    p.cargo("update my-dependency --precise 0.1.2-pre.0")
        .arg("-Zprerelease")
        .masquerade_as_nightly_cargo(&["prerelease"])
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[UPDATING] my-dependency v0.1.1 -> v0.1.2-pre.0

"#]])
        .run();

    p.cargo("check --locked")
        .arg("-Zprerelease")
        .masquerade_as_nightly_cargo(&["prerelease"])
        .with_stderr_data(str![[r#"
[DOWNLOADING] crates ...
[DOWNLOADED] my-dependency v0.1.2-pre.0 (registry `dummy-registry`)
[CHECKING] my-dependency v0.1.2-pre.0
[CHECKING] package v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("check")
        .arg("-Zprerelease")
        .masquerade_as_nightly_cargo(&["prerelease"])
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    let lockfile = p.read_lockfile();
    assert!(lockfile.contains("\nname = \"my-dependency\"\nversion = \"0.1.2-pre.0\""));
}

/// Like [`pin_prerelease_and_check`] but without `-Zprerelease`.
#[cargo_test]
fn pin_prerelease_and_check_without_zflag() {
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
            edition = "2018"
            [dependencies]
            my-dependency = "0.1.1"
            "#,
        )
        .file("src/lib.rs", "use my_dependency as _;")
        .build();

    p.cargo("update my-dependency --precise 0.1.2-pre.0")
        .arg("-Zprerelease")
        .masquerade_as_nightly_cargo(&["prerelease"])
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[UPDATING] my-dependency v0.1.1 -> v0.1.2-pre.0

"#]])
        .run();

    p.cargo("check --locked")
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[ERROR] cannot update the lock file [ROOT]/foo/Cargo.lock because --locked was passed to prevent this
[HELP] to generate the lock file without accessing the network, remove the --locked flag and use --offline instead.

"#]])
        .run();

    p.cargo("check")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to highest compatible version
[DOWNGRADING] my-dependency v0.1.2-pre.0 -> v0.1.1
[DOWNLOADING] crates ...
[DOWNLOADED] my-dependency v0.1.1 (registry `dummy-registry`)
[CHECKING] my-dependency v0.1.1
[CHECKING] package v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    let lockfile = p.read_lockfile();
    assert!(lockfile.contains("\nname = \"my-dependency\"\nversion = \"0.1.1\""));
}

/// Like [`pin_prerelease_and_check`]
/// but the pinned package is required transitively.
#[cargo_test]
fn pin_prerelease_for_transitive_dep_and_check() {
    cargo_test_support::registry::init();

    for version in ["0.1.1", "0.1.2-pre.0"] {
        cargo_test_support::registry::Package::new("my-dependency", version).publish();
    }
    cargo_test_support::registry::Package::new("bar", "1.0.0")
        .dep("my-dependency", "0.1.1")
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "package"
            edition = "2018"
            [dependencies]
            bar = "1.0.0"
            "#,
        )
        .file("src/lib.rs", "use bar as _;")
        .build();

    p.cargo("generate-lockfile").run();

    p.cargo("update my-dependency --precise 0.1.2-pre.0")
        .arg("-Zprerelease")
        .masquerade_as_nightly_cargo(&["prerelease"])
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[UPDATING] my-dependency v0.1.1 -> v0.1.2-pre.0

"#]])
        .run();
    let lockfile = p.read_lockfile();
    assert!(lockfile.contains("\nname = \"my-dependency\"\nversion = \"0.1.2-pre.0\""));

    p.cargo("check")
        .arg("-Zprerelease")
        .masquerade_as_nightly_cargo(&["prerelease"])
        .with_stderr_data(str![[r#"
[DOWNLOADING] crates ...
[DOWNLOADED] bar v1.0.0 (registry `dummy-registry`)
[DOWNLOADED] my-dependency v0.1.2-pre.0 (registry `dummy-registry`)
[CHECKING] my-dependency v0.1.2-pre.0
[CHECKING] bar v1.0.0
[CHECKING] package v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    let lockfile = p.read_lockfile();
    assert!(lockfile.contains("\nname = \"my-dependency\"\nversion = \"0.1.2-pre.0\""));
}

/// Like [`pin_prerelease_and_check`]
/// but the pinned package is required both directly and transitively.
#[cargo_test]
fn pin_prerelease_for_shared_direct_transitive_dep_and_check() {
    cargo_test_support::registry::init();

    for version in ["0.1.1", "0.1.2-pre.0"] {
        cargo_test_support::registry::Package::new("my-dependency", version).publish();
    }
    cargo_test_support::registry::Package::new("bar", "1.0.0")
        .dep("my-dependency", "0.1.1")
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "package"
            edition = "2018"
            [dependencies]
            bar = "1.0.0"
            "#,
        )
        .file("src/lib.rs", "use bar as _;")
        .build();

    p.cargo("generate-lockfile").run();

    p.change_file(
        "Cargo.toml",
        r#"
        [package]
        name = "package"
        edition = "2018"
        [dependencies]
        bar = "1.0.0"
        my-dependency = "0.1.2-pre.0"
        "#,
    );

    p.cargo("update my-dependency --precise 0.1.2-pre.0")
        .arg("-Zprerelease")
        .masquerade_as_nightly_cargo(&["prerelease"])
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[UPDATING] my-dependency v0.1.1 -> v0.1.2-pre.0

"#]])
        .run();

    let lockfile = p.read_lockfile();
    assert!(lockfile.contains("\nname = \"my-dependency\"\nversion = \"0.1.2-pre.0\""));

    p.cargo("check")
        .arg("-Zprerelease")
        .masquerade_as_nightly_cargo(&["prerelease"])
        .with_stderr_data(str![[r#"
[DOWNLOADING] crates ...
[DOWNLOADED] bar v1.0.0 (registry `dummy-registry`)
[DOWNLOADED] my-dependency v0.1.2-pre.0 (registry `dummy-registry`)
[CHECKING] my-dependency v0.1.2-pre.0
[CHECKING] bar v1.0.0
[CHECKING] package v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    let lockfile = p.read_lockfile();
    assert!(lockfile.contains("\nname = \"my-dependency\"\nversion = \"0.1.2-pre.0\""));
}

/// Like [`pin_prerelease_for_shared_direct_transitive_dep_and_check`] but without `-Zprerelease`.
#[cargo_test]
fn pin_prerelease_for_shared_direct_transitive_dep_and_check_without_zflag() {
    cargo_test_support::registry::init();

    for version in ["0.1.1", "0.1.2-pre.0"] {
        cargo_test_support::registry::Package::new("my-dependency", version).publish();
    }
    cargo_test_support::registry::Package::new("bar", "1.0.0")
        .dep("my-dependency", "0.1.1")
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "package"
            edition = "2018"
            [dependencies]
            bar = "1.0.0"
            "#,
        )
        .file("src/lib.rs", "use bar as _;")
        .build();

    p.cargo("generate-lockfile").run();

    p.change_file(
        "Cargo.toml",
        r#"
        [package]
        name = "package"
        edition = "2018"
        [dependencies]
        bar = "1.0.0"
        my-dependency = "0.1.2-pre.0"
        "#,
    );

    p.cargo("update my-dependency --precise 0.1.2-pre.0")
        .arg("-Zprerelease")
        .masquerade_as_nightly_cargo(&["prerelease"])
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[UPDATING] my-dependency v0.1.1 -> v0.1.2-pre.0

"#]])
        .run();

    let lockfile = p.read_lockfile();
    assert!(lockfile.contains("\nname = \"my-dependency\"\nversion = \"0.1.2-pre.0\""));

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to select a version for `my-dependency`.
    ... required by package `bar v1.0.0`
    ... which satisfies dependency `bar = "^1.0.0"` (locked to 1.0.0) of package `package v0.0.0 ([ROOT]/foo)`
versions that meet the requirements `^0.1.1` are: 0.1.1

all possible versions conflict with previously selected packages

  previously selected package `my-dependency v0.1.2-pre.0`
    ... which satisfies dependency `my-dependency = "^0.1.2-pre.0"` (locked to 0.1.2-pre.0) of package `package v0.0.0 ([ROOT]/foo)`

failed to select a version for `my-dependency` which could resolve this conflict

"#]])
        .run();

    let lockfile = p.read_lockfile();
    assert!(lockfile.contains("\nname = \"my-dependency\"\nversion = \"0.1.2-pre.0\""));
}
