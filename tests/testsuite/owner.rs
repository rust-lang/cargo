//! Tests for the `cargo owner` command.

use std::fs::{self, File};
use std::io::prelude::*;

use cargo_test_support::project;
use cargo_test_support::registry::{self, api_path, registry_url};

fn setup(name: &str) {
    fs::create_dir_all(&api_path().join(format!("api/v1/crates/{}", name))).unwrap();

    let dest = api_path().join(format!("api/v1/crates/{}/owners", name));

    let content = r#"{
        "users": [
            {
                "id": 70,
                "login": "github:rust-lang:core",
                "name": "Core"
            }
        ]
    }"#;

    File::create(&dest)
        .unwrap()
        .write_all(content.as_bytes())
        .unwrap();
}

#[cargo_test]
fn simple_list() {
    registry::init();
    setup("foo");

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

    p.cargo("owner -l --index")
        .arg(registry_url().to_string())
        .run();
}
