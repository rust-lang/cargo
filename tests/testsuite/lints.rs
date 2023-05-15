//! Tests for `[lints]`

use cargo_test_support::project;
use cargo_test_support::registry::Package;

#[cargo_test]
fn package_requires_option() {
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [lints.rust]
                unsafe_code = "forbid"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    foo.cargo("check")
        .with_status(101)
        .with_stderr("\
[..]

Caused by:
  feature `lints` is required

  The package requires the Cargo feature called `lints`, but that feature is not stabilized in this version of Cargo ([..]).
  Consider trying a newer version of Cargo (this may require the nightly release).
  See https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#lints for more information about the status of this feature.
")
        .run();
}

#[cargo_test]
fn workspace_requires_option() {
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [workspace.lints.rust]
                unsafe_code = "forbid"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    foo.cargo("check")
        .with_status(101)
        .with_stderr("\
[..]

Caused by:
  feature `lints` is required

  The package requires the Cargo feature called `lints`, but that feature is not stabilized in this version of Cargo ([..]).
  Consider trying a newer version of Cargo (this may require the nightly release).
  See https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#lints for more information about the status of this feature.
")
        .run();
}

#[cargo_test]
fn fail_on_invalid_tool() {
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["lints"]

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
        .masquerade_as_nightly_cargo(&["lints"])
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
fn fail_on_tool_injection() {
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["lints"]

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
        .masquerade_as_nightly_cargo(&["lints"])
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
                cargo-features = ["lints"]

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
        .masquerade_as_nightly_cargo(&["lints"])
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
                cargo-features = ["lints"]

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
        .masquerade_as_nightly_cargo(&["lints"])
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
                cargo-features = ["lints"]

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
        .masquerade_as_nightly_cargo(&["lints"])
        .with_status(101)
        .with_stderr(
            "\
[..]
error: usage of an `unsafe` block
[..]
[..]
[..]
[..]
[..]
[..]
[..]
error: could not compile `foo` (lib) due to previous error
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
                cargo-features = ["lints"]

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
        .masquerade_as_nightly_cargo(&["lints"])
        .with_status(101)
        .with_stderr(
            "\
[..]
error: usage of an `unsafe` block
[..]
[..]
[..]
[..]
[..]
[..]
[..]
error: could not compile `foo` (lib) due to previous error
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
                cargo-features = ["lints"]

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
        .masquerade_as_nightly_cargo(&["lints"])
        .with_status(0)
        .run();
}

#[cargo_test]
fn rustflags_has_precedence() {
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["lints"]

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
        .arg("-v")
        .env("RUSTFLAGS", "-Aunsafe_code")
        .masquerade_as_nightly_cargo(&["lints"])
        .with_status(0)
        .run();
}

#[cargo_test]
fn without_priority() {
    Package::new("reg-dep", "1.0.0").publish();

    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["lints"]

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
        .masquerade_as_nightly_cargo(&["lints"])
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
                cargo-features = ["lints"]

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

    foo.cargo("check")
        .masquerade_as_nightly_cargo(&["lints"])
        .with_status(0)
        .run();
}

#[cargo_test]
fn rustdoc_lint() {
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["lints"]

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
        .masquerade_as_nightly_cargo(&["lints"])
        .with_status(101)
        .with_stderr_contains(
            "\
error: unresolved link to `bar`
",
        )
        .run();
}
