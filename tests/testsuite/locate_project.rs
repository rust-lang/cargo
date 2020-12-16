//! Tests for the `cargo locate-project` command.

use cargo_test_support::project;

#[cargo_test]
fn simple() {
    let p = project().build();
    let root_manifest_path = p.root().join("Cargo.toml");

    p.cargo("locate-project")
        .with_stdout(format!(
            r#"{{"root":"{}"}}"#,
            root_manifest_path.to_str().unwrap()
        ))
        .run();
}

#[cargo_test]
fn message_format() {
    let p = project().build();
    let root_manifest_path = p.root().join("Cargo.toml");
    let root_str = root_manifest_path.to_str().unwrap();

    p.cargo("locate-project --message-format plain")
        .with_stdout(root_str)
        .run();

    p.cargo("locate-project --message-format json")
        .with_stdout(format!(r#"{{"root":"{}"}}"#, root_str))
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

    let outer_manifest = format!(
        r#"{{"root":"{}"}}"#,
        p.root().join("Cargo.toml").to_str().unwrap(),
    );
    let inner_manifest = format!(
        r#"{{"root":"{}"}}"#,
        p.root().join("inner").join("Cargo.toml").to_str().unwrap(),
    );

    p.cargo("locate-project").with_stdout(&outer_manifest).run();

    p.cargo("locate-project")
        .cwd("inner")
        .with_stdout(&inner_manifest)
        .run();

    p.cargo("locate-project --workspace")
        .with_stdout(&outer_manifest)
        .run();

    p.cargo("locate-project --workspace")
        .cwd("inner")
        .with_stdout(&outer_manifest)
        .run();
}
