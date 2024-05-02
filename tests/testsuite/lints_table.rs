//! Tests for `[lints]`

use cargo_test_support::project;
use cargo_test_support::registry::Package;

#[cargo_test]
fn dependency_warning_ignored() {
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies]
                bar.path = "../bar"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    let _bar = project()
        .at("bar")
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [lints.rust]
                unsafe_code = "forbid"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    foo.cargo("check")
        .with_stderr(
            "\
[LOCKING] 2 packages to latest compatible versions
[CHECKING] [..]
[CHECKING] [..]
[FINISHED] [..]
",
        )
        .run();
}

#[cargo_test]
fn malformed_on_stable() {
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                lints = 20
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

            "#,
        )
        .file("src/lib.rs", "")
        .build();

    foo.cargo("check")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] invalid type: integer `20`, expected a lints table
 --> Cargo.toml:2:25
  |
2 |                 lints = 20
  |                         ^^
  |
",
        )
        .run();
}

#[cargo_test]
fn fail_on_invalid_tool() {
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [workspace.lints.super-awesome-linter]
                unsafe_code = "forbid"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    foo.cargo("check")
        .with_stderr(
            "\
[WARNING] [CWD]/Cargo.toml: unrecognized lint tool `lints.super-awesome-linter`, specifying unrecognized tools may break in the future.
supported tools: cargo, clippy, rust, rustdoc
[CHECKING] foo v0.0.1 ([CWD])
[FINISHED] [..]
",
        )
        .run();
}

#[cargo_test]
fn invalid_type_in_lint_value() {
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"

                [workspace.lints.rust]
                rust-2018-idioms = -1
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    foo.cargo("check")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] invalid type: integer `-1`, expected a string or map
 --> Cargo.toml:8:36
  |
8 |                 rust-2018-idioms = -1
  |                                    ^^
  |
",
        )
        .run();
}

#[cargo_test]
fn warn_on_unused_key() {
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"

                [workspace.lints.rust]
                rust-2018-idioms = { level = "allow", unused = true }
                [lints.rust]
                rust-2018-idioms = { level = "allow", unused = true }
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    foo.cargo("check")
        .with_stderr(
            "\
[WARNING] [CWD]/Cargo.toml: unused manifest key: lints.rust.rust-2018-idioms.unused
[WARNING] [CWD]/Cargo.toml: unused manifest key: workspace.lints.rust.rust-2018-idioms.unused
[CHECKING] foo v0.0.1 ([CWD])
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [..]s
",
        )
        .run();
}

#[cargo_test]
fn fail_on_tool_injection() {
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [workspace.lints.rust]
                "clippy::cyclomatic_complexity" = "warn"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    foo.cargo("check")
        .with_status(101)
        .with_stderr(
            "\
[..]

Caused by:
  `lints.rust.clippy::cyclomatic_complexity` is not valid lint name; try `lints.clippy.cyclomatic_complexity`
",
        )
        .run();
}

#[cargo_test]
fn fail_on_redundant_tool() {
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [workspace.lints.rust]
                "rust::unsafe_code" = "forbid"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    foo.cargo("check")
        .with_status(101)
        .with_stderr(
            "\
[..]

Caused by:
  `lints.rust.rust::unsafe_code` is not valid lint name; try `lints.rust.unsafe_code`
",
        )
        .run();
}

#[cargo_test]
fn fail_on_conflicting_tool() {
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [workspace.lints.rust]
                "super-awesome-tool::unsafe_code" = "forbid"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    foo.cargo("check")
        .with_status(101)
        .with_stderr(
            "\
[..]

Caused by:
  `lints.rust.super-awesome-tool::unsafe_code` is not a valid lint name
",
        )
        .run();
}

#[cargo_test]
fn package_lint_deny() {
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [lints.rust]
                "unsafe_code" = "deny"
            "#,
        )
        .file(
            "src/lib.rs",
            "
pub fn foo(num: i32) -> u32 {
    unsafe { std::mem::transmute(num) }
}
",
        )
        .build();

    foo.cargo("check")
        .with_status(101)
        .with_stderr_contains(
            "\
error: usage of an `unsafe` block
",
        )
        .run();
}

#[cargo_test]
fn workspace_cant_be_false() {
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [lints]
                workspace = false

                [workspace.lints.rust]
                "unsafe_code" = "deny"
            "#,
        )
        .file(
            "src/lib.rs",
            "
pub fn foo(num: i32) -> u32 {
    unsafe { std::mem::transmute(num) }
}
",
        )
        .build();

    foo.cargo("check")
        .with_status(101)
        .with_stderr_contains(
            "\
error: `workspace` cannot be false
 --> Cargo.toml:9:29
  |
9 |                 workspace = false
  |                             ^^^^^
  |
",
        )
        .run();
}

#[cargo_test]
fn workspace_lint_deny() {
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [lints]
                workspace = true

                [workspace.lints.rust]
                "unsafe_code" = "deny"
            "#,
        )
        .file(
            "src/lib.rs",
            "
pub fn foo(num: i32) -> u32 {
    unsafe { std::mem::transmute(num) }
}
",
        )
        .build();

    foo.cargo("check")
        .with_status(101)
        .with_stderr_contains(
            "\
error: usage of an `unsafe` block
",
        )
        .run();
}

#[cargo_test]
fn workspace_and_package_lints() {
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [lints]
                workspace = true
                [lints.rust]
                "unsafe_code" = "allow"

                [workspace.lints.rust]
                "unsafe_code" = "deny"
            "#,
        )
        .file(
            "src/lib.rs",
            "
pub fn foo(num: i32) -> u32 {
    unsafe { std::mem::transmute(num) }
}
",
        )
        .build();

    foo.cargo("check")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] failed to parse manifest at `[CWD]/Cargo.toml`

Caused by:
  cannot override `workspace.lints` in `lints`, either remove the overrides or `lints.workspace = true` and manually specify the lints
",
        )
        .run();
}

#[cargo_test]
fn attribute_has_precedence() {
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [lints.rust]
                "unsafe_code" = "deny"
            "#,
        )
        .file(
            "src/lib.rs",
            "
#![allow(unsafe_code)]

pub fn foo(num: i32) -> u32 {
    unsafe { std::mem::transmute(num) }
}
",
        )
        .build();

    foo.cargo("check")
        .arg("-v") // Show order of rustflags on failure
        .run();
}

#[cargo_test]
fn rustflags_has_precedence() {
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [lints.rust]
                "unsafe_code" = "deny"
            "#,
        )
        .file(
            "src/lib.rs",
            "
pub fn foo(num: i32) -> u32 {
    unsafe { std::mem::transmute(num) }
}
",
        )
        .build();

    foo.cargo("check")
        .arg("-v") // Show order of rustflags on failure
        .env("RUSTFLAGS", "-Aunsafe_code")
        .run();
}

#[cargo_test]
fn profile_rustflags_has_precedence() {
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["profile-rustflags"]

                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"

                [lints.rust]
                "unsafe_code" = "deny"

                [profile.dev]
                rustflags = ["-A", "unsafe_code"]
            "#,
        )
        .file(
            "src/lib.rs",
            "
pub fn foo(num: i32) -> u32 {
    unsafe { std::mem::transmute(num) }
}
",
        )
        .build();

    foo.cargo("check")
        .arg("-v") // Show order of rustflags on failure
        .masquerade_as_nightly_cargo(&["profile-rustflags"])
        .run();
}

#[cargo_test]
fn build_rustflags_has_precedence() {
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"

                [lints.rust]
                "unsafe_code" = "deny"
            "#,
        )
        .file(
            ".cargo/config.toml",
            r#"
                [build]
                rustflags = ["-A", "unsafe_code"]
"#,
        )
        .file(
            "src/lib.rs",
            "
pub fn foo(num: i32) -> u32 {
    unsafe { std::mem::transmute(num) }
}
",
        )
        .build();

    foo.cargo("check")
        .arg("-v") // Show order of rustflags on failure
        .run();
}

#[cargo_test]
fn without_priority() {
    Package::new("reg-dep", "1.0.0").publish();

    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2018"
                authors = []

                [dependencies]
                reg-dep = "1.0.0"

                [lints.rust]
                "rust-2018-idioms" = "deny"
                "unused-extern-crates" = "allow"
            "#,
        )
        .file(
            "src/lib.rs",
            "
extern crate reg_dep;

pub fn foo() -> u32 {
    2
}
",
        )
        .build();

    foo.cargo("check")
        .with_status(101)
        .with_stderr_contains(
            "\
error: unused extern crate
",
        )
        .run();
}

#[cargo_test]
fn with_priority() {
    Package::new("reg-dep", "1.0.0").publish();

    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2018"
                authors = []

                [dependencies]
                reg-dep = "1.0.0"

                [lints.rust]
                "rust-2018-idioms" = { level = "deny", priority = -1 }
                "unused-extern-crates" = "allow"
            "#,
        )
        .file(
            "src/lib.rs",
            "
extern crate reg_dep;

pub fn foo() -> u32 {
    2
}
",
        )
        .build();

    foo.cargo("check").run();
}

#[cargo_test]
fn rustdoc_lint() {
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [lints.rustdoc]
                broken_intra_doc_links = "deny"
            "#,
        )
        .file(
            "src/lib.rs",
            "
/// [`bar`] doesn't exist
pub fn foo() -> u32 {
}
",
        )
        .build();

    foo.cargo("doc")
        .with_status(101)
        .with_stderr_contains(
            "\
error: unresolved link to `bar`
",
        )
        .run();
}

#[cargo_test]
fn doctest_respects_lints() {
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [lints.rust]
                confusable-idents = 'allow'
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
/// Test
///
/// [`Foo`]
///
/// ```
/// let s = "rust";
/// let ｓ_ｓ = "rust2";
/// ```
pub fn f() {}
pub const Ě: i32 = 1;
pub const Ĕ: i32 = 2;
"#,
        )
        .build();

    foo.cargo("check")
        .with_stderr(
            "\
[CHECKING] foo v0.0.1 ([CWD])
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [..]s
",
        )
        .run();

    foo.cargo("test --doc")
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [..]s
[DOCTEST] foo
",
        )
        .run();
}

#[cargo_test]
fn cargo_lints_nightly_required() {
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
[package]
name = "foo"
version = "0.0.1"
edition = "2015"
authors = []

[lints.cargo]
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    foo.cargo("check")
        .with_stderr(
            "\
[WARNING] unused manifest key `lints.cargo` (may be supported in a future version)

this Cargo does not support nightly features, but if you
switch to nightly channel you can pass
`-Zcargo-lints` to enable this feature.
[CHECKING] foo v0.0.1 ([CWD])
[FINISHED] [..]
",
        )
        .run();
}

#[cargo_test]
fn cargo_lints_no_z_flag() {
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
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    foo.cargo("check")
        .masquerade_as_nightly_cargo(&["cargo-lints", "test-dummy-unstable"])
        .with_stderr(
            "\
[WARNING] unused manifest key `lints.cargo` (may be supported in a future version)

consider passing `-Zcargo-lints` to enable this feature.
[CHECKING] foo v0.0.1 ([CWD])
[FINISHED] [..]
",
        )
        .run();
}

#[cargo_test]
fn cargo_lints_success() {
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
im_a_teapot = "warn"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints", "test-dummy-unstable"])
        .with_stderr(
            "\
warning: `im_a_teapot` is specified
 --> Cargo.toml:9:1
  |
9 | im-a-teapot = true
  | ------------------
  |
  = note: `cargo::im_a_teapot` is set to `warn` in `[lints]`
[CHECKING] foo v0.0.1 ([CWD])
[FINISHED] [..]
",
        )
        .run();
}

#[cargo_test]
fn cargo_lints_dashes_dont_get_rewritten() {
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
        .with_stderr(
            "\
warning: unknown lint: `im-a-teapot`
  --> Cargo.toml:12:1
   |
12 | im-a-teapot = \"warn\"
   | ^^^^^^^^^^^
   |
   = note: `cargo::unknown_lints` is set to `warn` by default
   = help: there is a lint with a similar name: `im_a_teapot`
[CHECKING] foo v0.0.1 ([CWD])
[FINISHED] [..]
",
        )
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
        .with_stderr(
            "\
error: `im_a_teapot` is specified
 --> Cargo.toml:9:1
  |
9 | im-a-teapot = true
  | ^^^^^^^^^^^^^^^^^^
  |
  = note: `cargo::im_a_teapot` is set to `forbid` in `[lints]`
",
        )
        .run();
}

#[cargo_test]
fn workspace_cargo_lints() {
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
        .with_stderr(
            "\
error: `im_a_teapot` is specified
  --> Cargo.toml:13:1
   |
13 | im-a-teapot = true
   | ^^^^^^^^^^^^^^^^^^
   |
   = note: `cargo::im_a_teapot` is set to `forbid` in `[lints]`
",
        )
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
        .with_stderr(
            "\
[CHECKING] foo v0.0.1 ([CWD]/foo)
[FINISHED] [..]
",
        )
        .run();
}

#[cargo_test]
fn cargo_lints_cap_lints() {
    Package::new("baz", "0.1.0").publish();
    Package::new("bar", "0.1.0")
        .file(
            "Cargo.toml",
            r#"
[package]
name = "bar"
version = "0.1.0"
edition = "2021"

[dependencies]
baz = { version = "0.1.0", optional = true }

[lints.cargo]
implicit_features = "warn"
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

[lints.cargo]
implicit_features = "warn"
"#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr(
            "\
[UPDATING] [..]
[LOCKING] 2 packages to latest compatible versions
[DOWNLOADING] crates ...
[DOWNLOADED] bar v0.1.0 ([..])
[CHECKING] bar v0.1.0
[CHECKING] foo v0.1.0 ([CWD])
[FINISHED] [..]
",
        )
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
        .with_stderr(
            "\
error: use of unstable lint `im_a_teapot`
 --> Cargo.toml:9:1
  |
9 | im_a_teapot = \"warn\"
  | ^^^^^^^^^^^ this is behind `test-dummy-unstable`, which is not enabled
  |
  = help: consider adding `cargo-features = [\"test-dummy-unstable\"]` to the top of the manifest
error: encountered 1 errors(s) while verifying lints
",
        )
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
        .with_stderr(
            "\
error: use of unstable lint `im_a_teapot`
 --> Cargo.toml:6:1
  |
6 | im_a_teapot = { level = \"warn\", priority = 10 }
  | ^^^^^^^^^^^ this is behind `test-dummy-unstable`, which is not enabled
  |
note: `cargo::im_a_teapot` was inherited
 --> foo/Cargo.toml:9:1
  |
9 | workspace = true
  | ----------------
  |
  = help: consider adding `cargo-features = [\"test-dummy-unstable\"]` to the top of the manifest
error: use of unstable lint `test_dummy_unstable`
 --> Cargo.toml:7:1
  |
7 | test_dummy_unstable = { level = \"forbid\", priority = -1 }
  | ^^^^^^^^^^^^^^^^^^^ this is behind `test-dummy-unstable`, which is not enabled
  |
note: `cargo::test_dummy_unstable` was inherited
 --> foo/Cargo.toml:9:1
  |
9 | workspace = true
  | ----------------
  |
  = help: consider adding `cargo-features = [\"test-dummy-unstable\"]` to the top of the manifest
error: encountered 2 errors(s) while verifying lints
",
        )
        .run();
}
