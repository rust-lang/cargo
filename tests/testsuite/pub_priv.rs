//! Tests for public/private dependencies.

use crate::prelude::*;
use cargo_test_support::registry::{Dependency, Package};
use cargo_test_support::{git, str};
use cargo_test_support::{project, registry};

#[cargo_test(nightly, reason = "exported_private_dependencies lint is unstable")]
fn exported_priv_warning() {
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
                edition = "2015"

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

    p.cargo("check --message-format=short")
        .masquerade_as_nightly_cargo(&["public-dependency"])
        .with_stderr_data(str![[r#"
...
src/lib.rs:3:13: [WARNING] type `FromPriv` from private dependency 'priv_dep' in public interface
...
"#]])
        .run();
}

#[cargo_test(nightly, reason = "exported_private_dependencies lint is unstable")]
fn exported_pub_dep() {
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
                edition = "2015"

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

    p.cargo("check --message-format=short")
        .masquerade_as_nightly_cargo(&["public-dependency"])
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] pub_dep v0.1.0 (registry `dummy-registry`)
[CHECKING] pub_dep v0.1.0
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
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

    p.cargo("check --message-format=short")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  the cargo feature `public-dependency` requires a nightly version of Cargo, but this is the `stable` channel
  See https://doc.rust-lang.org/book/appendix-07-nightly-rust.html for more information about Rust release channels.
  See https://doc.rust-lang.org/[..]cargo/reference/unstable.html#public-dependency for more information about using this feature.

"#]])
        .run();
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
                edition = "2015"

                [dependencies]
                pub_dep = { version = "0.1.0", public = true }
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check --message-format=short")
        .masquerade_as_nightly_cargo(&["public-dependency"])
        .with_stderr_data(str![[r#"
[WARNING] ignoring `public` on dependency pub_dep, pass `-Zpublic-dependency` to enable support for it
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] pub_dep v0.1.0 (registry `dummy-registry`)
[CHECKING] pub_dep v0.1.0
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
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
                edition = "2015"

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

    p.cargo("check --message-format=short")
        .masquerade_as_nightly_cargo(&["public-dependency"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  'public' specifier can only be used on regular dependencies, not dev-dependencies

"#]])
        .run();
}

#[cargo_test]
fn pub_dev_dependency_without_feature() {
    Package::new("pub_dep", "0.1.0")
        .file("src/lib.rs", "pub struct FromPub;")
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"

                [dev-dependencies]
                pub_dep = {version = "0.1.0", public = true}
            "#,
        )
        .file(
            "tests/mod.rs",
            "
            extern crate pub_dep;
            pub fn use_pub(_: pub_dep::FromPub) {}
        ",
        )
        .build();

    p.cargo("check --message-format=short")
        .with_stderr_data(str![[r#"
[WARNING] 'public' specifier can only be used on regular dependencies, not dev-dependencies
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test(nightly, reason = "exported_private_dependencies lint is unstable")]
fn workspace_pub_disallowed() {
    Package::new("foo1", "0.1.0")
        .file("src/lib.rs", "pub struct FromFoo;")
        .publish();
    Package::new("foo2", "0.1.0")
        .file("src/lib.rs", "pub struct FromFoo;")
        .publish();
    Package::new("foo3", "0.1.0")
        .file("src/lib.rs", "pub struct FromFoo;")
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["public-dependency"]

                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"

                [workspace.dependencies]
                foo1 = "0.1.0"
                foo2 = { version = "0.1.0", public = true }
                foo3 = { version = "0.1.0", public = false }

                [dependencies]
                foo1 = { workspace = true, public = true }
                foo2 = { workspace = true }
                foo3 = { workspace = true, public = true }
            "#,
        )
        .file(
            "src/lib.rs",
            "
                #![deny(exported_private_dependencies)]
                pub fn use_priv1(_: foo1::FromFoo) {}
                pub fn use_priv2(_: foo2::FromFoo) {}
                pub fn use_priv3(_: foo3::FromFoo) {}
            ",
        )
        .build();

    p.cargo("check")
        .masquerade_as_nightly_cargo(&["public-dependency"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  foo2 is public, but workspace dependencies cannot be public

"#]])
        .run();
}

#[cargo_test(nightly, reason = "exported_private_dependencies lint is unstable")]
fn allow_priv_in_tests() {
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
                edition = "2015"

                [dependencies]
                priv_dep = {version = "0.1.0", public = false}
            "#,
        )
        .file(
            "tests/mod.rs",
            "
            extern crate priv_dep;
            pub fn use_priv(_: priv_dep::FromPriv) {}
        ",
        )
        .build();

    p.cargo("check --tests --message-format=short")
        .masquerade_as_nightly_cargo(&["public-dependency"])
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] priv_dep v0.1.0 (registry `dummy-registry`)
[CHECKING] priv_dep v0.1.0
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test(nightly, reason = "exported_private_dependencies lint is unstable")]
fn allow_priv_in_benches() {
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
                edition = "2015"

                [dependencies]
                priv_dep = {version = "0.1.0", public = false}
            "#,
        )
        .file(
            "benches/mod.rs",
            "
            extern crate priv_dep;
            pub fn use_priv(_: priv_dep::FromPriv) {}
        ",
        )
        .build();

    p.cargo("check --benches --message-format=short")
        .masquerade_as_nightly_cargo(&["public-dependency"])
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] priv_dep v0.1.0 (registry `dummy-registry`)
[CHECKING] priv_dep v0.1.0
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test(nightly, reason = "exported_private_dependencies lint is unstable")]
fn allow_priv_in_bins() {
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
                edition = "2015"

                [dependencies]
                priv_dep = {version = "0.1.0", public = false}
            "#,
        )
        .file(
            "src/main.rs",
            "
            extern crate priv_dep;
            pub fn use_priv(_: priv_dep::FromPriv) {}
            fn main() {}
        ",
        )
        .build();

    p.cargo("check --bins --message-format=short")
        .masquerade_as_nightly_cargo(&["public-dependency"])
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] priv_dep v0.1.0 (registry `dummy-registry`)
[CHECKING] priv_dep v0.1.0
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test(nightly, reason = "exported_private_dependencies lint is unstable")]
fn allow_priv_in_examples() {
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
                edition = "2015"

                [dependencies]
                priv_dep = {version = "0.1.0", public = false}
            "#,
        )
        .file(
            "examples/lib.rs",
            "
            extern crate priv_dep;
            pub fn use_priv(_: priv_dep::FromPriv) {}
            fn main() {}
        ",
        )
        .build();

    p.cargo("check --examples --message-format=short")
        .masquerade_as_nightly_cargo(&["public-dependency"])
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] priv_dep v0.1.0 (registry `dummy-registry`)
[CHECKING] priv_dep v0.1.0
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test(nightly, reason = "exported_private_dependencies lint is unstable")]
fn allow_priv_in_custom_build() {
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
                edition = "2015"

                [build-dependencies]
                priv_dep = "0.1.0"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "build.rs",
            "
            extern crate priv_dep;
            pub fn use_priv(_: priv_dep::FromPriv) {}
            fn main() {}
        ",
        )
        .build();

    p.cargo("check --all-targets --message-format=short")
        .masquerade_as_nightly_cargo(&["public-dependency"])
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] priv_dep v0.1.0 (registry `dummy-registry`)
[COMPILING] priv_dep v0.1.0
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test(nightly, reason = "exported_private_dependencies lint is unstable")]
fn publish_package_with_public_dependency() {
    Package::new("pub_bar", "0.1.0")
        .file("src/lib.rs", "pub struct FromPub;")
        .publish();
    Package::new("bar", "0.1.0")
        .cargo_feature("public-dependency")
        .add_dep(Dependency::new("pub_bar", "0.1.0").public(true))
        .file(
            "src/lib.rs",
            "
            extern crate pub_bar;
            pub use pub_bar::FromPub as BarFromPub;
        ",
        )
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            cargo-features = ["public-dependency"]
            [package]
            name = "foo"
            version = "0.0.1"
            edition = "2015"
            [dependencies]
            bar = {version = "0.1.0", public = true}
        "#,
        )
        .file(
            "src/lib.rs",
            "
            extern crate bar;
            pub fn use_pub(_: bar::BarFromPub) {}
        ",
        )
        .build();

    p.cargo("check --message-format=short")
        .masquerade_as_nightly_cargo(&["public-dependency"])
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 2 packages to latest compatible versions
[DOWNLOADING] crates ...
[DOWNLOADED] pub_bar v0.1.0 (registry `dummy-registry`)
[DOWNLOADED] bar v0.1.0 (registry `dummy-registry`)
[CHECKING] pub_bar v0.1.0
[CHECKING] bar v0.1.0
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test(nightly, reason = "exported_private_dependencies lint is unstable")]
fn verify_mix_cargo_feature_z() {
    Package::new("dep", "0.1.0")
        .file("src/lib.rs", "pub struct FromDep;")
        .publish();
    Package::new("priv_dep", "0.1.0")
        .file("src/lib.rs", "pub struct FromPriv;")
        .publish();
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
                edition = "2015"

                [dependencies]
                dep = "0.1.0"
                priv_dep = {version = "0.1.0", public = false}
                pub_dep = {version = "0.1.0", public = true}
            "#,
        )
        .file(
            "src/lib.rs",
            "
            extern crate dep;
            extern crate priv_dep;
            extern crate pub_dep;
            pub fn use_dep(_: dep::FromDep) {}
            pub fn use_priv(_: priv_dep::FromPriv) {}
            pub fn use_pub(_: pub_dep::FromPub) {}
        ",
        )
        .build();

    p.cargo("check -Zpublic-dependency --message-format=short")
        .masquerade_as_nightly_cargo(&["public-dependency"])
        .with_stderr_data(str![[r#"
...
src/lib.rs:5:13: [WARNING] type `FromDep` from private dependency 'dep' in public interface
src/lib.rs:6:13: [WARNING] type `FromPriv` from private dependency 'priv_dep' in public interface
...
"#]])
        .run();
}

#[cargo_test(nightly, reason = "exported_private_dependencies lint is unstable")]
fn verify_z_public_dependency() {
    Package::new("dep", "0.1.0")
        .file("src/lib.rs", "pub struct FromDep;")
        .publish();
    Package::new("priv_dep", "0.1.0")
        .file("src/lib.rs", "pub struct FromPriv;")
        .publish();
    Package::new("pub_dep", "0.1.0")
        .file("src/lib.rs", "pub struct FromPub;")
        .publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"

                [dependencies]
                dep = "0.1.0"
                priv_dep = {version = "0.1.0", public = false}
                pub_dep = {version = "0.1.0", public = true}
            "#,
        )
        .file(
            "src/lib.rs",
            "
            extern crate dep;
            extern crate priv_dep;
            extern crate pub_dep;
            pub fn use_dep(_: dep::FromDep) {}
            pub fn use_priv(_: priv_dep::FromPriv) {}
            pub fn use_pub(_: pub_dep::FromPub) {}
        ",
        )
        .build();

    p.cargo("check -Zpublic-dependency --message-format=short")
        .masquerade_as_nightly_cargo(&["public-dependency"])
        .with_stderr_data(
            str![[r#"
...
src/lib.rs:5:13: [WARNING] type `FromDep` from private dependency 'dep' in public interface
src/lib.rs:6:13: [WARNING] type `FromPriv` from private dependency 'priv_dep' in public interface
...
"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test(nightly, reason = "exported_private_dependencies lint is unstable")]
fn manifest_location() {
    Package::new("dep", "0.1.0")
        .file("src/lib.rs", "pub struct FromDep;")
        .publish();
    Package::new("priv_dep", "0.1.0")
        .file("src/lib.rs", "pub struct FromPriv;")
        .publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"

                [dependencies]
                dep = "0.1.0"
                priv_dep = "0.1.0"
            "#,
        )
        .file(
            "src/lib.rs",
            "
            extern crate dep;
            extern crate priv_dep;
            pub fn use_dep(_: dep::FromDep) {}
            pub use priv_dep::FromPriv;
        ",
        )
        .build();

    p.cargo("check -Zpublic-dependency")
        .masquerade_as_nightly_cargo(&["public-dependency"])
        .with_stderr_data(
            str![[r#"
...
[WARNING] struct `FromPriv` from private dependency 'priv_dep' is re-exported
 --> src/lib.rs:5:21
  |
5 |             pub use priv_dep::FromPriv;
  |                     ^^^^^^^^^^^^^^^^^^
  |
  = [NOTE] `#[warn(exported_private_dependencies)]` on by default
[NOTE] dependency `priv_dep` declared here
 --> Cargo.toml:9:17
  |
9 |                 priv_dep = "0.1.0"
  |                 --------

[WARNING] type `FromDep` from private dependency 'dep' in public interface
 --> src/lib.rs:4:13
  |
4 |             pub fn use_dep(_: dep::FromDep) {}
  |             ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
[NOTE] dependency `dep` declared here
 --> Cargo.toml:8:17
  |
8 |                 dep = "0.1.0"
  |                 ---
...
"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test(nightly, reason = "exported_private_dependencies lint is unstable")]
fn renamed_dependency() {
    Package::new("dep", "0.1.0")
        .file("src/lib.rs", "pub struct FromDep;")
        .publish();
    Package::new("priv_dep", "0.1.0")
        .file("src/lib.rs", "pub struct FromPriv;")
        .publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"

                [dependencies]
                dep = { version = "0.1.0", package = "dep" }
                renamed_dep = {version = "0.1.0", package = "priv_dep" }
            "#,
        )
        .file(
            "src/lib.rs",
            "
            extern crate dep;
            extern crate renamed_dep;
            pub fn use_dep(_: dep::FromDep) {}
            pub fn use_priv(_: renamed_dep::FromPriv) {}
        ",
        )
        .build();

    p.cargo("check -Zpublic-dependency")
        .masquerade_as_nightly_cargo(&["public-dependency"])
        .with_stderr_data(
            str![[r#"
...
[WARNING] type `FromDep` from private dependency 'dep' in public interface
 --> src/lib.rs:4:13
  |
4 |             pub fn use_dep(_: dep::FromDep) {}
  |             ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = [NOTE] `#[warn(exported_private_dependencies)]` on by default
[NOTE] dependency `dep` declared here
 --> Cargo.toml:8:54
  |
8 |                 dep = { version = "0.1.0", package = "dep" }
  |                                                      -----

[WARNING] type `FromPriv` from private dependency 'priv_dep' in public interface
 --> src/lib.rs:5:13
  |
5 |             pub fn use_priv(_: renamed_dep::FromPriv) {}
  |             ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
[NOTE] dependency `priv_dep` declared here
 --> Cargo.toml:9:61
  |
9 |                 renamed_dep = {version = "0.1.0", package = "priv_dep" }
  |                                                             ----------
...
"#]]
            .unordered(),
        )
        .run();
}

// We don't point to the toml locations if the crate is ambiguous.
#[cargo_test(nightly, reason = "exported_private_dependencies lint is unstable")]
fn duplicate_renamed_dependency() {
    registry::alt_init();
    Package::new("dep", "0.1.0")
        .file("src/lib.rs", "pub struct FromDep;")
        .publish();
    Package::new("dep", "0.1.0")
        .file("src/lib.rs", "pub struct FromPriv;")
        .alternative(true)
        .publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"

                [dependencies]
                dep = { version = "0.1.0", package = "dep" }
                renamed_dep = {version = "0.1.0", package = "dep", registry = "alternative" }
            "#,
        )
        .file(
            "src/lib.rs",
            "
            extern crate dep;
            extern crate renamed_dep;
            pub fn use_dep(_: dep::FromDep) {}
            pub fn use_priv(_: renamed_dep::FromPriv) {}
        ",
        )
        .build();

    p.cargo("check -Zpublic-dependency")
        .masquerade_as_nightly_cargo(&["public-dependency"])
        .with_stderr_data(
            str![[r#"
...
[WARNING] type `FromDep` from private dependency 'dep' in public interface
 --> src/lib.rs:4:13
  |
4 |             pub fn use_dep(_: dep::FromDep) {}
  |             ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = [NOTE] `#[warn(exported_private_dependencies)]` on by default

[WARNING] type `FromPriv` from private dependency 'dep' in public interface
 --> src/lib.rs:5:13
  |
5 |             pub fn use_priv(_: renamed_dep::FromPriv) {}
  |             ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
...
"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test(nightly, reason = "exported_private_dependencies lint is unstable")]
fn dependency_location_in_target_table() {
    if crate::utils::cross_compile::disabled() {
        return;
    }

    let native = cargo_test_support::cross_compile::native();
    let alt = cargo_test_support::cross_compile::alternate();

    Package::new("dep", "0.1.0")
        .file("src/lib.rs", "pub struct FromDep;")
        .publish();
    Package::new("native_priv_dep", "0.1.0")
        .file("src/lib.rs", "pub struct FromPriv;")
        .publish();
    Package::new("alt_priv_dep", "0.1.0")
        .file("src/lib.rs", "pub struct FromPriv;")
        .publish();
    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"

                [target.{native}.dependencies]
                dep = {{ version = "0.1.0" }}
                renamed_dep = {{ version = "0.1.0", package = "native_priv_dep" }}

                [target.{alt}.dependencies]
                dep = {{ version = "0.1.0" }}
                renamed_dep = {{ version = "0.1.0", package = "alt_priv_dep" }}
            "#
            ),
        )
        .file(
            "src/lib.rs",
            "
            extern crate dep;
            extern crate renamed_dep;
            pub fn use_dep(_: dep::FromDep) {}
            pub fn use_priv(_: renamed_dep::FromPriv) {}
        ",
        )
        .build();

    p.cargo("check -Zpublic-dependency")
        .masquerade_as_nightly_cargo(&["public-dependency"])
        .with_stderr_data(
            str![[r#"
...
[WARNING] type `FromDep` from private dependency 'dep' in public interface
 --> src/lib.rs:4:13
  |
4 |             pub fn use_dep(_: dep::FromDep) {}
  |             ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = [NOTE] `#[warn(exported_private_dependencies)]` on by default
[NOTE] dependency `dep` declared here
 --> Cargo.toml:8:17
  |
8 |                 dep = { version = "0.1.0" }
  |                 ---

[WARNING] type `FromPriv` from private dependency 'native_priv_dep' in public interface
 --> src/lib.rs:5:13
  |
5 |             pub fn use_priv(_: renamed_dep::FromPriv) {}
  |             ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
[NOTE] dependency `native_priv_dep` declared here
 --> Cargo.toml:9:62
  |
9 |                 renamed_dep = { version = "0.1.0", package = "native_priv_dep" }
  |                                                              -----------------
...
"#]]
            .unordered(),
        )
        .run();

    p.cargo(&format!("check -Zpublic-dependency --target={alt}"))
        .masquerade_as_nightly_cargo(&["public-dependency"])
        .with_stderr_data(
            str![[r#"
...
[WARNING] type `FromDep` from private dependency 'dep' in public interface
 --> src/lib.rs:4:13
  |
4 |             pub fn use_dep(_: dep::FromDep) {}
  |             ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = [NOTE] `#[warn(exported_private_dependencies)]` on by default
[NOTE] dependency `dep` declared here
  --> Cargo.toml:12:17
   |
12 |                 dep = { version = "0.1.0" }
   |                 ---

[WARNING] type `FromPriv` from private dependency 'alt_priv_dep' in public interface
 --> src/lib.rs:5:13
  |
5 |             pub fn use_priv(_: renamed_dep::FromPriv) {}
  |             ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
[NOTE] dependency `alt_priv_dep` declared here
  --> Cargo.toml:13:62
   |
13 |                 renamed_dep = { version = "0.1.0", package = "alt_priv_dep" }
   |                                                              --------------
...
"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test(nightly, reason = "exported_private_dependencies lint is unstable")]
fn dependency_location_in_target_table_with_cfg() {
    if crate::utils::cross_compile::disabled() {
        return;
    }

    let native_arch = cargo_test_support::cross_compile::native_arch();
    let alt = cargo_test_support::cross_compile::alternate();

    Package::new("dep", "0.1.0")
        .file("src/lib.rs", "pub struct FromDep;")
        .publish();
    Package::new("native_priv_dep", "0.1.0")
        .file("src/lib.rs", "pub struct FromPriv;")
        .publish();
    Package::new("alt_priv_dep", "0.1.0")
        .file("src/lib.rs", "pub struct FromPriv;")
        .publish();
    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"

                [target.'cfg(target_arch = "{native_arch}")'.dependencies]
                dep = {{ version = "0.1.0" }}
                renamed_dep = {{ version = "0.1.0", package = "native_priv_dep" }}

                [target.'cfg(not(target_arch = "{native_arch}"))'.dependencies]
                dep = {{ version = "0.1.0" }}
                renamed_dep = {{ version = "0.1.0", package = "alt_priv_dep" }}
            "#
            ),
        )
        .file(
            "src/lib.rs",
            "
            extern crate dep;
            extern crate renamed_dep;
            pub fn use_dep(_: dep::FromDep) {}
            pub fn use_priv(_: renamed_dep::FromPriv) {}
        ",
        )
        .build();

    p.cargo("check -Zpublic-dependency")
        .masquerade_as_nightly_cargo(&["public-dependency"])
        .with_stderr_data(
            str![[r#"
...
[WARNING] type `FromDep` from private dependency 'dep' in public interface
 --> src/lib.rs:4:13
  |
4 |             pub fn use_dep(_: dep::FromDep) {}
  |             ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = [NOTE] `#[warn(exported_private_dependencies)]` on by default
[NOTE] dependency `dep` declared here
 --> Cargo.toml:8:17
  |
8 |                 dep = { version = "0.1.0" }
  |                 ---

[WARNING] type `FromPriv` from private dependency 'native_priv_dep' in public interface
 --> src/lib.rs:5:13
  |
5 |             pub fn use_priv(_: renamed_dep::FromPriv) {}
  |             ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
[NOTE] dependency `native_priv_dep` declared here
 --> Cargo.toml:9:62
  |
9 |                 renamed_dep = { version = "0.1.0", package = "native_priv_dep" }
  |                                                              -----------------
...
"#]]
            .unordered(),
        )
        .run();

    p.cargo(&format!("check -Zpublic-dependency --target={alt}"))
        .masquerade_as_nightly_cargo(&["public-dependency"])
        .with_stderr_data(
            str![[r#"
...
[WARNING] type `FromDep` from private dependency 'dep' in public interface
 --> src/lib.rs:4:13
  |
4 |             pub fn use_dep(_: dep::FromDep) {}
  |             ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = [NOTE] `#[warn(exported_private_dependencies)]` on by default
[NOTE] dependency `dep` declared here
  --> Cargo.toml:12:17
   |
12 |                 dep = { version = "0.1.0" }
   |                 ---

[WARNING] type `FromPriv` from private dependency 'alt_priv_dep' in public interface
 --> src/lib.rs:5:13
  |
5 |             pub fn use_priv(_: renamed_dep::FromPriv) {}
  |             ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
[NOTE] dependency `alt_priv_dep` declared here
  --> Cargo.toml:13:62
   |
13 |                 renamed_dep = { version = "0.1.0", package = "alt_priv_dep" }
   |                                                              --------------
...
"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test(nightly, reason = "exported_private_dependencies lint is unstable")]
fn dependency_location_in_workspace() {
    Package::new("dep", "0.1.0")
        .file("src/lib.rs", "pub struct FromDep;")
        .publish();
    let (p, repo) = git::new_repo("foo", |p| {
        p.file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["pkg"]
                [workspace.package]
                edition = "2015"
                [workspace.dependencies]
                dep = "0.1.0"
            "#,
        )
        .file(
            "pkg/Cargo.toml",
            r#"
                [package]
                name = "pkg"
                edition.workspace = true
                [dependencies]
                dep.workspace = true
            "#,
        )
        .file(
            "pkg/src/lib.rs",
            "
            extern crate dep;
            pub fn use_dep(_: dep::FromDep) {}
        ",
        )
    });
    git::commit(&repo);
    p.cargo(&format!("check -Zpublic-dependency"))
        .masquerade_as_nightly_cargo(&["public-dependency"])
        .with_stderr_data(str![[r#"
...
[WARNING] type `FromDep` from private dependency 'dep' in public interface
 --> pkg/src/lib.rs:3:13
  |
3 |             pub fn use_dep(_: dep::FromDep) {}
  |             ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = [NOTE] `#[warn(exported_private_dependencies)]` on by default
...
"#]])
        .run();
}

#[cargo_test(nightly, reason = "exported_private_dependencies lint is unstable")]
fn relative_display_path() {
    Package::new("priv_dep", "0.1.0")
        .file("src/lib.rs", "pub struct FromPriv;")
        .publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["foo"]
            "#,
        )
        .file(
            "foo/Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"

                [dependencies]
                priv_dep = "0.1.0"
            "#,
        )
        .file(
            "foo/src/lib.rs",
            "
            extern crate priv_dep;
            pub use priv_dep::FromPriv;
        ",
        )
        .build();
    p.cargo("check -Zpublic-dependency")
        .cwd("foo")
        .masquerade_as_nightly_cargo(&["public-dependency"])
        .with_stderr_data(str![[r#"
...
[WARNING] struct `FromPriv` from private dependency 'priv_dep' is re-exported
 --> foo/src/lib.rs:3:21
  |
3 |             pub use priv_dep::FromPriv;
  |                     ^^^^^^^^^^^^^^^^^^
  |
  = [NOTE] `#[warn(exported_private_dependencies)]` on by default
[NOTE] dependency `priv_dep` declared here
 --> foo/Cargo.toml:8:17
  |
8 |                 priv_dep = "0.1.0"
  |                 --------
...
"#]])
        .run();
}
