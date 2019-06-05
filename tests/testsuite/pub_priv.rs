use crate::support::registry::Package;
use crate::support::{is_nightly, project};

#[cargo_test]
fn exported_priv_warning() {
    if !is_nightly() {
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
        .with_stderr(
            "\
[UPDATING] `[..]` index
[DOWNLOADING] crates ...
[DOWNLOADED] priv_dep v0.1.0 ([..])
[COMPILING] priv_dep v0.1.0
[COMPILING] foo v0.0.1 ([CWD])
src/lib.rs:3:13: warning: type `priv_dep::FromPriv` from private dependency 'priv_dep' in public interface
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
"
        )
        .run()
}

#[cargo_test]
fn exported_pub_dep() {
    if !is_nightly() {
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

consider adding `cargo-features = [\"public-dependency\"]` to the manifest
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
