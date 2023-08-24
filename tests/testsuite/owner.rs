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
    let registry = registry::init();
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
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []
                license = "MIT"
                description = "foo"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("owner -l")
        .replace_crates_io(registry.index_url())
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
    let registry = registry::init();
    setup("foo", None);

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []
                license = "MIT"
                description = "foo"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("owner -a username")
        .replace_crates_io(registry.index_url())
        .with_status(101)
        .with_stderr(
            "    Updating crates.io index
error: failed to invite owners to crate `foo` on registry at file://[..]

Caused by:
  EOF while parsing a value at line 1 column 0",
        )
        .run();
}

#[cargo_test]
fn simple_add_with_asymmetric() {
    let registry = registry::RegistryBuilder::new()
        .http_api()
        .token(cargo_test_support::registry::Token::rfc_key())
        .build();
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

    // The http_api server will check that the authorization is correct.
    // If the authorization was not sent then we would get an unauthorized error.
    p.cargo("owner -a username")
        .arg("-Zasymmetric-token")
        .masquerade_as_nightly_cargo(&["asymmetric-token"])
        .replace_crates_io(registry.index_url())
        .with_status(0)
        .run();
}

#[cargo_test]
fn simple_remove() {
    let registry = registry::init();
    setup("foo", None);

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []
                license = "MIT"
                description = "foo"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("owner -r username")
        .replace_crates_io(registry.index_url())
        .with_status(101)
        .with_stderr(
            "    Updating crates.io index
       Owner removing [\"username\"] from crate foo
error: failed to remove owners from crate `foo` on registry at file://[..]

Caused by:
  EOF while parsing a value at line 1 column 0",
        )
        .run();
}

#[cargo_test]
fn simple_remove_with_asymmetric() {
    let registry = registry::RegistryBuilder::new()
        .http_api()
        .token(cargo_test_support::registry::Token::rfc_key())
        .build();
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

    // The http_api server will check that the authorization is correct.
    // If the authorization was not sent then we would get an unauthorized error.
    p.cargo("owner -r username")
        .arg("-Zasymmetric-token")
        .replace_crates_io(registry.index_url())
        .masquerade_as_nightly_cargo(&["asymmetric-token"])
        .with_status(0)
        .run();
}
