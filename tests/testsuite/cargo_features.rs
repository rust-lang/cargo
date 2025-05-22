//! Tests for `cargo-features` definitions.

use cargo_test_support::prelude::*;
use cargo_test_support::registry::Package;
use cargo_test_support::str;
use cargo_test_support::{project, registry};

#[cargo_test]
fn feature_required() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "a"
                version = "0.0.1"
                edition = "2015"
                authors = []
                im-a-teapot = true
            "#,
        )
        .file("src/lib.rs", "")
        .build();
    p.cargo("check")
        .masquerade_as_nightly_cargo(&["test-dummy-unstable"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  the `im-a-teapot` manifest key is unstable and may not work properly in England

Caused by:
  feature `test-dummy-unstable` is required

  The package requires the Cargo feature called `test-dummy-unstable`, but that feature is not stabilized in this version of Cargo (1.[..]).
  Consider adding `cargo-features = ["test-dummy-unstable"]` to the top of Cargo.toml (above the [package] table) to tell Cargo you are opting in to use this unstable feature.
  See https://doc.rust-lang.org/nightly/cargo/reference/unstable.html for more information about the status of this feature.

"#]])
        .run();

    // Same, but stable.
    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  the `im-a-teapot` manifest key is unstable and may not work properly in England

Caused by:
  feature `test-dummy-unstable` is required

  The package requires the Cargo feature called `test-dummy-unstable`, but that feature is not stabilized in this version of Cargo (1.[..]).
  Consider trying a newer version of Cargo (this may require the nightly release).
  See https://doc.rust-lang.org/nightly/cargo/reference/unstable.html for more information about the status of this feature.

"#]])
        .run();
}

#[cargo_test]
fn feature_required_dependency() {
    // The feature has been stabilized by a future version of Cargo, and
    // someone published something uses it, but this version of Cargo has not
    // yet stabilized it. Don't suggest editing Cargo.toml, since published
    // packages shouldn't be edited.
    Package::new("bar", "1.0.0")
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.1.0"
                edition = "2015"
                im-a-teapot = true
            "#,
        )
        .file("src/lib.rs", "")
        .publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

                [dependencies]
                bar = "1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check")
        .masquerade_as_nightly_cargo(&["test-dummy-unstable"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] bar v1.0.0 (registry `dummy-registry`)
[ERROR] failed to download replaced source registry `crates-io`

Caused by:
  failed to parse manifest at `[ROOT]/home/.cargo/registry/src/-[HASH]/bar-1.0.0/Cargo.toml`

Caused by:
  the `im-a-teapot` manifest key is unstable and may not work properly in England

Caused by:
  feature `test-dummy-unstable` is required

  The package requires the Cargo feature called `test-dummy-unstable`, but that feature is not stabilized in this version of Cargo (1.[..]).
  Consider trying a more recent nightly release.
  See https://doc.rust-lang.org/nightly/cargo/reference/unstable.html for more information about the status of this feature.

"#]])
        .run();

    // Same, but stable.
    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to download `bar v1.0.0`

Caused by:
  unable to get packages from source

Caused by:
  failed to download replaced source registry `crates-io`

Caused by:
  failed to parse manifest at `[ROOT]/home/.cargo/registry/src/-[HASH]/bar-1.0.0/Cargo.toml`

Caused by:
  the `im-a-teapot` manifest key is unstable and may not work properly in England

Caused by:
  feature `test-dummy-unstable` is required

  The package requires the Cargo feature called `test-dummy-unstable`, but that feature is not stabilized in this version of Cargo (1.[..]).
  Consider trying a newer version of Cargo (this may require the nightly release).
  See https://doc.rust-lang.org/nightly/cargo/reference/unstable.html for more information about the status of this feature.

"#]])
        .run();
}

#[cargo_test]
fn unknown_feature() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["foo"]

                [package]
                name = "a"
                version = "0.0.1"
                edition = "2015"
                authors = []
            "#,
        )
        .file("src/lib.rs", "")
        .build();
    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  unknown cargo feature `foo`

"#]])
        .run();
}

#[cargo_test]
fn stable_feature_warns() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["test-dummy-stable"]

                [package]
                name = "a"
                version = "0.0.1"
                edition = "2015"
                authors = []
            "#,
        )
        .file("src/lib.rs", "")
        .build();
    p.cargo("check")
        .with_stderr_data(str![[r#"
[WARNING] the cargo feature `test-dummy-stable` has been stabilized in the 1.0 release and is no longer necessary to be listed in the manifest
  See https://doc.rust-lang.org/cargo/ for more information about using this feature.
[CHECKING] a v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test(nightly, reason = "-Zallow-features is unstable")]
fn allow_features() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["test-dummy-unstable"]

                [package]
                name = "a"
                version = "0.0.1"
                edition = "2015"
                authors = []
                im-a-teapot = true
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("-Zallow-features=test-dummy-unstable check")
        .masquerade_as_nightly_cargo(&["allow-features", "test-dummy-unstable"])
        .with_stderr_data(str![[r#"
[CHECKING] a v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("-Zallow-features=test-dummy-unstable,print-im-a-teapot -Zprint-im-a-teapot check")
        .masquerade_as_nightly_cargo(&[
            "allow-features",
            "test-dummy-unstable",
            "print-im-a-teapot",
        ])
        .with_stdout_data(str![[r#"
im-a-teapot = true

"#]])
        .run();

    p.cargo("-Zallow-features=test-dummy-unstable -Zprint-im-a-teapot check")
        .masquerade_as_nightly_cargo(&[
            "allow-features",
            "test-dummy-unstable",
            "print-im-a-teapot",
        ])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] the feature `print-im-a-teapot` is not in the list of allowed features: [test-dummy-unstable]

"#]])
        .run();

    p.cargo("-Zallow-features= check")
        .masquerade_as_nightly_cargo(&["allow-features", "test-dummy-unstable"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  the feature `test-dummy-unstable` is not in the list of allowed features: []

"#]])
        .run();
}

#[cargo_test(nightly, reason = "-Zallow-features is unstable")]
fn allow_features_to_rustc() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "a"
                version = "0.0.1"
                edition = "2015"
                authors = []
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
                #![allow(internal_features)]
                #![feature(rustc_attrs)]
            "#,
        )
        .build();

    p.cargo("-Zallow-features= check")
        .masquerade_as_nightly_cargo(&["allow-features"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[CHECKING] a v0.0.1 ([ROOT]/foo)
error[E0725]: the feature `rustc_attrs` is not in the list of allowed features
...
"#]])
        .run();

    p.cargo("-Zallow-features=rustc_attrs check")
        .masquerade_as_nightly_cargo(&["allow-features"])
        .with_stderr_data(str![[r#"
[CHECKING] a v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test(nightly, reason = "-Zallow-features is unstable")]
fn allow_features_in_cfg() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["test-dummy-unstable"]

                [package]
                name = "a"
                version = "0.0.1"
                edition = "2015"
                authors = []
                im-a-teapot = true
            "#,
        )
        .file(
            ".cargo/config.toml",
            r#"
                [unstable]
                allow-features = ["test-dummy-unstable", "print-im-a-teapot"]
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check")
        .masquerade_as_nightly_cargo(&[
            "allow-features",
            "test-dummy-unstable",
            "print-im-a-teapot",
        ])
        .with_stderr_data(str![[r#"
[CHECKING] a v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("-Zprint-im-a-teapot check")
        .masquerade_as_nightly_cargo(&[
            "allow-features",
            "test-dummy-unstable",
            "print-im-a-teapot",
        ])
        .with_stdout_data(str![[r#"
im-a-teapot = true

"#]])
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("-Zunstable-options check")
        .masquerade_as_nightly_cargo(&["allow-features", "test-dummy-unstable", "print-im-a-teapot"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] the feature `unstable-options` is not in the list of allowed features: [print-im-a-teapot, test-dummy-unstable]

"#]])
        .run();

    // -Zallow-features overrides .cargo/config.toml
    p.cargo("-Zallow-features=test-dummy-unstable -Zprint-im-a-teapot check")
        .masquerade_as_nightly_cargo(&[
            "allow-features",
            "test-dummy-unstable",
            "print-im-a-teapot",
        ])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] the feature `print-im-a-teapot` is not in the list of allowed features: [test-dummy-unstable]

"#]])
        .run();

    p.cargo("-Zallow-features= check")
        .masquerade_as_nightly_cargo(&[
            "allow-features",
            "test-dummy-unstable",
            "print-im-a-teapot",
        ])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  the feature `test-dummy-unstable` is not in the list of allowed features: []

"#]])
        .run();
}

#[cargo_test]
fn nightly_feature_requires_nightly() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["test-dummy-unstable"]

                [package]
                name = "a"
                version = "0.0.1"
                edition = "2015"
                authors = []
                im-a-teapot = true
            "#,
        )
        .file("src/lib.rs", "")
        .build();
    p.cargo("check")
        .masquerade_as_nightly_cargo(&["test-dummy-unstable"])
        .with_stderr_data(str![[r#"
[CHECKING] a v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  the cargo feature `test-dummy-unstable` requires a nightly version of Cargo, but this is the `stable` channel
...
"#]])
        .run();
}

#[cargo_test]
fn nightly_feature_requires_nightly_in_dep() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "b"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies]
                a = { path = "a" }
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "a/Cargo.toml",
            r#"
                cargo-features = ["test-dummy-unstable"]

                [package]
                name = "a"
                version = "0.0.1"
                edition = "2015"
                authors = []
                im-a-teapot = true
            "#,
        )
        .file("a/src/lib.rs", "")
        .build();
    p.cargo("check")
        .masquerade_as_nightly_cargo(&["test-dummy-unstable"])
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to latest compatible version
[CHECKING] a v0.0.1 ([ROOT]/foo/a)
[CHECKING] b v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to get `a` as a dependency of package `b v0.0.1 ([ROOT]/foo)`

Caused by:
  failed to load source for dependency `a`

Caused by:
  Unable to update [ROOT]/foo/a

Caused by:
  failed to parse manifest at `[ROOT]/foo/a/Cargo.toml`

Caused by:
  the cargo feature `test-dummy-unstable` requires a nightly version of Cargo, but this is the `stable` channel
...
"#]])
        .run();
}

#[cargo_test]
fn cant_publish() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["test-dummy-unstable"]

                [package]
                name = "a"
                version = "0.0.1"
                edition = "2015"
                authors = []
                im-a-teapot = true
            "#,
        )
        .file("src/lib.rs", "")
        .build();
    p.cargo("check")
        .masquerade_as_nightly_cargo(&["test-dummy-unstable"])
        .with_stderr_data(str![[r#"
[CHECKING] a v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  the cargo feature `test-dummy-unstable` requires a nightly version of Cargo, but this is the `stable` channel
...

"#]])
        .run();
}

#[cargo_test]
fn z_flags_rejected() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["test-dummy-unstable"]

                [package]
                name = "a"
                version = "0.0.1"
                edition = "2015"
                authors = []
                im-a-teapot = true
            "#,
        )
        .file("src/lib.rs", "")
        .build();
    p.cargo("check -Zprint-im-a-teapot")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] the `-Z` flag is only accepted on the nightly channel of Cargo, but this is the `stable` channel
See [..]

"#]])
        .run();

    p.cargo("check -Zarg")
        .masquerade_as_nightly_cargo(&["test-dummy-unstable"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] unknown `-Z` flag specified: arg

For available unstable features, see https://doc.rust-lang.org/nightly/cargo/reference/unstable.html
If you intended to use an unstable rustc feature, try setting `RUSTFLAGS="-Zarg"`

"#]])
        .run();

    p.cargo("check -Zprint-im-a-teapot")
        .masquerade_as_nightly_cargo(&["test-dummy-unstable"])
        .with_stdout_data(str![[r#"
im-a-teapot = true

"#]])
        .with_stderr_data(str![[r#"
[CHECKING] a v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn publish_allowed() {
    let registry = registry::RegistryBuilder::new()
        .http_api()
        .http_index()
        .build();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["test-dummy-unstable"]

                [package]
                name = "a"
                version = "0.0.1"
                edition = "2015"
                authors = []
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("publish")
        .replace_crates_io(registry.index_url())
        .masquerade_as_nightly_cargo(&["test-dummy-unstable"])
        .with_stderr_data(str![[r#"
[UPDATING] crates.io index
[WARNING] manifest has no description, license, license-file, documentation, homepage or repository.
See https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info.
[PACKAGING] a v0.0.1 ([ROOT]/foo)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] a v0.0.1 ([ROOT]/foo)
[COMPILING] a v0.0.1 ([ROOT]/foo/target/package/a-0.0.1)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[UPLOADING] a v0.0.1 ([ROOT]/foo)
[UPLOADED] a v0.0.1 to registry `crates-io`
[NOTE] waiting for `a v0.0.1` to be available at registry `crates-io`.
You may press ctrl-c to skip waiting; the crate should be available shortly.
[PUBLISHED] a v0.0.1 at registry `crates-io`

"#]])
        .run();
}

#[cargo_test]
fn wrong_position() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"
                cargo-features = ["test-dummy-unstable"]
            "#,
        )
        .file("src/lib.rs", "")
        .build();
    p.cargo("check")
        .masquerade_as_nightly_cargo(&["test-dummy-unstable"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] the field `cargo-features` should be set at the top of Cargo.toml before any tables
 --> Cargo.toml:6:34
  |
6 |                 cargo-features = ["test-dummy-unstable"]
  |                                  ^^^^^^^^^^^^^^^^^^^^^^^
  |

"#]])
        .run();
}

#[cargo_test]
fn z_stabilized() {
    let p = project().file("src/lib.rs", "").build();

    p.cargo("check -Z cache-messages")
        .masquerade_as_nightly_cargo(&["always_nightly"])
        .with_stderr_data(str![[r#"
[WARNING] flag `-Z cache-messages` has been stabilized in the 1.40 release, and is no longer necessary
  Message caching is now always enabled.

[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("check -Z offline")
        .masquerade_as_nightly_cargo(&["always_nightly"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] flag `-Z offline` has been stabilized in the 1.36 release
  Offline mode is now available via the --offline CLI option


"#]])
        .run();
}
