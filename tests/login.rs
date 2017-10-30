#[macro_use]
extern crate cargotest;
extern crate cargo;
extern crate hamcrest;
extern crate toml;

use std::io::prelude::*;
use std::fs::{self, File};

use cargotest::cargo_process;
use cargotest::support::execs;
use cargotest::support::registry::registry;
use cargotest::install::cargo_home;
use cargo::util::config::Config;
use cargo::core::Shell;
use hamcrest::{assert_that, existing_file, is_not};

const TOKEN: &str = "test-token";
const ORIGINAL_TOKEN: &str = "api-token";
const CONFIG_FILE: &str = r#"
    [registry]
    token = "api-token"

    [registries.test-reg]
    index = "dummy_index"

    [registries.test.reg]
    index = "dummy_index"
"#;

fn setup_old_credentials() {
    let config = cargo_home().join("config");
    t!(fs::create_dir_all(config.parent().unwrap()));
    t!(t!(File::create(&config)).write_all(CONFIG_FILE.as_bytes()));
}

fn setup_new_credentials() {
    let config = cargo_home().join("credentials");
    t!(fs::create_dir_all(config.parent().unwrap()));
    t!(t!(File::create(&config)).write_all(format!(r#"
        token = "{token}"
    "#, token = ORIGINAL_TOKEN)
    .as_bytes()));
}

fn check_token(expected_token: &str, registry: Option<&str>) -> bool {

    let credentials = cargo_home().join("credentials");
    assert_that(&credentials, existing_file());

    let mut contents = String::new();
    File::open(&credentials).unwrap().read_to_string(&mut contents).unwrap();
    let toml: toml::Value = contents.parse().unwrap();

    let token = match (registry, toml) {
        // A registry has been provided, so check that the token exists in a
        // table for the registry.
        (Some(registry), toml::Value::Table(table)) => {
            table.get(registry).and_then(|registry_table| {
                match registry_table.get("token") {
                    Some(&toml::Value::String(ref token)) => Some(token.as_str().to_string()),
                    _ => None,
                }
            })
        },
        // There is no registry provided, so check the global token instead.
        (None, toml::Value::Table(table)) => {
            table.get("token").and_then(|v| {
                match v {
                    &toml::Value::String(ref token) => Some(token.as_str().to_string()),
                    _ => None,
                }
            })
        }
        _ => None
    };

    if let Some(token_val) = token {
        token_val == expected_token
    } else {
        false
    }
}

#[test]
fn login_with_old_credentials() {
    setup_old_credentials();

    assert_that(cargo_process().arg("login")
                .arg("--host").arg(registry().to_string()).arg(TOKEN),
                execs().with_status(0));

    let config = cargo_home().join("config");
    assert_that(&config, existing_file());

    let mut contents = String::new();
    File::open(&config).unwrap().read_to_string(&mut contents).unwrap();
    assert_eq!(CONFIG_FILE, contents);

    // Ensure that we get the new token for the registry
    assert!(check_token(TOKEN, None));
}

#[test]
fn login_with_new_credentials() {
    setup_new_credentials();

    assert_that(cargo_process().arg("login")
                .arg("--host").arg(registry().to_string()).arg(TOKEN),
                execs().with_status(0));

    let config = cargo_home().join("config");
    assert_that(&config, is_not(existing_file()));

    // Ensure that we get the new token for the registry
    assert!(check_token(TOKEN, None));
}

#[test]
fn login_with_old_and_new_credentials() {
    setup_new_credentials();
    login_with_old_credentials();
}

#[test]
fn login_without_credentials() {
    assert_that(cargo_process().arg("login")
                .arg("--host").arg(registry().to_string()).arg(TOKEN),
                execs().with_status(0));

    let config = cargo_home().join("config");
    assert_that(&config, is_not(existing_file()));

    // Ensure that we get the new token for the registry
    assert!(check_token(TOKEN, None));
}

#[test]
fn new_credentials_is_used_instead_old() {
    setup_old_credentials();
    setup_new_credentials();

    assert_that(cargo_process().arg("login")
                .arg("--host").arg(registry().to_string()).arg(TOKEN),
                execs().with_status(0));

    let config = Config::new(Shell::new(), cargo_home(), cargo_home());

    let token = config.get_string("registry.token").unwrap().map(|p| p.val);
    assert_eq!(token.unwrap(), TOKEN);
}

#[test]
fn registry_credentials() {
    setup_old_credentials();
    setup_new_credentials();

    let reg = "test-reg";

    assert_that(cargo_process().arg("login")
                .arg("--registry").arg(reg).arg(TOKEN),
                execs().with_status(0));

    // Ensure that we have not updated the default token
    assert!(check_token(ORIGINAL_TOKEN, None));

    // Also ensure that we get the new token for the registry
    assert!(check_token(TOKEN, Some(reg)));
}

#[test]
fn registry_credentials_with_dots() {
    setup_old_credentials();
    setup_new_credentials();

    let reg = "test.reg";

    assert_that(cargo_process().arg("login")
                .arg("--registry").arg(reg).arg(TOKEN),
                execs().with_status(0));

    // Ensure that we have not updated the default token
    assert!(check_token(ORIGINAL_TOKEN, None));

    // Also ensure that we get the new token for the registry
    assert!(check_token(TOKEN, Some(reg)));
}
