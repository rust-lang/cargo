//! Tests for the `cargo locate-project` command.

use crate::prelude::*;
use cargo_test_support::project;
use cargo_test_support::str;

#[cargo_test]
fn simple() {
    let p = project().build();

    p.cargo("locate-project")
        .with_stdout_data(
            str![[r#"
{
  "root": "[ROOT]/foo/Cargo.toml"
}
"#]]
            .is_json(),
        )
        .run();
}

#[cargo_test]
fn message_format() {
    let p = project().build();

    p.cargo("locate-project --message-format plain")
        .with_stdout_data(str![[r#"
[ROOT]/foo/Cargo.toml

"#]])
        .run();

    p.cargo("locate-project --message-format json")
        .with_stdout_data(
            str![[r#"
{
  "root": "[ROOT]/foo/Cargo.toml"
}
"#]]
            .is_json(),
        )
        .run();

    p.cargo("locate-project --message-format cryptic")
        .with_stderr_data(str![[r#"
[ERROR] invalid value 'cryptic' for '--message-format <FMT>'
  [possible values: json, plain]

For more information, try '--help'.

"#]])
        .with_status(1)
        .run();
}

#[cargo_test]
fn workspace() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "outer"
                version = "0.0.0"

                [workspace]
                members = ["inner"]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "inner/Cargo.toml",
            r#"
                [package]
                name = "inner"
                version = "0.0.0"
            "#,
        )
        .file("inner/src/lib.rs", "")
        .build();

    p.cargo("locate-project")
        .with_stdout_data(
            str![[r#"
{
  "root": "[ROOT]/foo/Cargo.toml"
}
"#]]
            .is_json(),
        )
        .run();

    p.cargo("locate-project")
        .cwd("inner")
        .with_stdout_data(
            str![[r#"
{
  "root": "[ROOT]/foo/inner/Cargo.toml"
}
"#]]
            .is_json(),
        )
        .run();

    p.cargo("locate-project --workspace")
        .with_stdout_data(
            str![[r#"
{
  "root": "[ROOT]/foo/Cargo.toml"
}
"#]]
            .is_json(),
        )
        .run();

    p.cargo("locate-project --workspace")
        .cwd("inner")
        .with_stdout_data(
            str![[r#"
{
  "root": "[ROOT]/foo/Cargo.toml"
}
"#]]
            .is_json(),
        )
        .run();
}

#[cargo_test]
fn workspace_missing_member() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "root"
                version = "0.0.0"

                [workspace]
                members = ["missing_member"]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    // Current behavior: fails because args.workspace() loads all members
    //FIXME: After fix, should succeed by only finding workspace root
    p.cargo("locate-project --workspace")
        .with_stderr_data(str![[r#"
[ERROR] failed to load manifest for workspace member `[ROOT]/foo/missing_member`
...
"#]])
        .with_status(101)
        .run();
}

#[cargo_test]
fn workspace_nested_with_explicit_pointer() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["nested"]
            "#,
        )
        .file(
            "nested/Cargo.toml",
            r#"
                [package]
                name = "nested"
                version = "0.0.0"
                workspace = ".."
            "#,
        )
        .file("nested/src/lib.rs", "")
        .build();

    p.cargo("locate-project --workspace")
        .cwd("nested")
        .with_stdout_data(
            str![[r#"
{
  "root": "[ROOT]/foo/Cargo.toml"
}
"#]]
            .is_json(),
        )
        .run();
}
