//! Tests for public/private dependencies.

use cargo_test_support::registry::Package;
use cargo_test_support::{is_nightly, project};

#[cargo_test]
fn exported_priv_warning() {
    if !is_nightly() {
        // exported_private_dependencies lint is unstable
        return;
    }
    Package::new("priv_dep", "0.1.0")
        .file("src/lib.rs", "pub struct FromPriv;")
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["public-dependency"]

                [package]
                name = "foo"
                version = "0.0.1"

                [dependencies]
                priv_dep = "0.1.0"
            "#,
        )
        .file(
            "src/lib.rs",
            "
            extern crate priv_dep;
            pub fn use_priv(_: priv_dep::FromPriv) {}
        ",
        )
        .build();

    p.cargo("build --message-format=short")
        .masquerade_as_nightly_cargo()
        .with_stderr_contains(
            "\
src/lib.rs:3:13: warning: type `[..]FromPriv` from private dependency 'priv_dep' in public interface
",
        )
        .run()
}

#[cargo_test]
fn exported_pub_dep() {
    if !is_nightly() {
        // exported_private_dependencies lint is unstable
        return;
    }
    Package::new("pub_dep", "0.1.0")
        .file("src/lib.rs", "pub struct FromPub;")
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["public-dependency"]

                [package]
                name = "foo"
                version = "0.0.1"

                [dependencies]
                pub_dep = {version = "0.1.0", public = true}
            "#,
        )
        .file(
            "src/lib.rs",
            "
            extern crate pub_dep;
            pub fn use_pub(_: pub_dep::FromPub) {}
        ",
        )
        .build();

    p.cargo("build --message-format=short")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[UPDATING] `[..]` index
[DOWNLOADING] crates ...
[DOWNLOADED] pub_dep v0.1.0 ([..])
[COMPILING] pub_dep v0.1.0
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run()
}

#[cargo_test]
pub fn requires_nightly_cargo() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["public-dependency"]
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build --message-format=short")
        .with_status(101)
        .with_stderr(
            "\
error: failed to parse manifest at `[..]`

Caused by:
  the cargo feature `public-dependency` requires a nightly version of Cargo, but this is the `stable` channel
  See https://doc.rust-lang.org/book/appendix-07-nightly-rust.html for more information about Rust release channels.
  See https://doc.rust-lang.org/[..]cargo/reference/unstable.html#public-dependency for more information about using this feature.
"
        )
        .run()
}

#[cargo_test]
fn requires_feature() {
    Package::new("pub_dep", "0.1.0")
        .file("src/lib.rs", "")
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"

                [dependencies]
                pub_dep = { version = "0.1.0", public = true }
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build --message-format=short")
        .masquerade_as_nightly_cargo()
        .with_status(101)
        .with_stderr(
            "\
error: failed to parse manifest at `[..]`

Caused by:
  feature `public-dependency` is required

  The package requires the Cargo feature called `public-dependency`, \
  but that feature is not stabilized in this version of Cargo (1.[..]).
  Consider adding `cargo-features = [\"public-dependency\"]` to the top of Cargo.toml \
  (above the [package] table) to tell Cargo you are opting in to use this unstable feature.
  See https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#public-dependency \
  for more information about the status of this feature.
",
        )
        .run()
}

#[cargo_test]
fn pub_dev_dependency() {
    Package::new("pub_dep", "0.1.0")
        .file("src/lib.rs", "pub struct FromPub;")
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["public-dependency"]

                [package]
                name = "foo"
                version = "0.0.1"

                [dev-dependencies]
                pub_dep = {version = "0.1.0", public = true}
            "#,
        )
        .file(
            "src/lib.rs",
            "
            extern crate pub_dep;
            pub fn use_pub(_: pub_dep::FromPub) {}
        ",
        )
        .build();

    p.cargo("build --message-format=short")
        .masquerade_as_nightly_cargo()
        .with_status(101)
        .with_stderr(
            "\
error: failed to parse manifest at `[..]`

Caused by:
  'public' specifier can only be used on regular dependencies, not Development dependencies
",
        )
        .run()
}
