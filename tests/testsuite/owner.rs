//! Tests for the `cargo owner` command.

use std::fs;

use cargo_test_support::paths::CargoPathExt;
use cargo_test_support::project;
use cargo_test_support::registry::{self, api_path, registry_url};

fn setup(name: &str, content: Option<&str>) {
    let dir = api_path().join(format!("api/v1/crates/{}", name));
    dir.mkdir_p();
    match content {
        Some(body) => {
            fs::write(dir.join("owners"), body).unwrap();
        }
        None => {}
    }
}

#[cargo_test]
fn simple_list() {
    registry::init();
    let content = r#"{
        "users": [
            {
                "id": 70,
                "login": "github:rust-lang:core",
                "name": "Core"
            }
        ]
    }"#;
    setup("foo", Some(content));

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

#[cargo_test]
fn simple_add() {
    registry::init();
    setup("foo", None);

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

    p.cargo("owner -a username --index")
        .arg(registry_url().to_string())
        .with_status(101)
        .with_stderr(
            "    Updating `[..]` index
error: failed to invite owners to crate foo: EOF while parsing a value at line 1 column 0",
        )
        .run();
}

#[cargo_test]
fn simple_remove() {
    registry::init();
    setup("foo", None);

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

    p.cargo("owner -r username --index")
        .arg(registry_url().to_string())
        .with_status(101)
        .with_stderr(
            "    Updating `[..]` index
       Owner removing [\"username\"] from crate foo
error: failed to remove owners from crate foo

Caused by:
  EOF while parsing a value at line 1 column 0",
        )
        .run();
}
