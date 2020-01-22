//! Tests for the `cargo yank` command.

use std::fs;

use cargo_test_support::paths::CargoPathExt;
use cargo_test_support::project;
use cargo_test_support::registry::{self, api_path, registry_url};

fn setup(name: &str, version: &str) {
    let dir = api_path().join(format!("api/v1/crates/{}/{}", name, version));
    dir.mkdir_p();
    fs::write(dir.join("yank"), r#"{"ok": true}"#).unwrap();
}

#[cargo_test]
fn simple() {
    registry::init();
    setup("foo", "0.0.1");

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
            license = "MIT"
            description = "foo"
        "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("yank --vers 0.0.1 --index")
        .arg(registry_url().to_string())
        .run();

    p.cargo("yank --undo --vers 0.0.1 --index")
        .arg(registry_url().to_string())
        .with_status(101)
        .with_stderr(
            "    Updating `[..]` index
      Unyank foo:0.0.1
error: failed to undo a yank

Caused by:
  EOF while parsing a value at line 1 column 0",
        )
        .run();
}
