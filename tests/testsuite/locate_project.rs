//! Tests for the `cargo locate-project` command.

use cargo_test_support::project;

#[cargo_test]
fn simple() {
    let p = project().build();

    p.cargo("locate-project")
        .with_json(r#"{"root": "[ROOT]/foo/Cargo.toml"}"#)
        .run();
}

#[cargo_test]
fn message_format() {
    let p = project().build();

    p.cargo("locate-project --message-format plain")
        .with_stdout("[ROOT]/foo/Cargo.toml")
        .run();

    p.cargo("locate-project --message-format json")
        .with_json(r#"{"root": "[ROOT]/foo/Cargo.toml"}"#)
        .run();

    p.cargo("locate-project --message-format cryptic")
        .with_stderr("error: invalid message format specifier: `cryptic`")
        .with_status(101)
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

    let outer_manifest = r#"{"root": "[ROOT]/foo/Cargo.toml"}"#;
    let inner_manifest = r#"{"root": "[ROOT]/foo/inner/Cargo.toml"}"#;

    p.cargo("locate-project").with_json(outer_manifest).run();

    p.cargo("locate-project")
        .cwd("inner")
        .with_json(inner_manifest)
        .run();

    p.cargo("locate-project --workspace")
        .with_json(outer_manifest)
        .run();

    p.cargo("locate-project --workspace")
        .cwd("inner")
        .with_json(outer_manifest)
        .run();
}
