use cargotest::support::{execs, project};
use hamcrest::assert_that;

#[test]
fn deny() {
    let p = project("foo")
        .file(
            "Cargo.toml",
            r#"
            [project]
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
    let p = project("foo")
        .file(
            "Cargo.toml",
            r#"
            [project]
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
    let p = project("foo")
        .file(
            "Cargo.toml",
            r#"
            [project]
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
