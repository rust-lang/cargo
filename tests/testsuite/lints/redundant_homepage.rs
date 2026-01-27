use crate::prelude::*;
use cargo_test_support::project;
use cargo_test_support::str;

#[cargo_test]
fn with_repo() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
[package]
name = "cargo"
version = "0.0.1"
edition = "2015"
repository = "https://github.com/rust-lang/cargo/"
homepage = "https://github.com/rust-lang/cargo/"

[lints.cargo]
redundant_homepage = "warn"
"#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("README.md", "")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[WARNING] `package.homepage` is redundant with another manifest field
 --> Cargo.toml:7:12
  |
6 | repository = "https://github.com/rust-lang/cargo/"
  |              -------------------------------------
7 | homepage = "https://github.com/rust-lang/cargo/"
  |            ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = [NOTE] `cargo::redundant_homepage` is set to `warn` in `[lints]`
[HELP] consider removing `package.homepage`
  |
7 - homepage = "https://github.com/rust-lang/cargo/"
  |
[CHECKING] cargo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn with_docs() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
[package]
name = "cargo"
version = "0.0.1"
edition = "2015"
documentation = "https://docs.rs/cargo/latest/cargo/"
homepage = "https://docs.rs/cargo/latest/cargo/"

[lints.cargo]
redundant_homepage = "warn"
"#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("README.md", "")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[WARNING] `package.homepage` is redundant with another manifest field
 --> Cargo.toml:7:12
  |
6 | documentation = "https://docs.rs/cargo/latest/cargo/"
  |                 -------------------------------------
7 | homepage = "https://docs.rs/cargo/latest/cargo/"
  |            ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = [NOTE] `cargo::redundant_homepage` is set to `warn` in `[lints]`
[HELP] consider removing `package.homepage`
  |
7 - homepage = "https://docs.rs/cargo/latest/cargo/"
  |
[CHECKING] cargo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}
