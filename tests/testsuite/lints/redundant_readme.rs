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
[WARNING] explicit `package.readme` can be inferred
 --> Cargo.toml:7:1
  |
7 | readme = "README.md"
  | ^^^^^^^^^^^^^^^^^^^^
  |
  = [NOTE] `cargo::redundant_readme` is set to `warn` in `[lints]`
[HELP] consider removing `package.readme`
  |
7 - readme = "README.md"
  |
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
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn inherited() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
[workspace.package]
readme = "README.md"

[package]
name = "foo"
version = "0.0.1"
edition = "2015"
authors = []
readme.workspace = true

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
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}
