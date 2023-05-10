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
