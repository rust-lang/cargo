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
