//! Tests for registry authentication.

use cargo_test_support::registry::{Package, RegistryBuilder};
use cargo_test_support::{project, Execs, Project};

fn cargo(p: &Project, s: &str) -> Execs {
    let mut e = p.cargo(s);
    e.masquerade_as_nightly_cargo(&["registry-auth"])
        .arg("-Zregistry-auth");
    e
}

fn make_project() -> Project {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies.bar]
                version = "0.0.1"
                registry = "alternative"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();
    Package::new("bar", "0.0.1").alternative(true).publish();
    p
}

static SUCCESS_OUTPUT: &'static str = "\
[UPDATING] `alternative` index
[DOWNLOADING] crates ...
[DOWNLOADED] bar v0.0.1 (registry `alternative`)
[COMPILING] bar v0.0.1 (registry `alternative`)
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
";

#[cargo_test]
fn requires_nightly() {
    let _registry = RegistryBuilder::new()
        .alternative()
        .auth_required()
        .http_api()
        .build();

    let p = make_project();
    p.cargo("check")
        .with_status(101)
        .with_stderr(
            r#"[UPDATING] `alternative` index
[DOWNLOADING] crates ...
error: failed to download from `[..]/dl/bar/0.0.1/download`

Caused by:
  failed to get successful HTTP response from `[..]` (127.0.0.1), got 401
  body:
  Unauthorized message from server.
"#,
        )
        .run();
}

#[cargo_test]
fn simple() {
    let _registry = RegistryBuilder::new()
        .alternative()
        .auth_required()
        .http_index()
        .build();

    let p = make_project();
    cargo(&p, "build").with_stderr(SUCCESS_OUTPUT).run();
}

#[cargo_test]
fn simple_with_asymmetric() {
    let _registry = RegistryBuilder::new()
        .alternative()
        .auth_required()
        .http_index()
        .token(cargo_test_support::registry::Token::rfc_key())
        .build();

    let p = make_project();
    cargo(&p, "build").with_stderr(SUCCESS_OUTPUT).run();
}

#[cargo_test]
fn environment_config() {
    let registry = RegistryBuilder::new()
        .alternative()
        .auth_required()
        .no_configure_registry()
        .no_configure_token()
        .http_index()
        .build();
    let p = make_project();
    cargo(&p, "build")
        .env(
            "CARGO_REGISTRIES_ALTERNATIVE_INDEX",
            registry.index_url().as_str(),
        )
        .env("CARGO_REGISTRIES_ALTERNATIVE_TOKEN", registry.token())
        .with_stderr(SUCCESS_OUTPUT)
        .run();
}

#[cargo_test]
fn environment_token() {
    let registry = RegistryBuilder::new()
        .alternative()
        .auth_required()
        .no_configure_token()
        .http_index()
        .build();

    let p = make_project();
    cargo(&p, "build")
        .env("CARGO_REGISTRIES_ALTERNATIVE_TOKEN", registry.token())
        .with_stderr(SUCCESS_OUTPUT)
        .run();
}

#[cargo_test]
fn environment_token_with_asymmetric() {
    let registry = RegistryBuilder::new()
        .alternative()
        .auth_required()
        .no_configure_token()
        .http_index()
        .token(cargo_test_support::registry::Token::Keys(
            "k3.secret.fNYVuMvBgOlljt9TDohnaYLblghqaHoQquVZwgR6X12cBFHZLFsaU3q7X3k1Zn36"
                .to_string(),
            None,
        ))
        .build();

    let p = make_project();
    cargo(&p, "build")
        .env("CARGO_REGISTRIES_ALTERNATIVE_SECRET_KEY", registry.key())
        .with_stderr(SUCCESS_OUTPUT)
        .run();
}

#[cargo_test]
fn warn_both_asymmetric_and_token() {
    let _server = RegistryBuilder::new()
        .alternative()
        .no_configure_token()
        .build();
    let p = project()
        .file(
            ".cargo/config",
            r#"
                [registries.alternative]
                token = "sekrit"
                secret-key = "k3.secret.fNYVuMvBgOlljt9TDohnaYLblghqaHoQquVZwgR6X12cBFHZLFsaU3q7X3k1Zn36"
            "#,
        )
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                description = "foo"
                authors = []
                license = "MIT"
                homepage = "https://example.com/"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("publish --no-verify --registry alternative")
        .masquerade_as_nightly_cargo(&["credential-process", "registry-auth"])
        .arg("-Zregistry-auth")
        .with_status(101)
        .with_stderr(
            "\
[UPDATING] [..]
[ERROR] both `token` and `secret-key` were specified in the config for registry `alternative`.
Only one of these values may be set, remove one or the other to proceed.
",
        )
        .run();
}

#[cargo_test]
fn warn_both_asymmetric_and_credential_process() {
    let _server = RegistryBuilder::new()
        .alternative()
        .no_configure_token()
        .build();
    let p = project()
        .file(
            ".cargo/config",
            r#"
                [registries.alternative]
                credential-process = "false"
                secret-key = "k3.secret.fNYVuMvBgOlljt9TDohnaYLblghqaHoQquVZwgR6X12cBFHZLFsaU3q7X3k1Zn36"
            "#,
        )
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                description = "foo"
                authors = []
                license = "MIT"
                homepage = "https://example.com/"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("publish --no-verify --registry alternative")
        .masquerade_as_nightly_cargo(&["credential-process", "registry-auth"])
        .arg("-Zcredential-process")
        .arg("-Zregistry-auth")
        .with_status(101)
        .with_stderr(
            "\
[UPDATING] [..]
[ERROR] both `credential-process` and `secret-key` were specified in the config for registry `alternative`.
Only one of these values may be set, remove one or the other to proceed.
",
        )
        .run();
}

#[cargo_test]
fn bad_environment_token_with_asymmetric_subject() {
    let registry = RegistryBuilder::new()
        .alternative()
        .auth_required()
        .no_configure_token()
        .http_index()
        .token(cargo_test_support::registry::Token::Keys(
            "k3.secret.fNYVuMvBgOlljt9TDohnaYLblghqaHoQquVZwgR6X12cBFHZLFsaU3q7X3k1Zn36"
                .to_string(),
            None,
        ))
        .build();

    let p = make_project();
    cargo(&p, "build")
        .env("CARGO_REGISTRIES_ALTERNATIVE_SECRET_KEY", registry.key())
        .env(
            "CARGO_REGISTRIES_ALTERNATIVE_SECRET_KEY_SUBJECT",
            "incorrect",
        )
        .with_stderr_contains(
            "  token rejected for `alternative`, please run `cargo login --registry alternative`",
        )
        .with_status(101)
        .run();
}

#[cargo_test]
fn bad_environment_token_with_asymmetric_incorrect_subject() {
    let registry = RegistryBuilder::new()
        .alternative()
        .auth_required()
        .no_configure_token()
        .http_index()
        .token(cargo_test_support::registry::Token::rfc_key())
        .build();

    let p = make_project();
    cargo(&p, "build")
        .env("CARGO_REGISTRIES_ALTERNATIVE_SECRET_KEY", registry.key())
        .env(
            "CARGO_REGISTRIES_ALTERNATIVE_SECRET_KEY_SUBJECT",
            "incorrect",
        )
        .with_stderr_contains(
            "  token rejected for `alternative`, please run `cargo login --registry alternative`",
        )
        .with_status(101)
        .run();
}

#[cargo_test]
fn bad_environment_token_with_incorrect_asymmetric() {
    let _registry = RegistryBuilder::new()
        .alternative()
        .auth_required()
        .no_configure_token()
        .http_index()
        .token(cargo_test_support::registry::Token::Keys(
            "k3.secret.fNYVuMvBgOlljt9TDohnaYLblghqaHoQquVZwgR6X12cBFHZLFsaU3q7X3k1Zn36"
                .to_string(),
            None,
        ))
        .build();

    let p = make_project();
    cargo(&p, "build")
        .env(
            "CARGO_REGISTRIES_ALTERNATIVE_SECRET_KEY",
            "k3.secret.9Vxr5hVlI_g_orBZN54vPz20bmB4O76wB_MVqUSuJJJqHFLwP8kdn_RY5g6J6pQG",
        )
        .with_stderr_contains(
            "  token rejected for `alternative`, please run `cargo login --registry alternative`",
        )
        .with_status(101)
        .run();
}

#[cargo_test]
fn missing_token() {
    let _registry = RegistryBuilder::new()
        .alternative()
        .auth_required()
        .no_configure_token()
        .http_index()
        .build();

    let p = make_project();
    cargo(&p, "build")
        .with_status(101)
        .with_stderr(
            "\
[UPDATING] `alternative` index
[ERROR] failed to get `bar` as a dependency of package `foo v0.0.1 ([..])`

Caused by:
  no token found for `alternative`, please run `cargo login --registry alternative`
  or use environment variable CARGO_REGISTRIES_ALTERNATIVE_TOKEN",
        )
        .run();
}

#[cargo_test]
fn missing_token_git() {
    let _registry = RegistryBuilder::new()
        .alternative()
        .auth_required()
        .no_configure_token()
        .build();

    let p = make_project();
    cargo(&p, "build")
        .with_status(101)
        .with_stderr(
            "\
[UPDATING] `alternative` index
[ERROR] failed to download `bar v0.0.1 (registry `alternative`)`

Caused by:
  unable to get packages from source

Caused by:
  no token found for `alternative`, please run `cargo login --registry alternative`
  or use environment variable CARGO_REGISTRIES_ALTERNATIVE_TOKEN",
        )
        .run();
}

#[cargo_test]
fn incorrect_token() {
    let _registry = RegistryBuilder::new()
        .alternative()
        .auth_required()
        .no_configure_token()
        .http_index()
        .build();

    let p = make_project();
    cargo(&p, "build")
        .env("CARGO_REGISTRIES_ALTERNATIVE_TOKEN", "incorrect")
        .with_status(101)
        .with_stderr(
            "\
[UPDATING] `alternative` index
[ERROR] failed to get `bar` as a dependency of package `foo v0.0.1 ([..])`

Caused by:
  token rejected for `alternative`, please run `cargo login --registry alternative`
  or use environment variable CARGO_REGISTRIES_ALTERNATIVE_TOKEN

Caused by:
  failed to get successful HTTP response from `http://[..]/index/config.json`, got 401
  body:
  Unauthorized message from server.",
        )
        .run();
}

#[cargo_test]
fn incorrect_token_git() {
    let _registry = RegistryBuilder::new()
        .alternative()
        .auth_required()
        .no_configure_token()
        .http_api()
        .build();

    let p = make_project();
    cargo(&p, "build")
        .env("CARGO_REGISTRIES_ALTERNATIVE_TOKEN", "incorrect")
        .with_status(101)
        .with_stderr(
            "\
[UPDATING] `alternative` index
[DOWNLOADING] crates ...
[ERROR] failed to download from `http://[..]/dl/bar/0.0.1/download`

Caused by:
  failed to get successful HTTP response from `http://[..]/dl/bar/0.0.1/download` (127.0.0.1), got 401
  body:
  Unauthorized message from server.",
        )
        .run();
}

#[cargo_test]
fn anonymous_alt_registry() {
    // An alternative registry that requires auth, but is not in the config.
    let registry = RegistryBuilder::new()
        .alternative()
        .auth_required()
        .no_configure_token()
        .no_configure_registry()
        .http_index()
        .build();

    let p = make_project();
    cargo(&p, &format!("install --index {} bar", registry.index_url()))
        .with_status(101)
        .with_stderr(
            "\
[UPDATING] `[..]` index
[ERROR] no token found for `[..]`
consider setting up an alternate registry in Cargo's configuration
as described by https://doc.rust-lang.org/cargo/reference/registries.html

[registries]
my-registry = { index = \"[..]\" }

",
        )
        .run();
}

#[cargo_test]
fn login() {
    let _registry = RegistryBuilder::new()
        .alternative()
        .no_configure_token()
        .auth_required()
        .http_index()
        .build();

    let p = make_project();
    cargo(&p, "login --registry alternative")
        .with_stdout("please paste the token found on https://test-registry-login/me below")
        .with_stdin("sekrit")
        .run();
}

#[cargo_test]
fn login_existing_token() {
    let _registry = RegistryBuilder::new()
        .alternative()
        .auth_required()
        .http_index()
        .build();

    let p = make_project();
    cargo(&p, "login --registry alternative")
        .with_stdout("please paste the token found on file://[..]/me below")
        .with_stdin("sekrit")
        .run();
}

#[cargo_test]
fn duplicate_index() {
    let server = RegistryBuilder::new()
        .alternative()
        .no_configure_token()
        .auth_required()
        .build();
    let p = make_project();

    // Two alternative registries with the same index.
    cargo(&p, "build")
        .env(
            "CARGO_REGISTRIES_ALTERNATIVE1_INDEX",
            server.index_url().as_str(),
        )
        .env(
            "CARGO_REGISTRIES_ALTERNATIVE2_INDEX",
            server.index_url().as_str(),
        )
        .with_status(101)
        .with_stderr(
            "\
[UPDATING] `alternative` index
[ERROR] failed to download `bar v0.0.1 (registry `alternative`)`

Caused by:
  unable to get packages from source

Caused by:
  multiple registries are configured with the same index url \
  'registry+file://[..]/alternative-registry': alternative1, alternative2
",
        )
        .run();
}
