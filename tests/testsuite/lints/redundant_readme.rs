use crate::prelude::*;
use cargo_test_support::project;
use cargo_test_support::str;

#[cargo_test]
fn explicit_readme() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
[package]
name = "foo"
version = "0.0.1"
edition = "2015"
authors = []
readme = "README.md"

[lints.cargo]
redundant_readme = "warn"
"#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("README.md", "")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[WARNING] unknown lint: `redundant_readme`
  --> Cargo.toml:10:1
   |
10 | redundant_readme = "warn"
   | ^^^^^^^^^^^^^^^^
   |
   = [NOTE] `cargo::unknown_lints` is set to `warn` by default
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn implicit_readme() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
[package]
name = "foo"
version = "0.0.1"
edition = "2015"
authors = []

[lints.cargo]
redundant_readme = "warn"
"#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("README.md", "")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[WARNING] unknown lint: `redundant_readme`
 --> Cargo.toml:9:1
  |
9 | redundant_readme = "warn"
  | ^^^^^^^^^^^^^^^^
  |
  = [NOTE] `cargo::unknown_lints` is set to `warn` by default
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn custom_name() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
[package]
name = "foo"
version = "0.0.1"
edition = "2015"
authors = []
readme = "FOO.md"

[lints.cargo]
redundant_readme = "warn"
"#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("FOO.md", "")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[WARNING] unknown lint: `redundant_readme`
  --> Cargo.toml:10:1
   |
10 | redundant_readme = "warn"
   | ^^^^^^^^^^^^^^^^
   |
   = [NOTE] `cargo::unknown_lints` is set to `warn` by default
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn custom_location() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
[package]
name = "foo"
version = "0.0.1"
edition = "2015"
authors = []
readme = "src/README.md"

[lints.cargo]
redundant_readme = "warn"
"#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("src/README.md", "")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[WARNING] unknown lint: `redundant_readme`
  --> Cargo.toml:10:1
   |
10 | redundant_readme = "warn"
   | ^^^^^^^^^^^^^^^^
   |
   = [NOTE] `cargo::unknown_lints` is set to `warn` by default
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}
