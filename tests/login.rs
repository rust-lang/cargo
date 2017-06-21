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
const CONFIG_FILE: &str = r#"
    [registry]
    token = "api-token"
"#;

fn setup_old_credentials() {
    let config = cargo_home().join("config");
    t!(fs::create_dir_all(config.parent().unwrap()));
    t!(t!(File::create(&config)).write_all(&CONFIG_FILE.as_bytes()));
}

fn setup_new_credentials() {
    let config = cargo_home().join("credentials");
    t!(fs::create_dir_all(config.parent().unwrap()));
    t!(t!(File::create(&config)).write_all(br#"
        token = "api-token"
    "#));
}

fn check_host_token(toml: toml::Value) -> bool {
    match toml {
        toml::Value::Table(table) => match table.get("token") {
            Some(v) => match v {
                &toml::Value::String(ref token) => (token.as_str() == TOKEN),
                _ => false,
            },
            None => false,
        },
        _ => false,
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
    assert!(CONFIG_FILE == &contents);

    let credentials = cargo_home().join("credentials");
    assert_that(&credentials, existing_file());

    contents.clear();
    File::open(&credentials).unwrap().read_to_string(&mut contents).unwrap();
    assert!(check_host_token(contents.parse().unwrap()));
}

#[test]
fn login_with_new_credentials() {
    setup_new_credentials();

    assert_that(cargo_process().arg("login")
                .arg("--host").arg(registry().to_string()).arg(TOKEN),
                execs().with_status(0));

    let config = cargo_home().join("config");
    assert_that(&config, is_not(existing_file()));

    let credentials = cargo_home().join("credentials");
    assert_that(&credentials, existing_file());

    let mut contents = String::new();
    File::open(&credentials).unwrap().read_to_string(&mut contents).unwrap();
    assert!(check_host_token(contents.parse().unwrap()));
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

    let credentials = cargo_home().join("credentials");
    assert_that(&credentials, existing_file());

    let mut contents = String::new();
    File::open(&credentials).unwrap().read_to_string(&mut contents).unwrap();
    assert!(check_host_token(contents.parse().unwrap()));
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
    assert!(token.unwrap() == TOKEN);
}
