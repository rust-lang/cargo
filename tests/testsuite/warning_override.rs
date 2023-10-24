//! Tests for overriding warning behavior using `--warnings=` on the CLI or `term.warnings` config option.

use cargo_test_support::{project, Project};

const WARNING1: &'static str = "[WARNING] unused variable: `x`";
const WARNING2: &'static str = "[WARNING] unused config key `build.xyz` in `[..]`";

fn make_project(main_src: &str) -> Project {
    project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []
            "#,
        )
        .file("src/main.rs", &format!("fn main() {{ {} }}", main_src))
        .build()
}

#[cargo_test]
fn rustc_warnings() {
    let p = make_project("let x = 3;");
    p.cargo("check --warnings=warn")
        .with_stderr_contains(WARNING1)
        .run();

    p.cargo("check --warnings=error")
        .with_stderr_contains(WARNING1)
        .with_stderr_contains(
            "[ERROR] warnings detected and warnings are disallowed by configuration",
        )
        .with_status(101)
        .run();

    p.cargo("check --warnings=ignore")
        .with_stderr_does_not_contain(WARNING1)
        .run();
}

#[cargo_test]
fn config() {
    let p = make_project("let x = 3;");
    p.cargo("check")
        .env("CARGO_TERM_WARNINGS", "error")
        .with_stderr_contains(WARNING1)
        .with_stderr_contains(
            "[ERROR] warnings detected and warnings are disallowed by configuration",
        )
        .with_status(101)
        .run();

    // CLI has precedence over config
    p.cargo("check --warnings=ignore")
        .env("CARGO_TERM_WARNINGS", "error")
        .with_stderr_does_not_contain(WARNING1)
        .run();

    p.cargo("check")
        .env("CARGO_TERM_WARNINGS", "ignore")
        .with_stderr_does_not_contain(WARNING1)
        .run();
}

#[cargo_test]
/// Warnings that come from cargo rather than rustc
fn cargo_warnings() {
    let p = make_project("");
    p.change_file(".cargo/config.toml", "[build]\nxyz = false");
    p.cargo("check").with_stderr_contains(WARNING2).run();

    p.cargo("check --warnings=error")
        .with_stderr_contains(WARNING2)
        .with_stderr_contains(
            "[ERROR] warnings detected and warnings are disallowed by configuration",
        )
        .with_status(101)
        .run();

    p.cargo("check --warnings=ignore")
        .with_stderr_does_not_contain(WARNING2)
        .run();
}
