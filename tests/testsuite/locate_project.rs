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
