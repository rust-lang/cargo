//! Tests for `cargo-features` definitions.

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
            authors = []
            im-a-teapot = true
        "#,
        )
        .file("src/lib.rs", "")
        .build();
    p.cargo("build")
        .masquerade_as_nightly_cargo()
        .with_status(101)
        .with_stderr(
            "\
error: failed to parse manifest at `[..]`

Caused by:
  the `im-a-teapot` manifest key is unstable and may not work properly in England

Caused by:
  feature `test-dummy-unstable` is required

  consider adding `cargo-features = [\"test-dummy-unstable\"]` to the manifest
",
        )
        .run();

    p.cargo("build")
        .with_status(101)
        .with_stderr(
            "\
error: failed to parse manifest at `[..]`

Caused by:
  the `im-a-teapot` manifest key is unstable and may not work properly in England

Caused by:
  feature `test-dummy-unstable` is required

  this Cargo does not support nightly features, but if you
  switch to nightly channel you can add
  `cargo-features = [\"test-dummy-unstable\"]` to enable this feature
",
        )
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
            authors = []
        "#,
        )
        .file("src/lib.rs", "")
        .build();
    p.cargo("build")
        .with_status(101)
        .with_stderr(
            "\
error: failed to parse manifest at `[..]`

Caused by:
  unknown cargo feature `foo`
",
        )
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
            authors = []
        "#,
        )
        .file("src/lib.rs", "")
        .build();
    p.cargo("build")
        .with_stderr(
            "\
warning: the cargo feature `test-dummy-stable` is now stable and is no longer \
necessary to be listed in the manifest
[COMPILING] a [..]
[FINISHED] [..]
",
        )
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
            authors = []
            im-a-teapot = true
        "#,
        )
        .file("src/lib.rs", "")
        .build();
    p.cargo("build")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[COMPILING] a [..]
[FINISHED] [..]
",
        )
        .run();

    p.cargo("build")
        .with_status(101)
        .with_stderr(
            "\
error: failed to parse manifest at `[..]`

Caused by:
  the cargo feature `test-dummy-unstable` requires a nightly version of Cargo, \
  but this is the `stable` channel
  See [..]
",
        )
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
            authors = []
            im-a-teapot = true
        "#,
        )
        .file("a/src/lib.rs", "")
        .build();
    p.cargo("build")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[COMPILING] a [..]
[COMPILING] b [..]
[FINISHED] [..]
",
        )
        .run();

    p.cargo("build")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] failed to get `a` as a dependency of package `b v0.0.1 ([..])`

Caused by:
  failed to load source for dependency `a`

Caused by:
  Unable to update [..]

Caused by:
  failed to parse manifest at `[..]`

Caused by:
  the cargo feature `test-dummy-unstable` requires a nightly version of Cargo, \
  but this is the `stable` channel
  See [..]
",
        )
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
            authors = []
            im-a-teapot = true
        "#,
        )
        .file("src/lib.rs", "")
        .build();
    p.cargo("build")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[COMPILING] a [..]
[FINISHED] [..]
",
        )
        .run();

    p.cargo("build")
        .with_status(101)
        .with_stderr(
            "\
error: failed to parse manifest at `[..]`

Caused by:
  the cargo feature `test-dummy-unstable` requires a nightly version of Cargo, \
  but this is the `stable` channel
  See [..]
",
        )
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
            authors = []
            im-a-teapot = true
        "#,
        )
        .file("src/lib.rs", "")
        .build();
    p.cargo("build -Zprint-im-a-teapot")
        .with_status(101)
        .with_stderr(
            "error: the `-Z` flag is only accepted on the nightly \
             channel of Cargo, but this is the `stable` channel\n\
             See [..]",
        )
        .run();

    p.cargo("build -Zarg")
        .masquerade_as_nightly_cargo()
        .with_status(101)
        .with_stderr("error: unknown `-Z` flag specified: arg")
        .run();

    p.cargo("build -Zprint-im-a-teapot")
        .masquerade_as_nightly_cargo()
        .with_stdout("im-a-teapot = true\n")
        .with_stderr(
            "\
[COMPILING] a [..]
[FINISHED] [..]
",
        )
        .run();
}

#[cargo_test]
fn publish_allowed() {
    registry::init();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            cargo-features = ["test-dummy-unstable"]

            [package]
            name = "a"
            version = "0.0.1"
            authors = []
        "#,
        )
        .file("src/lib.rs", "")
        .build();
    p.cargo("publish --token sekrit")
        .masquerade_as_nightly_cargo()
        .run();
}
