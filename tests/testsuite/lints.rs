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
                authors = []

            "#,
        )
        .file("src/lib.rs", "")
        .build();

    foo.cargo("check")
        .with_status(101)
        .with_stderr(
            "\
error: failed to parse manifest[..]

Caused by:
  TOML parse error at line 2, column 25
    |
  2 |                 lints = 20
    |                         ^^
  invalid type: integer `20`, expected a lints table
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
                authors = []

                [workspace.lints.super-awesome-linter]
                unsafe_code = "forbid"
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
  unsupported `super-awesome-linter` in `[lints]`, must be one of rust, clippy, rustdoc
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
error: failed to parse manifest at `[..]/Cargo.toml`

Caused by:
  TOML parse error at line 7, column 36
    |
  7 |                 rust-2018-idioms = -1
    |                                    ^^
  invalid type: integer `-1`, expected a string or map
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
fn workspace_lint_deny() {
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
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
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
",
        )
        .run();

    foo.cargo("test --doc")
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]s
[DOCTEST] foo
",
        )
        .run();
}
