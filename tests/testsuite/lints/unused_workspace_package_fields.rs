use crate::prelude::*;
use cargo_test_support::project;
use cargo_test_support::str;

#[cargo_test]
fn unused() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
[workspace]
members = ["bar"]

[workspace.package]
documentation = "docs.rs/foo"
homepage = "bar.rs"
rust-version = "1.0"
unknown = "foo"

[workspace.lints.cargo]
unused_workspace_package_fields = "warn"

[package]
name = "foo"
version = "0.0.1"
edition = "2015"
documentation.workspace = true

[lints]
workspace = true
"#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "bar/Cargo.toml",
            r#"
[package]
name = "bar"
version = "0.0.1"
edition = "2015"
homepage.workspace = true

[lints]
workspace = true
"#,
        )
        .file("bar/src/main.rs", "fn main() {}")
        .build();

    p.cargo("check -Zcargo-lints")
        .masquerade_as_nightly_cargo(&["cargo-lints"])
        .with_stderr_data(str![[r#"
[WARNING] unused field in `workspace.package`
 --> Cargo.toml:8:1
  |
8 | rust-version = "1.0"
  | ^^^^^^^^^^^^
  |
  = [NOTE] `cargo::unused_workspace_package_fields` is set to `warn` by default
[HELP] consider removing the unused field
  |
8 - rust-version = "1.0"
  |
[WARNING] unused field in `workspace.package`
 --> Cargo.toml:9:1
  |
9 | unknown = "foo"
  | ^^^^^^^
  |
[HELP] consider removing the unused field
  |
9 - unknown = "foo"
  |
[WARNING] [ROOT]/foo/Cargo.toml: unused manifest key: workspace.package.unknown
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}
