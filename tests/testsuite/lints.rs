//! Tests for `[lints]`

use cargo_test_support::project;

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
