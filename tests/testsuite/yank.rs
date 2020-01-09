//! Tests for the `cargo yank` command.

use std::fs::{self, File};
use std::io::prelude::*;

use cargo_test_support::project;
use cargo_test_support::registry::{self, api_path, registry_url};

fn setup(name: &str, version: &str) {
    fs::create_dir_all(&api_path().join(format!("api/v1/crates/{}/{}", name, version))).unwrap();

    let dest = api_path().join(format!("api/v1/crates/{}/{}/yank", name, version));

    let content = r#"{
        "ok": true
    }"#;

    File::create(&dest)
        .unwrap()
        .write_all(content.as_bytes())
        .unwrap();
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
}
