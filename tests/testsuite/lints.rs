use support::{execs, project};
use support::hamcrest::assert_that;

#[test]
fn deny() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [lints]
            dead_code = "deny"
        "#,
        )
        .file("src/lib.rs", "fn foo() {}")
        .build();

    assert_that(
        p.cargo("build"),
        execs()
            .with_status(101)
            .with_stderr_contains("[..]error: function is never used: `foo`[..]"),
    );
}

#[test]
fn empty_lints_block() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [lints]
        "#,
        )
        .file("src/lib.rs", "fn foo() {}")
        .build();

    assert_that(
        p.cargo("build"),
        execs().with_status(0),
    );
}

#[test]
fn invalid_state() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [lints]
            non_snake_case = "something something"
        "#,
        )
        .file("src/lib.rs", "fn foo() {}")
        .build();

    assert_that(
        p.cargo("build"),
        execs().with_status(0),
    );
}

#[test]
fn virtual_workspace() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["bar"]

            [lints]
            dead_code = "deny"
        "#,
        )
        .file(
            "bar/Cargo.toml",
            r#"
            [project]
            name = "bar"
            version = "0.1.0"
            authors = []
        "#,
        )
        .file("bar/src/lib.rs", "fn baz() {}")
        .build();

    assert_that(
        p.cargo("build"),
        execs()
            .with_status(101)
            .with_stderr_contains("[..]error: function is never used: `baz`[..]"),
    );
}

#[test]
fn member_workspace() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["bar"]
        "#,
        )
        .file(
            "bar/Cargo.toml",
            r#"
            [project]
            name = "bar"
            version = "0.1.0"
            authors = []

            [lints]
            dead_code = "deny"
        "#,
        )
        .file("bar/src/lib.rs", "fn baz() {}")
        .build();

    assert_that(
        p.cargo("build"),
        execs()
            .with_status(101)
            .with_stderr_contains("[..]error: function is never used: `baz`[..]"),
    );
}

#[test]
fn virtual_workspace_overrides() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["bar"]

            [lints]
            dead_code = "deny"
        "#,
        )
        .file(
            "bar/Cargo.toml",
            r#"
            [project]
            name = "bar"
            version = "0.1.0"
            authors = []

            [lints]
            dead_code = "allow"
        "#,
        )
        .file("bar/src/lib.rs", "fn baz() {}")
        .build();

    assert_that(
        p.cargo("build"),
        execs()
            .with_status(101)
            .with_stderr_contains("[..]error: function is never used: `baz`[..]"),
    );
}
