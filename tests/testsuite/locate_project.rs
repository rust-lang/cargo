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
fn format() {
    let p = project().build();
    let root_manifest_path = p.root().join("Cargo.toml");
    let root_str = root_manifest_path.to_str().unwrap();

    p.cargo("locate-project --format {root}")
        .with_stdout(root_str)
        .run();

    p.cargo("locate-project -f{root}")
        .with_stdout(root_str)
        .run();

    p.cargo("locate-project --format root={root}")
        .with_stdout(format!("root={}", root_str))
        .run();

    p.cargo("locate-project --format {toor}")
        .with_stderr(
            "\
[ERROR] locate-project format `{toor}` not valid

Caused by:
  unsupported pattern `toor`
",
        )
        .with_status(101)
        .run();
}
