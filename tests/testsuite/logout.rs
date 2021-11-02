//! Tests for the `cargo logout` command.

use cargo_test_support::install::cargo_home;
use cargo_test_support::{cargo_process, registry};
use std::fs;
use toml_edit::easy as toml;

#[cargo_test]
fn gated() {
    registry::init();
    cargo_process("logout")
        .masquerade_as_nightly_cargo()
        .with_status(101)
        .with_stderr(
            "\
[ERROR] the `cargo logout` command is unstable, pass `-Z unstable-options` to enable it
See https://github.com/rust-lang/cargo/issues/8933 for more information about \
the `cargo logout` command.
",
        )
        .run();
}

/// Checks whether or not the token is set for the given token.
fn check_config_token(registry: Option<&str>, should_be_set: bool) {
    let credentials = cargo_home().join("credentials");
    let contents = fs::read_to_string(&credentials).unwrap();
    let toml: toml::Value = contents.parse().unwrap();
    if let Some(registry) = registry {
        assert_eq!(
            toml.get("registries")
                .and_then(|registries| registries.get(registry))
                .and_then(|registry| registry.get("token"))
                .is_some(),
            should_be_set
        );
    } else {
        assert_eq!(
            toml.get("registry")
                .and_then(|registry| registry.get("token"))
                .is_some(),
            should_be_set
        );
    }
}

fn simple_logout_test(reg: Option<&str>, flag: &str) {
    registry::init();
    let msg = reg.unwrap_or("crates.io");
    check_config_token(reg, true);
    cargo_process(&format!("logout -Z unstable-options {}", flag))
        .masquerade_as_nightly_cargo()
        .with_stderr(&format!(
            "\
[UPDATING] [..]
[LOGOUT] token for `{}` has been removed from local storage
",
            msg
        ))
        .run();
    check_config_token(reg, false);

    cargo_process(&format!("logout -Z unstable-options {}", flag))
        .masquerade_as_nightly_cargo()
        .with_stderr(&format!(
            "\
[LOGOUT] not currently logged in to `{}`
",
            msg
        ))
        .run();
    check_config_token(reg, false);
}

#[cargo_test]
fn default_registry() {
    simple_logout_test(None, "");
}

#[cargo_test]
fn other_registry() {
    registry::alt_init();
    simple_logout_test(Some("alternative"), "--registry alternative");
}
