//! Tests for overriding warning behavior using `build.warnings` config option.

use std::sync::LazyLock;

use cargo_test_support::{cargo_test, project, str, tools, Project};
use snapbox::data::Inline;

const ALLOW_CLEAN: LazyLock<Inline> = LazyLock::new(|| {
    str![[r#"
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]
});

const ALLOW_CACHED: LazyLock<Inline> = LazyLock::new(|| {
    str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]
});

static WARN: LazyLock<Inline> = LazyLock::new(|| {
    str![[r#"
...
[WARNING] unused variable: `x`
...
[WARNING] `foo` (bin "foo") generated 1 warning
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]
});

const DENY: LazyLock<Inline> = LazyLock::new(|| {
    str![[r#"
...
[WARNING] unused variable: `x`
...
[WARNING] `foo` (bin "foo") generated 1 warning
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[ERROR] warnings are denied by `build.warnings` configuration

"#]]
});

fn make_project_with_rustc_warning() -> Project {
    project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2021"
            "#
            ),
        )
        .file("src/main.rs", "fn main() { let x = 3; }")
        .build()
}

#[cargo_test]
fn rustc_caching_allow_first() {
    let p = make_project_with_rustc_warning();
    p.cargo("check")
        .masquerade_as_nightly_cargo(&["warnings"])
        .arg("-Zwarnings")
        .arg("--config")
        .arg("build.warnings='allow'")
        .with_stderr_data(ALLOW_CLEAN.clone())
        .run();

    p.cargo("check")
        .masquerade_as_nightly_cargo(&["warnings"])
        .arg("-Zwarnings")
        .arg("--config")
        .arg("build.warnings='deny'")
        .with_stderr_data(DENY.clone())
        .with_status(101)
        .run();
}

#[cargo_test]
fn rustc_caching_deny_first() {
    let p = make_project_with_rustc_warning();
    p.cargo("check")
        .masquerade_as_nightly_cargo(&["warnings"])
        .arg("-Zwarnings")
        .arg("--config")
        .arg("build.warnings='deny'")
        .with_stderr_data(DENY.clone())
        .with_status(101)
        .run();

    p.cargo("check")
        .masquerade_as_nightly_cargo(&["warnings"])
        .arg("-Zwarnings")
        .arg("--config")
        .arg("build.warnings='allow'")
        .with_stderr_data(ALLOW_CACHED.clone())
        .run();
}

#[cargo_test]
fn config() {
    let p = make_project_with_rustc_warning();
    p.cargo("check")
        .masquerade_as_nightly_cargo(&["warnings"])
        .arg("-Zwarnings")
        .env("CARGO_BUILD_WARNINGS", "deny")
        .with_stderr_data(DENY.clone())
        .with_status(101)
        .run();

    // CLI has precedence over env
    p.cargo("check")
        .masquerade_as_nightly_cargo(&["warnings"])
        .arg("-Zwarnings")
        .arg("--config")
        .arg("build.warnings='warn'")
        .env("CARGO_BUILD_WARNINGS", "deny")
        .with_stderr_data(WARN.clone())
        .run();
}

#[cargo_test]
fn requires_nightly() {
    // build.warnings has no effect without -Zwarnings.
    let p = make_project_with_rustc_warning();
    p.cargo("check")
        .arg("--config")
        .arg("build.warnings='deny'")
        .with_stderr_data(WARN.clone())
        .run();
}

#[cargo_test]
fn clippy() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
            "#,
        )
        .file("src/lib.rs", "use std::io;") // <-- unused import
        .build();

    p.cargo("check")
        .masquerade_as_nightly_cargo(&["warnings"])
        .arg("-Zwarnings")
        .arg("--config")
        .arg("build.warnings='deny'")
        .env("RUSTC_WORKSPACE_WRAPPER", tools::wrapped_clippy_driver())
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[WARNING] unused import: `std::io`
...
[WARNING] `foo` (lib) generated 1 warning (run `cargo clippy --fix --lib -p foo` to apply 1 suggestion)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[ERROR] warnings are denied by `build.warnings` configuration

"#]])
        .with_status(101)
        .run();
}
