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
fn no_subcommand_return_help() {
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

    p.cargo("owner")
        .with_status(1)
        .with_stderr(
            "\
Manage the owners of a crate on the registry

Usage: cargo owner add    <OWNER_NAME> [CRATE_NAME] [OPTIONS]
       cargo owner remove <OWNER_NAME> [CRATE_NAME] [OPTIONS]
       cargo owner list   [CRATE_NAME] [OPTIONS]

Commands:
  add     Name of a user or team to invite as an owner
  remove  Name of a user or team to remove as an owner
  list    List owners of a crate

Options:
  -q, --quiet                Do not print cargo log messages
  -v, --verbose...           Use verbose output (-vv very verbose/build.rs output)
      --color <WHEN>         Coloring: auto, always, never
      --frozen               Require Cargo.lock and cache are up to date
      --index <INDEX>        Registry index to modify owners for
      --locked               Require Cargo.lock is up to date
      --token <TOKEN>        API token to use when authenticating
      --offline              Run without accessing the network
      --registry <REGISTRY>  Registry to use
      --config <KEY=VALUE>   Override a configuration value
  -Z <FLAG>                  Unstable (nightly-only) flags to Cargo, see 'cargo -Z help' for details
  -h, --help                 Print help

Run `cargo help owner` for more detailed information.",
        )
        .run();
}

#[cargo_test]
fn add_no_ownername_return_error() {
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

    p.cargo("owner add")
        .with_status(1)
        .with_stderr(
            "\
error: the following required arguments were not provided:
  <OWNER_NAME>

Usage: cargo owner add <OWNER_NAME> [CRATE_NAME]

For more information, try '--help'.",
        )
        .run();
}

#[cargo_test]
fn remove_no_ownername_return_error() {
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

    p.cargo("owner remove")
        .with_status(1)
        .with_stderr(
            "\
error: the following required arguments were not provided:
  <OWNER_NAME>

Usage: cargo owner remove <OWNER_NAME> [CRATE_NAME]

For more information, try '--help'.",
        )
        .run();
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

    let commands = ["-l", "--list", "list"];
    for command in commands.iter() {
        p.cargo(&format!("owner {}", command))
            .replace_crates_io(registry.index_url())
            .with_stdout(
                "\
github:rust-lang:core (Core)
octocat
",
            )
            .run();
    }
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

    let commands = ["-a", "--add", "add"];
    for command in commands.iter() {
        p.cargo(&format!("owner {} username", command))
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
}

#[cargo_test]
fn simple_flag_add_with_asymmetric() {
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
    p.cargo("owner --add username")
        .arg("-Zregistry-auth")
        .masquerade_as_nightly_cargo(&["registry-auth"])
        .replace_crates_io(registry.index_url())
        .with_status(0)
        .run();
}

#[cargo_test]
fn simple_subcommand_add_with_asymmetric() {
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
    p.cargo("owner add username")
        .arg("-Zregistry-auth")
        .masquerade_as_nightly_cargo(&["registry-auth"])
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

    let commands = ["remove", "--remove", "-r"];
    for command in commands.iter() {
        p.cargo(&format!("owner {} username", command))
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
}

#[cargo_test]
fn simple_flag_remove_with_asymmetric() {
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
    p.cargo("owner --remove username")
        .arg("-Zregistry-auth")
        .replace_crates_io(registry.index_url())
        .masquerade_as_nightly_cargo(&["registry-auth"])
        .with_status(0)
        .run();
}

#[cargo_test]
fn simple_subcommand_remove_with_asymmetric() {
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
    p.cargo("owner remove username")
        .arg("-Zregistry-auth")
        .replace_crates_io(registry.index_url())
        .masquerade_as_nightly_cargo(&["registry-auth"])
        .with_status(0)
        .run();
}
