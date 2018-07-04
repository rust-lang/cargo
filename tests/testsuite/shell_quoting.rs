//! this file tests that when the commands being run are shown
//! in the output, their arguments are quoted properly
//! so that the command can be run in a terminal

use cargotest::support::{
    execs,
    project,
};
use hamcrest::assert_that;

#[test]
fn features_are_quoted() {
    let p = project("foo")
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.1.0"
            authors = ["mikeyhew@example.com"]

            [features]
            some_feature = []
            default = ["some_feature"]
            "#,
        )
        .file("src/main.rs", "fn main() {error}")
        .build();

    assert_that(
        p.cargo("check -v"),
        execs()
            .with_status(101)
            .with_stderr_contains(
                r#"[RUNNING] `rustc [..] --cfg 'feature="default"' --cfg 'feature="some_feature"' [..]`"#
            ).with_stderr_contains(
                r#"
Caused by:
  process didn't exit successfully: [..] --cfg 'feature="default"' --cfg 'feature="some_feature"' [..]"#
            )
    );
}
