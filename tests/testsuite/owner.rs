//! Tests for the `cargo owner` command.

use std::fs;

use cargo_test_support::paths::CargoPathExt;
use cargo_test_support::project;
use cargo_test_support::registry::{self, api_path};

fn setup(name: &str, content: Option<&str>) {
    let dir = api_path().join(format!("api/v1/crates/{}", name));
    dir.mkdir_p();
    if let Some(body) = content {
        fs::write(dir.join("owners"), body).unwrap();
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
            },
            {
                "id": 123,
                "login": "octocat"
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

    p.cargo("owner -l --token sekrit")
        .with_stdout(
            "\
github:rust-lang:core (Core)
octocat
",
        )
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

    p.cargo("owner -a username --token sekrit")
        .with_status(101)
        .with_stderr(
            "    Updating `[..]` index
error: failed to invite owners to crate `foo` on registry at file://[..]

Caused by:
  EOF while parsing a value at line 1 column 0",
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

    p.cargo("owner -r username --token sekrit")
        .with_status(101)
        .with_stderr(
            "    Updating `[..]` index
       Owner removing [\"username\"] from crate foo
error: failed to remove owners from crate `foo` on registry at file://[..]

Caused by:
  EOF while parsing a value at line 1 column 0",
        )
        .run();
}
