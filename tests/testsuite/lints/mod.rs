use crate::prelude::*;
use cargo_test_support::project;
use cargo_test_support::registry::Package;
use cargo_test_support::str;

mod blanket_hint_mostly_unused;
mod error;
mod inherited;
mod unknown_lints;
mod warning;

#[cargo_test]
fn dashes_dont_get_rewritten() {
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
cargo-features = ["test-dummy-unstable"]

[package]
name = "foo"
version = "0.0.1"
edition = "2015"
authors = []
im-a-teapot = true

[lints.cargo]
im-a-teapot = "warn"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    foo.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints", "test-dummy-unstable"])
        .with_stderr_data(str![[r#"
[WARNING] unknown lint: `im-a-teapot`
  --> Cargo.toml:12:1
   |
12 | im-a-teapot = "warn"
   | ^^^^^^^^^^^
   |
   = [NOTE] `cargo::unknown_lints` is set to `warn` by default
   = [HELP] there is a lint with a similar name: `im_a_teapot`
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn forbid_not_overridden() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
cargo-features = ["test-dummy-unstable"]

[package]
name = "foo"
version = "0.0.1"
edition = "2015"
authors = []
im-a-teapot = true

[lints.cargo]
im_a_teapot = { level = "warn", priority = 10 }
test_dummy_unstable = { level = "forbid", priority = -1 }
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints", "test-dummy-unstable"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] `im_a_teapot` is specified
 --> Cargo.toml:9:1
  |
9 | im-a-teapot = true
  | ^^^^^^^^^^^^^^^^^^
  |
  = [NOTE] `cargo::im_a_teapot` is set to `forbid` in `[lints]`

"#]])
        .run();
}

#[cargo_test]
fn workspace_lints() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
cargo-features = ["test-dummy-unstable"]

[workspace.lints.cargo]
im_a_teapot = { level = "warn", priority = 10 }
test_dummy_unstable = { level = "forbid", priority = -1 }

[package]
name = "foo"
version = "0.0.1"
edition = "2015"
authors = []
im-a-teapot = true

[lints]
workspace = true
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints", "test-dummy-unstable"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] `im_a_teapot` is specified
  --> Cargo.toml:13:1
   |
13 | im-a-teapot = true
   | ^^^^^^^^^^^^^^^^^^
   |
   = [NOTE] `cargo::im_a_teapot` is set to `forbid` in `[lints]`

"#]])
        .run();
}

#[cargo_test]
fn dont_always_inherit_workspace_lints() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
[workspace]
members = ["foo"]

[workspace.lints.cargo]
im_a_teapot = "warn"
"#,
        )
        .file(
            "foo/Cargo.toml",
            r#"
cargo-features = ["test-dummy-unstable"]

[package]
name = "foo"
version = "0.0.1"
edition = "2015"
authors = []
im-a-teapot = true
"#,
        )
        .file("foo/src/lib.rs", "")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.0.1 ([ROOT]/foo/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn cap_lints() {
    Package::new("baz", "0.1.0").publish();
    Package::new("bar", "0.1.0")
        .file(
            "Cargo.toml",
            r#"
cargo-features = ["test-dummy-unstable"]

[package]
name = "bar"
version = "0.1.0"
edition = "2021"
im-a-teapot = true

[dependencies]
baz = { version = "0.1.0", optional = true }

[lints.cargo]
im_a_teapot = "warn"
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
edition = "2021"

[dependencies]
bar = "0.1.0"
"#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] bar v0.1.0 (registry `dummy-registry`)
[CHECKING] bar v0.1.0
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn check_feature_gated() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
[package]
name = "foo"
version = "0.0.1"
edition = "2015"
authors = []

[lints.cargo]
im_a_teapot = "warn"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] use of unstable lint `im_a_teapot`
 --> Cargo.toml:9:1
  |
9 | im_a_teapot = "warn"
  | ^^^^^^^^^^^ this is behind `test-dummy-unstable`, which is not enabled
  |
  = [HELP] consider adding `cargo-features = ["test-dummy-unstable"]` to the top of the manifest
[ERROR] encountered 1 errors(s) while verifying lints

"#]])
        .run();
}

#[cargo_test]
fn check_feature_gated_workspace() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
[workspace]
members = ["foo"]

[workspace.lints.cargo]
im_a_teapot = { level = "warn", priority = 10 }
test_dummy_unstable = { level = "forbid", priority = -1 }
            "#,
        )
        .file(
            "foo/Cargo.toml",
            r#"
[package]
name = "foo"
version = "0.0.1"
edition = "2015"
authors = []

[lints]
workspace = true
            "#,
        )
        .file("foo/src/lib.rs", "")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] use of unstable lint `im_a_teapot`
 --> Cargo.toml:6:1
  |
6 | im_a_teapot = { level = "warn", priority = 10 }
  | ^^^^^^^^^^^ this is behind `test-dummy-unstable`, which is not enabled
  |
  = [HELP] consider adding `cargo-features = ["test-dummy-unstable"]` to the top of the manifest
[NOTE] `cargo::im_a_teapot` was inherited
 --> foo/Cargo.toml:9:1
  |
9 | workspace = true
  | ----------------
[ERROR] use of unstable lint `test_dummy_unstable`
 --> Cargo.toml:7:1
  |
7 | test_dummy_unstable = { level = "forbid", priority = -1 }
  | ^^^^^^^^^^^^^^^^^^^ this is behind `test-dummy-unstable`, which is not enabled
  |
  = [HELP] consider adding `cargo-features = ["test-dummy-unstable"]` to the top of the manifest
[NOTE] `cargo::test_dummy_unstable` was inherited
 --> foo/Cargo.toml:9:1
  |
9 | workspace = true
  | ----------------
[ERROR] encountered 2 errors(s) while verifying lints

"#]])
        .run();
}
