//! Tests for overriding warning behavior using `build.warnings` config option.

use crate::prelude::*;
use crate::utils::tools;
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
 --> src/main.rs:1:17
  |
1 | fn main() { let x = 3; }
  |                 ^ [HELP] if this is intentional, prefix it with an underscore: `_x`
  |
  = [NOTE] `#[warn(unused_variables)]` [..]on by default

[WARNING] `foo` (bin "foo") generated 1 warning[..]
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
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
 --> src/main.rs:1:17
  |
1 | fn main() { let x = 3; }
  |                 ^ [HELP] if this is intentional, prefix it with an underscore: `_x`
  |
  = [NOTE] `#[warn(unused_variables)]` [..]on by default

[WARNING] `foo` (bin "foo") generated 1 warning[..]
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
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
fn config() {
    let p = make_project_with_rustc_warning();
    p.cargo("check")
        .masquerade_as_nightly_cargo(&["warnings"])
        .arg("-Zwarnings")
        .env("CARGO_BUILD_WARNINGS", "deny")
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[WARNING] unused variable: `x`
 --> src/main.rs:1:17
  |
1 | fn main() { let x = 3; }
  |                 ^ [HELP] if this is intentional, prefix it with an underscore: `_x`
  |
  = [NOTE] `#[warn(unused_variables)]` [..]on by default

[WARNING] `foo` (bin "foo") generated 1 warning[..]
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
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
 --> src/main.rs:1:17
  |
1 | fn main() { let x = 3; }
  |                 ^ [HELP] if this is intentional, prefix it with an underscore: `_x`
  |
  = [NOTE] `#[warn(unused_variables)]` [..]on by default

[WARNING] `foo` (bin "foo") generated 1 warning[..]
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
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
 --> src/main.rs:1:17
  |
1 | fn main() { let x = 3; }
  |                 ^ [HELP] if this is intentional, prefix it with an underscore: `_x`
  |
  = [NOTE] `#[warn(unused_variables)]` [..]on by default

[WARNING] `foo` (bin "foo") generated 1 warning[..]
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
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
fn hard_warning_deny() {
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
        .file("src/main.rs", "fn main() {}")
        .build();
    p.cargo("rustc")
        .masquerade_as_nightly_cargo(&["warnings"])
        .arg("-Zwarnings")
        .arg("--config")
        .arg("build.warnings='deny'")
        .arg("--")
        .arg("-ox.rs")
        .with_stderr_data(str![[r#"
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
                edition = "2021"
            "#
            ),
        )
        .file("src/main.rs", "fn main() {}")
        .build();
    p.cargo("rustc")
        .masquerade_as_nightly_cargo(&["warnings"])
        .arg("-Zwarnings")
        .arg("--config")
        .arg("build.warnings='allow'")
        .arg("--")
        .arg("-ox.rs")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .with_status(0)
        .run();
}
