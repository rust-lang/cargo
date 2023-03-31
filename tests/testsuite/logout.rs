//! Tests for the `cargo logout` command.

use cargo_test_support::install::cargo_home;
use cargo_test_support::registry::TestRegistry;
use cargo_test_support::{cargo_process, registry};
use std::fs;

#[cargo_test]
fn gated() {
    registry::init();
    cargo_process("logout")
        .masquerade_as_nightly_cargo(&["cargo-logout"])
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
    let credentials = cargo_home().join("credentials.toml");
    let contents = fs::read_to_string(&credentials).unwrap();
    let toml: toml::Table = contents.parse().unwrap();
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

fn simple_logout_test(registry: &TestRegistry, reg: Option<&str>, flag: &str, note: &str) {
    let msg = reg.unwrap_or("crates-io");
    check_config_token(reg, true);
    let mut cargo = cargo_process(&format!("logout -Z unstable-options {}", flag));
    if reg.is_none() {
        cargo.replace_crates_io(registry.index_url());
    }
    cargo
        .masquerade_as_nightly_cargo(&["cargo-logout"])
        .with_stderr(&format!(
            "\
[LOGOUT] token for `{msg}` has been removed from local storage
[NOTE] This does not revoke the token on the registry server.\n    \
If you need to revoke the token, visit {note} and follow the instructions there.
"
        ))
        .run();
    check_config_token(reg, false);

    let mut cargo = cargo_process(&format!("logout -Z unstable-options {}", flag));
    if reg.is_none() {
        cargo.replace_crates_io(registry.index_url());
    }
    cargo
        .masquerade_as_nightly_cargo(&["cargo-logout"])
        .with_stderr(&format!("[LOGOUT] not currently logged in to `{msg}`"))
        .run();
    check_config_token(reg, false);
}

#[cargo_test]
fn default_registry() {
    let registry = registry::init();
    simple_logout_test(&registry, None, "", "<https://crates.io/me>");
}

#[cargo_test]
fn other_registry() {
    let registry = registry::alt_init();
    simple_logout_test(
        &registry,
        Some("alternative"),
        "--registry alternative",
        "the `alternative` website",
    );
}
