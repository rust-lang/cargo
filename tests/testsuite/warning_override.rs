//! Tests for overriding warning behavior using `build.warnings` config option.

use crate::prelude::*;
use crate::utils::tools;
use cargo_test_support::registry::Package;
use cargo_test_support::{Project, cargo_test, project, str};

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
fn requires_nightly() {
    // build.warnings has no effect without -Zwarnings.
    let p = make_project_with_rustc_warning();
    p.cargo("check")
        .arg("--config")
        .arg("build.warnings='deny'")
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[WARNING] unused variable: `x`
...
[WARNING] `foo` (bin "foo") generated 1 warning[..]
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn always_show_error_diags() {
    let p = make_project_with_rustc_warning();
    p.cargo("check")
        .masquerade_as_nightly_cargo(&["warnings"])
        .env("RUSTFLAGS", "-Dunused_variables")
        .arg("-Zwarnings")
        .arg("--config")
        .arg("build.warnings='allow'")
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[ERROR] unused variable: `x`
...
[ERROR] could not compile `foo` (bin "foo") due to 1 previous error

"#]])
        .with_status(101)
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
[ERROR] `foo` (lib) generated 1 warning (run `cargo clippy --fix --lib -p foo` to apply 1 suggestion)
[ERROR] warnings are denied by `build.warnings` configuration

"#]])
        .with_status(101)
        .run();
}

#[cargo_test]
fn config() {
    let p = make_project_with_rustc_warning();
    p.cargo("check")
        .masquerade_as_nightly_cargo(&["warnings"])
        .arg("-Zwarnings")
        .env("CARGO_BUILD_WARNINGS", "deny")
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[WARNING] unused variable: `x`
...
[ERROR] `foo` (bin "foo") generated 1 warning[..]
[ERROR] warnings are denied by `build.warnings` configuration

"#]])
        .with_status(101)
        .run();

    // CLI has precedence over env
    p.cargo("check")
        .masquerade_as_nightly_cargo(&["warnings"])
        .arg("-Zwarnings")
        .arg("--config")
        .arg("build.warnings='warn'")
        .env("CARGO_BUILD_WARNINGS", "deny")
        .with_stderr_data(str![[r#"
[WARNING] unused variable: `x`
...
[WARNING] `foo` (bin "foo") generated 1 warning[..]
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn unknown_value() {
    let p = make_project_with_rustc_warning();
    p.cargo("check")
        .masquerade_as_nightly_cargo(&["warnings"])
        .arg("-Zwarnings")
        .arg("--config")
        .arg("build.warnings='forbid'")
        .with_stderr_data(str![[r#"
[ERROR] error in --config cli option: could not load config key `build.warnings`

Caused by:
  unknown variant `forbid`, expected one of `warn`, `allow`, `deny`

"#]])
        .with_status(101)
        .run();
}

#[cargo_test]
fn keep_going() {
    let p = project()
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
        .file("build.rs", "fn main() { let x = 3; }")
        .file("src/main.rs", "fn main() { let y = 4; }")
        .build();

    p.cargo("build")
        .masquerade_as_nightly_cargo(&["warnings"])
        .arg("-Zwarnings")
        .arg("--config")
        .arg("build.warnings='deny'")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[WARNING] unused variable: `x`
...
[ERROR] `foo` (build script) generated 1 warning
[ERROR] warnings are denied by `build.warnings` configuration

"#]])
        .with_status(101)
        .run();
    // No uplifting
    assert!(!p.bin("foo").is_file());

    p.cargo("build --keep-going")
        .masquerade_as_nightly_cargo(&["warnings"])
        .arg("-Zwarnings")
        .arg("--config")
        .arg("build.warnings='deny'")
        .with_stderr_data(str![[r#"
[WARNING] unused variable: `x`
...
[ERROR] `foo` (build script) generated 1 warning
[COMPILING] foo v0.0.1 ([ROOT]/foo)
...
[ERROR] `foo` (bin "foo") generated 1 warning (run `cargo fix --bin "foo" -p foo` to apply 1 suggestion)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[ERROR] warnings are denied by `build.warnings` configuration

"#]])
        .with_status(101)
        .run();
    // Uplifting happened despite the error
    assert!(p.bin("foo").is_file());
}

#[cargo_test]
fn rustc_caching_allow_first() {
    let p = make_project_with_rustc_warning();
    p.cargo("check")
        .masquerade_as_nightly_cargo(&["warnings"])
        .arg("-Zwarnings")
        .arg("--config")
        .arg("build.warnings='allow'")
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("check")
        .masquerade_as_nightly_cargo(&["warnings"])
        .arg("-Zwarnings")
        .arg("--config")
        .arg("build.warnings='deny'")
        .with_stderr_data(str![[r#"
[WARNING] unused variable: `x`
...
[ERROR] `foo` (bin "foo") generated 1 warning[..]
[ERROR] warnings are denied by `build.warnings` configuration

"#]])
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
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[WARNING] unused variable: `x`
...
[ERROR] `foo` (bin "foo") generated 1 warning[..]
[ERROR] warnings are denied by `build.warnings` configuration

"#]])
        .with_status(101)
        .run();

    p.cargo("check")
        .masquerade_as_nightly_cargo(&["warnings"])
        .arg("-Zwarnings")
        .arg("--config")
        .arg("build.warnings='allow'")
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn hard_warning_deny() {
    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                [package]
                name = "foo"
                version = "0.0.1"
            "#
            ),
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    // Baseline behavior
    p.cargo("rustc")
        .arg("--")
        .arg("-ox.rs")
        .with_stderr_data(str![[r#"
[WARNING] `package.edition` is unspecified, defaulting to `2015` while the latest is `[..]`
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[WARNING] [..]

[WARNING] [..]

[WARNING] `foo` (bin "foo") generated 2 warnings
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    // Compare with `RUSTFLAGS
    p.cargo("rustc")
        .env("RUSTFLAGS", "-Dwarnings")
        .arg("--")
        .arg("-ox.rs")
        .with_stderr_data(str![[r#"
[WARNING] `package.edition` is unspecified, defaulting to `2015` while the latest is `[..]`
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[WARNING] [..]

[WARNING] [..]

[WARNING] `foo` (bin "foo") generated 2 warnings
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    // Behavior under test
    p.cargo("rustc")
        .masquerade_as_nightly_cargo(&["warnings"])
        .arg("-Zwarnings")
        .arg("--config")
        .arg("build.warnings='deny'")
        .arg("--")
        .arg("-ox.rs")
        .with_stderr_data(str![[r#"
[WARNING] `package.edition` is unspecified, defaulting to `2015` while the latest is `[..]`
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[WARNING] [..]

[WARNING] [..]

[WARNING] `foo` (bin "foo") generated 2 warnings
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn hard_warning_allow() {
    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                [package]
                name = "foo"
                version = "0.0.1"
            "#
            ),
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    // Baseline behavior
    p.cargo("rustc")
        .arg("--")
        .arg("-ox.rs")
        .with_stderr_data(str![[r#"
[WARNING] `package.edition` is unspecified, defaulting to `2015` while the latest is `[..]`
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[WARNING] [..]

[WARNING] [..]

[WARNING] `foo` (bin "foo") generated 2 warnings
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .with_status(0)
        .run();

    // Compare with `RUSTFLAGS
    p.cargo("rustc")
        .env("RUSTFLAGS", "-Awarnings")
        .arg("--")
        .arg("-ox.rs")
        .with_stderr_data(str![[r#"
[WARNING] `package.edition` is unspecified, defaulting to `2015` while the latest is `[..]`
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .with_status(0)
        .run();

    // Behavior under test
    p.cargo("rustc")
        .masquerade_as_nightly_cargo(&["warnings"])
        .arg("-Zwarnings")
        .arg("--config")
        .arg("build.warnings='allow'")
        .arg("--")
        .arg("-ox.rs")
        .with_stderr_data(str![[r#"
[WARNING] `package.edition` is unspecified, defaulting to `2015` while the latest is `[..]`
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn cap_lints_deny() {
    Package::new("has_warning", "1.0.0")
        .file("src/lib.rs", "pub fn foo() { let x = 3; }")
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2021"

                [dependencies]
                has_warning = "1"
            "#
            ),
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    // Baseline behavior
    p.cargo("check -vv")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] has_warning v1.0.0 (registry `dummy-registry`)
[CHECKING] has_warning v1.0.0
[RUNNING] [..]
[WARNING] unused variable: `x`
...
[WARNING] `has_warning` (lib) generated 1 warning
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] [..]
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    // Compare with `RUSTFLAGS
    p.cargo("check -vv")
        .env("RUSTFLAGS", "-Dwarnings")
        .with_stderr_data(str![[r#"
[CHECKING] has_warning v1.0.0
[RUNNING] [..]
[WARNING] unused variable: `x`
...
[WARNING] `has_warning` (lib) generated 1 warning
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] [..]
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    // Behavior under test
    p.cargo("check -vv")
        .masquerade_as_nightly_cargo(&["warnings"])
        .arg("-Zwarnings")
        .arg("--config")
        .arg("build.warnings='deny'")
        .with_stderr_data(str![[r#"
[FRESH] has_warning v1.0.0
[WARNING] unused variable: `x`
...
[WARNING] `has_warning` (lib) generated 1 warning
[FRESH] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn cap_lints_allow() {
    Package::new("has_warning", "1.0.0")
        .file("src/lib.rs", "pub fn foo() { let x = 3; }")
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2021"

                [dependencies]
                has_warning = "1"
            "#
            ),
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    // Baseline behavior
    p.cargo("check -vv")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] has_warning v1.0.0 (registry `dummy-registry`)
[CHECKING] has_warning v1.0.0
[RUNNING] [..]
[WARNING] unused variable: `x`
...
[WARNING] `has_warning` (lib) generated 1 warning
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] [..]
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    // Compare with `RUSTFLAGS
    p.cargo("check -vv")
        .env("RUSTFLAGS", "-Awarnings")
        .with_stderr_data(str![[r#"
[CHECKING] has_warning v1.0.0
[RUNNING] [..]
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] [..]
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    // Behavior under test
    p.cargo("check -vv")
        .masquerade_as_nightly_cargo(&["warnings"])
        .arg("-Zwarnings")
        .arg("--config")
        .arg("build.warnings='allow'")
        .with_stderr_data(str![[r#"
[FRESH] has_warning v1.0.0
[FRESH] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}
