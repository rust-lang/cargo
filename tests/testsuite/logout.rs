//! Tests for the `cargo logout` command.

use super::login::check_token;
use cargo_test_support::paths::{self, CargoPathExt};
use cargo_test_support::registry::TestRegistry;
use cargo_test_support::{cargo_process, registry};

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

fn simple_logout_test(registry: &TestRegistry, reg: Option<&str>, flag: &str, note: &str) {
    let msg = reg.unwrap_or("crates-io");
    check_token(Some(registry.token()), reg);
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
    check_token(None, reg);

    let mut cargo = cargo_process(&format!("logout -Z unstable-options {}", flag));
    if reg.is_none() {
        cargo.replace_crates_io(registry.index_url());
    }
    cargo
        .masquerade_as_nightly_cargo(&["cargo-logout"])
        .with_stderr(&format!("[LOGOUT] not currently logged in to `{msg}`"))
        .run();
    check_token(None, reg);
}

#[cargo_test]
fn default_registry_unconfigured() {
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
    // It should not touch crates.io.
    check_token(Some("sekrit"), None);
}

#[cargo_test]
fn default_registry_configured() {
    // When registry.default is set, logout should use that one when
    // --registry is not used.
    let cargo_home = paths::home().join(".cargo");
    cargo_home.mkdir_p();
    cargo_util::paths::write(
        &cargo_home.join("config.toml"),
        r#"
            [registry]
            default = "dummy-registry"

            [registries.dummy-registry]
            index = "https://127.0.0.1/index"
        "#,
    )
    .unwrap();
    cargo_util::paths::write(
        &cargo_home.join("credentials.toml"),
        r#"
        [registry]
        token = "crates-io-token"

        [registries.dummy-registry]
        token = "dummy-token"
        "#,
    )
    .unwrap();
    check_token(Some("dummy-token"), Some("dummy-registry"));
    check_token(Some("crates-io-token"), None);

    cargo_process("logout -Zunstable-options")
        .masquerade_as_nightly_cargo(&["cargo-logout"])
        .with_stderr(
            "\
[LOGOUT] token for `crates-io` has been removed from local storage
[NOTE] This does not revoke the token on the registry server.
    If you need to revoke the token, visit <https://crates.io/me> \
    and follow the instructions there.
",
        )
        .run();
    check_token(Some("dummy-token"), Some("dummy-registry"));
    check_token(None, None);

    cargo_process("logout -Zunstable-options")
        .masquerade_as_nightly_cargo(&["cargo-logout"])
        .with_stderr("[LOGOUT] not currently logged in to `crates-io`")
        .run();
}
