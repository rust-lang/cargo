//! Tests for unstable `patch-files` feature.

use cargo_test_support::registry::Package;
use cargo_test_support::project;
use cargo_test_support::str;

#[cargo_test]
fn gated_manifest() {
    Package::new("bar", "1.0.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                edition = "2015"

                [dependencies]
                bar = "1"

                [patch.crates-io]
                bar = { version = "=1.0.0", patches = [] }
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[WARNING] ignoring `patches` on patch for `bar` in `https://github.com/rust-lang/crates.io-index`; see https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#patch-files about the status of this feature.
[UPDATING] `dummy-registry` index
[ERROR] failed to resolve patches for `https://github.com/rust-lang/crates.io-index`

Caused by:
  patch for `bar` in `https://github.com/rust-lang/crates.io-index` points to the same source, but patches must point to different sources

"#]])
        .run();
}

#[cargo_test]
fn gated_config() {
    Package::new("bar", "1.0.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                edition = "2015"

                [dependencies]
                bar = "1"

                [patch.crates-io]
                bar = { version = "=1.0.0", patches = [] }
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            ".cargo/config.toml",
            r#"
                [patch.crates-io]
                bar = { version = "=1.0.0", patches = [] }
            "#,
        )
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[WARNING] ignoring `patches` on patch for `bar` in `https://github.com/rust-lang/crates.io-index`; see https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#patch-files about the status of this feature.
[WARNING] [patch] in cargo config: ignoring `patches` on patch for `bar` in `https://github.com/rust-lang/crates.io-index`; see https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#patch-files about the status of this feature.
[UPDATING] `dummy-registry` index
[ERROR] failed to resolve patches for `https://github.com/rust-lang/crates.io-index`

Caused by:
  patch for `bar` in `https://github.com/rust-lang/crates.io-index` points to the same source, but patches must point to different sources

"#]])
        .run();
}

#[cargo_test]
fn warn_if_in_normal_dep() {
    Package::new("bar", "1.0.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                edition = "2015"

                [dependencies]
                bar = { version = "1", patches = [] }
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check")
        .with_stderr_data(str![[r#"
[WARNING] unused manifest key: dependencies.bar.patches; see https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#patch-files about the status of this feature.
[UPDATING] `dummy-registry` index
[LOCKING] 2 packages to latest compatible versions
[DOWNLOADING] crates ...
[DOWNLOADED] bar v1.0.0 (registry `dummy-registry`)
[CHECKING] bar v1.0.0
[CHECKING] foo v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}
