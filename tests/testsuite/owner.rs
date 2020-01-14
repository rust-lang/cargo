//! Tests for the `cargo owner` command.

use std::fs;

use cargo_test_support::paths::CargoPathExt;
use cargo_test_support::project;
use cargo_test_support::registry::{self, api_path, registry_url};

fn setup(name: &str) {
    let dir = api_path().join(format!("api/v1/crates/{}", name));
    dir.mkdir_p();
    fs::write(
        dir.join("owners"),
        r#"{
        "users": [
            {
                "id": 70,
                "login": "github:rust-lang:core",
                "name": "Core"
            }
        ]
    }"#,
    )
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
