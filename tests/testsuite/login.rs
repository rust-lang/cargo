//! Tests for the `cargo login` command.

use cargo_test_support::cargo_process;
use cargo_test_support::paths::{self, CargoPathExt};
use cargo_test_support::registry::{self, RegistryBuilder};
use cargo_test_support::t;
use std::fs;
use std::path::PathBuf;

const TOKEN: &str = "test-token";
const TOKEN2: &str = "test-token2";
const ORIGINAL_TOKEN: &str = "api-token";

fn credentials_toml() -> PathBuf {
    paths::home().join(".cargo/credentials.toml")
}

fn setup_new_credentials() {
    setup_new_credentials_at(credentials_toml());
}

fn setup_new_credentials_at(config: PathBuf) {
    t!(fs::create_dir_all(config.parent().unwrap()));
    t!(fs::write(
        &config,
        format!(r#"token = "{token}""#, token = ORIGINAL_TOKEN)
    ));
}

/// Asserts whether or not the token is set to the given value for the given registry.
pub fn check_token(expected_token: Option<&str>, registry: Option<&str>) {
    let credentials = credentials_toml();
    assert!(credentials.is_file());

    let contents = fs::read_to_string(&credentials).unwrap();
    let toml: toml::Table = contents.parse().unwrap();

    let actual_token = match registry {
        // A registry has been provided, so check that the token exists in a
        // table for the registry.
        Some(registry) => toml
            .get("registries")
            .and_then(|registries_table| registries_table.get(registry))
            .and_then(|registry_table| match registry_table.get("token") {
                Some(&toml::Value::String(ref token)) => Some(token.as_str().to_string()),
                _ => None,
            }),
        // There is no registry provided, so check the global token instead.
        None => toml
            .get("registry")
            .and_then(|registry_table| registry_table.get("token"))
            .and_then(|v| match v {
                toml::Value::String(ref token) => Some(token.as_str().to_string()),
                _ => None,
            }),
    };

    match (actual_token, expected_token) {
        (None, None) => {}
        (Some(actual), Some(expected)) => assert_eq!(actual, expected),
        (None, Some(expected)) => {
            panic!("expected `{registry:?}` to be `{expected}`, but was not set")
        }
        (Some(actual), None) => {
            panic!("expected `{registry:?}` to be unset, but was set to `{actual}`")
        }
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
    check_token(Some(ORIGINAL_TOKEN), None);

    // Also ensure that we get the new token for the registry
    check_token(Some(TOKEN), Some(reg));

    let reg2 = "alternative2";
    cargo_process("login --registry")
        .arg(reg2)
        .arg(TOKEN2)
        .run();

    // Ensure not overwriting 1st alternate registry token with
    // 2nd alternate registry token (see rust-lang/cargo#7701).
    check_token(Some(ORIGINAL_TOKEN), None);
    check_token(Some(TOKEN), Some(reg));
    check_token(Some(TOKEN2), Some(reg2));
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
        .with_stdin("\t\n")
        .with_stderr(
            "\
[UPDATING] crates.io index
[ERROR] credential provider `cargo:token` failed action `login`

Caused by:
  please provide a non-empty token
",
        )
        .with_status(101)
        .run();

    cargo_process("login")
        .replace_crates_io(registry.index_url())
        .arg("")
        .with_stderr(
            "\
[ERROR] credential provider `cargo:token` failed action `login`

Caused by:
  please provide a non-empty token
",
        )
        .with_status(101)
        .run();
}

#[cargo_test]
fn invalid_login_token() {
    let registry = RegistryBuilder::new()
        .no_configure_registry()
        .no_configure_token()
        .build();
    setup_new_credentials();

    let check = |stdin: &str, stderr: &str, status: i32| {
        cargo_process("login")
            .replace_crates_io(registry.index_url())
            .with_stdin(stdin)
            .with_stderr(stderr)
            .with_status(status)
            .run();
    };

    let invalid = |stdin: &str| {
        check(
            stdin,
            "[ERROR] credential provider `cargo:token` failed action `login`

Caused by:
  token contains invalid characters.
  Only printable ISO-8859-1 characters are allowed as it is sent in a HTTPS header.",
            101,
        )
    };
    let valid = |stdin: &str| check(stdin, "[LOGIN] token for `crates-io` saved", 0);

    // Update config.json so that the rest of the tests don't need to care
    // whether or not `Updating` is printed.
    check(
        "test",
        "\
[UPDATING] crates.io index
[LOGIN] token for `crates-io` saved
",
        0,
    );

    invalid("😄");
    invalid("\u{0016}");
    invalid("\u{0000}");
    invalid("你好");
    valid("foo\tbar");
    valid("foo bar");
    valid(
        r##"!"#$%&'()*+,-./0123456789:;<=>?@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\]^_`abcdefghijklmnopqrstuvwxyz{|}~"##,
    );
}

#[cargo_test]
fn bad_asymmetric_token_args() {
    let registry = RegistryBuilder::new()
        .credential_provider(&["cargo:paseto"])
        .no_configure_token()
        .build();

    // These cases are kept brief as the implementation is covered by clap, so this is only smoke testing that we have clap configured correctly.
    cargo_process("login -Zasymmetric-token -- --key-subject")
        .masquerade_as_nightly_cargo(&["asymmetric-token"])
        .replace_crates_io(registry.index_url())
        .with_stderr_contains(
            "  error: a value is required for '--key-subject <SUBJECT>' but none was supplied",
        )
        .with_status(101)
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
    let credentials = fs::read_to_string(credentials_toml()).unwrap();
    assert_eq!(credentials, "[registry]\ntoken = \"foo\"\n");
}

#[cargo_test]
fn login_with_asymmetric_token_and_subject_on_stdin() {
    let registry = RegistryBuilder::new()
        .credential_provider(&["cargo:paseto"])
        .no_configure_token()
        .build();
    let credentials = credentials_toml();
    cargo_process("login -v -Z asymmetric-token -- --key-subject=foo")
        .masquerade_as_nightly_cargo(&["asymmetric-token"])
        .replace_crates_io(registry.index_url())
        .with_stderr_contains(
            "\
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
fn login_with_differently_sized_token() {
    // Verify that the configuration file gets properly truncated.
    let registry = registry::init();
    let credentials = credentials_toml();
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
    let credentials = credentials_toml();
    fs::remove_file(&credentials).unwrap();
    cargo_process("login lmao -v")
        .replace_crates_io(registry.index_url())
        .run();
    cargo_process("login")
        .replace_crates_io(registry.index_url())
        .with_stdin("some token")
        .run();
    let credentials = fs::read_to_string(&credentials).unwrap();
    assert_eq!(credentials, "[registry]\ntoken = \"some token\"\n");
}

#[cargo_test]
fn login_with_asymmetric_token_on_stdin() {
    let _registry = RegistryBuilder::new()
        .credential_provider(&["cargo:paseto"])
        .alternative()
        .no_configure_token()
        .build();
    let credentials = credentials_toml();
    cargo_process("login -v -Z asymmetric-token --registry alternative")
        .masquerade_as_nightly_cargo(&["asymmetric-token"])
        .with_stderr(
            "\
[UPDATING] [..]
[CREDENTIAL] cargo:paseto login alternative
k3.public.AmDwjlyf8jAV3gm5Z7Kz9xAOcsKslt_Vwp5v-emjFzBHLCtcANzTaVEghTNEMj9PkQ",
        )
        .with_stdin("k3.secret.fNYVuMvBgOlljt9TDohnaYLblghqaHoQquVZwgR6X12cBFHZLFsaU3q7X3k1Zn36")
        .run();
    let credentials = fs::read_to_string(&credentials).unwrap();
    assert_eq!(credentials, "[registries.alternative]\nsecret-key = \"k3.secret.fNYVuMvBgOlljt9TDohnaYLblghqaHoQquVZwgR6X12cBFHZLFsaU3q7X3k1Zn36\"\n");
}

#[cargo_test]
fn login_with_generate_asymmetric_token() {
    let _registry = RegistryBuilder::new()
        .credential_provider(&["cargo:paseto"])
        .alternative()
        .no_configure_token()
        .build();
    let credentials = credentials_toml();
    cargo_process("login -Z asymmetric-token --registry alternative")
        .masquerade_as_nightly_cargo(&["asymmetric-token"])
        .with_stderr("[UPDATING] `alternative` index\nk3.public.[..]")
        .run();
    let credentials = fs::read_to_string(&credentials).unwrap();
    assert!(credentials.contains("secret-key = \"k3.secret."));
}

#[cargo_test]
fn default_registry_configured() {
    // When registry.default is set, login should use that one when
    // --registry is not used.
    let _alternative = RegistryBuilder::new().alternative().build();
    let cargo_home = paths::home().join(".cargo");
    cargo_util::paths::append(
        &cargo_home.join("config"),
        br#"
            [registry]
            default = "alternative"
        "#,
    )
    .unwrap();

    cargo_process("login")
        .arg("a-new-token")
        .with_stderr(
            "\
[UPDATING] `alternative` index
[LOGIN] token for `alternative` saved
",
        )
        .run();

    check_token(None, None);
    check_token(Some("a-new-token"), Some("alternative"));
}
