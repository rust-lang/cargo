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
