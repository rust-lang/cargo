use std::fs::{self, File};
use std::io::prelude::*;

use crate::support::cargo_process;
use crate::support::install::cargo_home;
use crate::support::registry::{self, registry_url};
use cargo::core::Shell;
use cargo::util::config::Config;
use toml;

const TOKEN: &str = "test-token";
const ORIGINAL_TOKEN: &str = "api-token";

fn setup_new_credentials() {
    let config = cargo_home().join("credentials");
    t!(fs::create_dir_all(config.parent().unwrap()));
    t!(t!(File::create(&config))
        .write_all(format!(r#"token = "{token}""#, token = ORIGINAL_TOKEN).as_bytes()));
}

fn check_token(expected_token: &str, registry: Option<&str>) -> bool {
    let credentials = cargo_home().join("credentials");
    assert!(credentials.is_file());

    let mut contents = String::new();
    File::open(&credentials)
        .unwrap()
        .read_to_string(&mut contents)
        .unwrap();
    let toml: toml::Value = contents.parse().unwrap();

    let token = match (registry, toml) {
        // A registry has been provided, so check that the token exists in a
        // table for the registry.
        (Some(registry), toml::Value::Table(table)) => table
            .get("registries")
            .and_then(|registries_table| registries_table.get(registry))
            .and_then(|registry_table| match registry_table.get("token") {
                Some(&toml::Value::String(ref token)) => Some(token.as_str().to_string()),
                _ => None,
            }),
        // There is no registry provided, so check the global token instead.
        (None, toml::Value::Table(table)) => table
            .get("registry")
            .and_then(|registry_table| registry_table.get("token"))
            .and_then(|v| match v {
                toml::Value::String(ref token) => Some(token.as_str().to_string()),
                _ => None,
            }),
        _ => None,
    };

    if let Some(token_val) = token {
        token_val == expected_token
    } else {
        false
    }
}

#[cargo_test]
fn login_with_old_credentials() {
    registry::init();

    cargo_process("login --host")
        .arg(registry_url().to_string())
        .arg(TOKEN)
        .run();

    // Ensure that we get the new token for the registry
    assert!(check_token(TOKEN, None));
}

#[cargo_test]
fn login_with_new_credentials() {
    registry::init();
    setup_new_credentials();

    cargo_process("login --host")
        .arg(registry_url().to_string())
        .arg(TOKEN)
        .run();

    // Ensure that we get the new token for the registry
    assert!(check_token(TOKEN, None));
}

#[cargo_test]
fn login_with_old_and_new_credentials() {
    setup_new_credentials();
    login_with_old_credentials();
}

#[cargo_test]
fn login_without_credentials() {
    registry::init();
    cargo_process("login --host")
        .arg(registry_url().to_string())
        .arg(TOKEN)
        .run();

    // Ensure that we get the new token for the registry
    assert!(check_token(TOKEN, None));
}

#[cargo_test]
fn new_credentials_is_used_instead_old() {
    registry::init();
    setup_new_credentials();

    cargo_process("login --host")
        .arg(registry_url().to_string())
        .arg(TOKEN)
        .run();

    let config = Config::new(Shell::new(), cargo_home(), cargo_home());

    let token = config.get_string("registry.token").unwrap().map(|p| p.val);
    assert_eq!(token.unwrap(), TOKEN);
}

#[cargo_test]
fn registry_credentials() {
    registry::init();
    setup_new_credentials();

    let reg = "alternative";

    cargo_process("login --registry")
        .arg(reg)
        .arg(TOKEN)
        .arg("-Zunstable-options")
        .masquerade_as_nightly_cargo()
        .run();

    // Ensure that we have not updated the default token
    assert!(check_token(ORIGINAL_TOKEN, None));

    // Also ensure that we get the new token for the registry
    assert!(check_token(TOKEN, Some(reg)));
}
