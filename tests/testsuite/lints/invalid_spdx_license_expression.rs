use crate::prelude::*;
use cargo_test_support::project;
use cargo_test_support::str;

#[cargo_test]
fn invalid_slash_operator() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
[package]
name = "foo"
version = "0.1.0"
edition = "2024"
license = "MIT / Apache-2.0"
"#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[WARNING] invalid SPDX license expression: `MIT / Apache-2.0`
 --> Cargo.toml:6:16
  |
6 | license = "MIT / Apache-2.0"
  |                ------------ invalid character(s)
  |
  = [NOTE] `cargo::invalid_spdx_license_expression` is set to `warn` by default
  = [HELP] see https://spdx.org/licenses/ for valid SPDX license expressions
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn invalid_lowercase_operators() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
[package]
name = "foo"
version = "0.1.0"
edition = "2024"
license = "GPL-3.0 with exception"
"#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[WARNING] invalid SPDX license expression: `GPL-3.0 with exception`
 --> Cargo.toml:6:20
  |
6 | license = "GPL-3.0 with exception"
  |                    ---- unknown term
  |
  = [NOTE] `cargo::invalid_spdx_license_expression` is set to `warn` by default
  = [HELP] see https://spdx.org/licenses/ for valid SPDX license expressions
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn invalid_deprecated_plus() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
[package]
name = "foo"
version = "0.1.0"
edition = "2024"
license = "GPL-3.0+"
"#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[WARNING] invalid SPDX license expression: `GPL-3.0+`
 --> Cargo.toml:6:19
  |
6 | license = "GPL-3.0+"
  |                   - a GNU license was followed by a `+`
  |
  = [NOTE] `cargo::invalid_spdx_license_expression` is set to `warn` by default
  = [HELP] see https://spdx.org/licenses/ for valid SPDX license expressions
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn invalid_malformed() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
[package]
name = "foo"
version = "0.1.0"
edition = "2024"
license = "MIT OR (Apache-2.0"
"#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[WARNING] invalid SPDX license expression: `MIT OR (Apache-2.0`
 --> Cargo.toml:6:19
  |
6 | license = "MIT OR (Apache-2.0"
  |                   - unclosed parens
  |
  = [NOTE] `cargo::invalid_spdx_license_expression` is set to `warn` by default
  = [HELP] see https://spdx.org/licenses/ for valid SPDX license expressions
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn valid() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
[package]
name = "foo"
version = "0.1.0"
edition = "2024"
license = "MIT OR Apache-2.0"
"#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn valid_complex() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
[package]
name = "foo"
version = "0.1.0"
edition = "2024"
license = "(MIT OR Apache-2.0) AND GPL-3.0-or-later WITH Classpath-exception-2.0"
"#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn no_license_field() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
[package]
name = "foo"
version = "0.1.0"
edition = "2024"
"#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn non_default_lint_level() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
[package]
name = "foo"
version = "0.1.0"
edition = "2024"
license = "MIT / Apache-2.0"

[lints.cargo]
invalid_spdx_license_expression = "deny"
"#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] invalid SPDX license expression: `MIT / Apache-2.0`
 --> Cargo.toml:6:16
  |
6 | license = "MIT / Apache-2.0"
  |                ^^^^^^^^^^^^ invalid character(s)
  |
  = [NOTE] `cargo::invalid_spdx_license_expression` is set to `deny` in `[lints]`
  = [HELP] see https://spdx.org/licenses/ for valid SPDX license expressions

"#]])
        .run();
}

#[cargo_test]
fn inherited_license() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
[workspace]
members = ["foo"]
resolver = "2"
[workspace.package]
license = "MIT / Apache-2.0"
"#,
        )
        .file(
            "foo/Cargo.toml",
            r#"
[package]
name = "foo"
version = "0.1.0"
edition = "2024"
license.workspace = true
"#,
        )
        .file("foo/src/lib.rs", "")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[WARNING] invalid SPDX license expression: `MIT / Apache-2.0`
 --> Cargo.toml:6:16
  |
6 | license = "MIT / Apache-2.0"
  |                ------------ invalid character(s)
  |
[NOTE] the `package.license` field was inherited
 --> foo/Cargo.toml:6:9
  |
6 | license.workspace = true
  |         ----------------
  |
  = [NOTE] `cargo::invalid_spdx_license_expression` is set to `warn` by default
  = [HELP] see https://spdx.org/licenses/ for valid SPDX license expressions
[CHECKING] foo v0.1.0 ([ROOT]/foo/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test(nightly, reason = "future edition is always unstable")]
fn edition_future_deny_by_default() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
cargo-features = ["unstable-editions"]

[package]
name = "foo"
version = "0.1.0"
edition = "future"
license = "MIT / Apache-2.0"
"#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints", "unstable-editions"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] invalid SPDX license expression: `MIT / Apache-2.0`
 --> Cargo.toml:8:16
  |
8 | license = "MIT / Apache-2.0"
  |                ^^^^^^^^^^^^ invalid character(s)
  |
  = [NOTE] `cargo::invalid_spdx_license_expression` is set to `deny` in edition future
  = [HELP] see https://spdx.org/licenses/ for valid SPDX license expressions

"#]])
        .run();
}
