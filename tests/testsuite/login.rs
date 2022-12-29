//! Tests for the `cargo login` command.

use cargo_test_support::cargo_process;
use cargo_test_support::install::cargo_home;
use cargo_test_support::paths::{self, CargoPathExt};
use cargo_test_support::registry::{self, RegistryBuilder};
use cargo_test_support::t;
use std::fs::{self};
use std::path::PathBuf;
use toml_edit::easy as toml;

const TOKEN: &str = "test-token";
const TOKEN2: &str = "test-token2";
const ORIGINAL_TOKEN: &str = "api-token";

fn setup_new_credentials() {
    let config = cargo_home().join("credentials");
    setup_new_credentials_at(config);
}

fn setup_new_credentials_at(config: PathBuf) {
    t!(fs::create_dir_all(config.parent().unwrap()));
    t!(fs::write(
        &config,
        format!(r#"token = "{token}""#, token = ORIGINAL_TOKEN)
    ));
}

fn check_token(expected_token: &str, registry: Option<&str>) -> bool {
    let credentials = cargo_home().join("credentials");
    assert!(credentials.is_file());

    let contents = fs::read_to_string(&credentials).unwrap();
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
fn registry_credentials() {
    let _alternative = RegistryBuilder::new().alternative().build();
    let _alternative2 = RegistryBuilder::new()
        .alternative_named("alternative2")
        .build();

    setup_new_credentials();

    let reg = "alternative";

    cargo_process("login --registry").arg(reg).arg(TOKEN).run();

    // Ensure that we have not updated the default token
    assert!(check_token(ORIGINAL_TOKEN, None));

    // Also ensure that we get the new token for the registry
    assert!(check_token(TOKEN, Some(reg)));

    let reg2 = "alternative2";
    cargo_process("login --registry")
        .arg(reg2)
        .arg(TOKEN2)
        .run();

    // Ensure not overwriting 1st alternate registry token with
    // 2nd alternate registry token (see rust-lang/cargo#7701).
    assert!(check_token(ORIGINAL_TOKEN, None));
    assert!(check_token(TOKEN, Some(reg)));
    assert!(check_token(TOKEN2, Some(reg2)));
}

#[cargo_test]
fn empty_login_token() {
    let registry = RegistryBuilder::new()
        .no_configure_registry()
        .no_configure_token()
        .build();
    setup_new_credentials();

    cargo_process("login")
        .replace_crates_io(registry.index_url())
        .with_stdout("please paste the token found on [..]/me below")
        .with_stdin("\t\n")
        .with_stderr(
            "\
[UPDATING] crates.io index
[ERROR] please provide a non-empty token
",
        )
        .with_status(101)
        .run();

    cargo_process("login")
        .replace_crates_io(registry.index_url())
        .arg("")
        .with_stderr(
            "\
[ERROR] please provide a non-empty token
",
        )
        .with_status(101)
        .run();
}

#[cargo_test]
fn bad_asymmetric_token_args() {
    // These cases are kept brief as the implementation is covered by clap, so this is only smoke testing that we have clap configured correctly.
    cargo_process("login --key-subject=foo tok")
        .with_stderr_contains(
            "[ERROR] The argument '--key-subject <SUBJECT>' cannot be used with '[token]'",
        )
        .with_status(1)
        .run();

    cargo_process("login --generate-keypair tok")
        .with_stderr_contains(
            "[ERROR] The argument '--generate-keypair' cannot be used with '[token]'",
        )
        .with_status(1)
        .run();

    cargo_process("login --secret-key tok")
        .with_stderr_contains("[ERROR] The argument '--secret-key' cannot be used with '[token]'")
        .with_status(1)
        .run();

    cargo_process("login --generate-keypair --secret-key")
        .with_stderr_contains(
            "[ERROR] The argument '--generate-keypair' cannot be used with '--secret-key'",
        )
        .with_status(1)
        .run();
}

#[cargo_test]
fn asymmetric_requires_nightly() {
    let registry = registry::init();
    cargo_process("login --key-subject=foo")          
        .replace_crates_io(registry.index_url())
        .with_status(101)
        .with_stderr_contains("[ERROR] the `key-subject` flag is unstable, pass `-Z registry-auth` to enable it\n\
            See https://github.com/rust-lang/cargo/issues/10519 for more information about the `key-subject` flag.")
        .run();
    cargo_process("login --generate-keypair")
        .replace_crates_io(registry.index_url())
        .with_status(101)
        .with_stderr_contains("[ERROR] the `generate-keypair` flag is unstable, pass `-Z registry-auth` to enable it\n\
            See https://github.com/rust-lang/cargo/issues/10519 for more information about the `generate-keypair` flag.")
        .run();
    cargo_process("login --secret-key")
        .replace_crates_io(registry.index_url())
        .with_status(101)
        .with_stderr_contains("[ERROR] the `secret-key` flag is unstable, pass `-Z registry-auth` to enable it\n\
            See https://github.com/rust-lang/cargo/issues/10519 for more information about the `secret-key` flag.")
        .run();
}

#[cargo_test]
fn login_with_no_cargo_dir() {
    // Create a config in the root directory because `login` requires the
    // index to be updated, and we don't want to hit crates.io.
    let registry = registry::init();
    fs::rename(paths::home().join(".cargo"), paths::root().join(".cargo")).unwrap();
    paths::home().rm_rf();
    cargo_process("login foo -v")
        .replace_crates_io(registry.index_url())
        .run();
    let credentials = fs::read_to_string(paths::home().join(".cargo/credentials")).unwrap();
    assert_eq!(credentials, "[registry]\ntoken = \"foo\"\n");
}

#[cargo_test]
fn login_with_differently_sized_token() {
    // Verify that the configuration file gets properly truncated.
    let registry = registry::init();
    let credentials = paths::home().join(".cargo/credentials");
    fs::remove_file(&credentials).unwrap();
    cargo_process("login lmaolmaolmao -v")
        .replace_crates_io(registry.index_url())
        .run();
    cargo_process("login lmao -v")
        .replace_crates_io(registry.index_url())
        .run();
    cargo_process("login lmaolmaolmao -v")
        .replace_crates_io(registry.index_url())
        .run();
    let credentials = fs::read_to_string(&credentials).unwrap();
    assert_eq!(credentials, "[registry]\ntoken = \"lmaolmaolmao\"\n");
}

#[cargo_test]
fn login_with_token_on_stdin() {
    let registry = registry::init();
    let credentials = paths::home().join(".cargo/credentials");
    fs::remove_file(&credentials).unwrap();
    cargo_process("login lmao -v")
        .replace_crates_io(registry.index_url())
        .run();
    cargo_process("login")
        .replace_crates_io(registry.index_url())
        .with_stdout("please paste the token found on [..]/me below")
        .with_stdin("some token")
        .run();
    let credentials = fs::read_to_string(&credentials).unwrap();
    assert_eq!(credentials, "[registry]\ntoken = \"some token\"\n");
}

#[cargo_test]
fn login_with_asymmetric_token_and_subject_on_stdin() {
    let registry = registry::init();
    let credentials = paths::home().join(".cargo/credentials");
    fs::remove_file(&credentials).unwrap();
    cargo_process("login --key-subject=foo --secret-key -v -Z registry-auth")
        .masquerade_as_nightly_cargo(&["registry-auth"])
        .replace_crates_io(registry.index_url())
        .with_stdout(
            "\
        please paste the API secret key below
k3.public.AmDwjlyf8jAV3gm5Z7Kz9xAOcsKslt_Vwp5v-emjFzBHLCtcANzTaVEghTNEMj9PkQ",
        )
        .with_stdin("k3.secret.fNYVuMvBgOlljt9TDohnaYLblghqaHoQquVZwgR6X12cBFHZLFsaU3q7X3k1Zn36")
        .run();
    let credentials = fs::read_to_string(&credentials).unwrap();
    assert!(credentials.starts_with("[registry]\n"));
    assert!(credentials.contains("secret-key-subject = \"foo\"\n"));
    assert!(credentials.contains("secret-key = \"k3.secret.fNYVuMvBgOlljt9TDohnaYLblghqaHoQquVZwgR6X12cBFHZLFsaU3q7X3k1Zn36\"\n"));
}

#[cargo_test]
fn login_with_asymmetric_token_on_stdin() {
    let registry = registry::init();
    let credentials = paths::home().join(".cargo/credentials");
    fs::remove_file(&credentials).unwrap();
    cargo_process("login --secret-key -v -Z registry-auth")
        .masquerade_as_nightly_cargo(&["registry-auth"])
        .replace_crates_io(registry.index_url())
        .with_stdout(
            "\
    please paste the API secret key below
k3.public.AmDwjlyf8jAV3gm5Z7Kz9xAOcsKslt_Vwp5v-emjFzBHLCtcANzTaVEghTNEMj9PkQ",
        )
        .with_stdin("k3.secret.fNYVuMvBgOlljt9TDohnaYLblghqaHoQquVZwgR6X12cBFHZLFsaU3q7X3k1Zn36")
        .run();
    let credentials = fs::read_to_string(&credentials).unwrap();
    assert_eq!(credentials, "[registry]\nsecret-key = \"k3.secret.fNYVuMvBgOlljt9TDohnaYLblghqaHoQquVZwgR6X12cBFHZLFsaU3q7X3k1Zn36\"\n");
}

#[cargo_test]
fn login_with_asymmetric_key_subject_without_key() {
    let registry = registry::init();
    let credentials = paths::home().join(".cargo/credentials");
    fs::remove_file(&credentials).unwrap();
    cargo_process("login --key-subject=foo -Z registry-auth")
        .masquerade_as_nightly_cargo(&["registry-auth"])
        .replace_crates_io(registry.index_url())
        .with_stderr_contains("error: need a secret_key to set a key_subject")
        .with_status(101)
        .run();

    // ok so add a secret_key to the credentials
    cargo_process("login --secret-key -v -Z registry-auth")
        .masquerade_as_nightly_cargo(&["registry-auth"])
        .replace_crates_io(registry.index_url())
        .with_stdout(
            "please paste the API secret key below
k3.public.AmDwjlyf8jAV3gm5Z7Kz9xAOcsKslt_Vwp5v-emjFzBHLCtcANzTaVEghTNEMj9PkQ",
        )
        .with_stdin("k3.secret.fNYVuMvBgOlljt9TDohnaYLblghqaHoQquVZwgR6X12cBFHZLFsaU3q7X3k1Zn36")
        .run();

    // and then it shuld work
    cargo_process("login --key-subject=foo -Z registry-auth")
        .masquerade_as_nightly_cargo(&["registry-auth"])
        .replace_crates_io(registry.index_url())
        .run();

    let credentials = fs::read_to_string(&credentials).unwrap();
    assert!(credentials.starts_with("[registry]\n"));
    assert!(credentials.contains("secret-key-subject = \"foo\"\n"));
    assert!(credentials.contains("secret-key = \"k3.secret.fNYVuMvBgOlljt9TDohnaYLblghqaHoQquVZwgR6X12cBFHZLFsaU3q7X3k1Zn36\"\n"));
}

#[cargo_test]
fn login_with_generate_asymmetric_token() {
    let registry = registry::init();
    let credentials = paths::home().join(".cargo/credentials");
    fs::remove_file(&credentials).unwrap();
    cargo_process("login --generate-keypair -Z registry-auth")
        .masquerade_as_nightly_cargo(&["registry-auth"])
        .replace_crates_io(registry.index_url())
        .with_stdout("k3.public.[..]")
        .run();
    let credentials = fs::read_to_string(&credentials).unwrap();
    assert!(credentials.contains("secret-key = \"k3.secret."));
}
