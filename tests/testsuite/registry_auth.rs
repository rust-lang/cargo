//! Tests for registry authentication.

use cargo_test_support::compare::match_contains;
use cargo_test_support::registry::{Package, RegistryBuilder, Token};
use cargo_test_support::{project, Execs, Project};

fn cargo(p: &Project, s: &str) -> Execs {
    let mut e = p.cargo(s);
    e.masquerade_as_nightly_cargo(&["asymmetric-token"])
        .arg("-Zasymmetric-token");
    e.env(
        "CARGO_REGISTRY_GLOBAL_CREDENTIAL_PROVIDERS",
        "cargo:paseto cargo:token",
    );
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
fn requires_credential_provider() {
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
error: failed to download `bar v0.0.1 (registry `alternative`)`

Caused by:
  unable to get packages from source

Caused by:
  authenticated registries require a credential-provider to be configured
  see https://doc.rust-lang.org/cargo/reference/registry-authentication.html for details"#,
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

#[cargo_test]
fn token_not_logged() {
    // Checks that the token isn't displayed in debug output (for both HTTP
    // index and registry API). Note that this doesn't fully verify the
    // correct behavior since we don't have an HTTP2 server, and curl behaves
    // significantly differently when using HTTP2.
    let crates_io = RegistryBuilder::new()
        .http_api()
        .http_index()
        .auth_required()
        .token(Token::Plaintext("a-unique_token".to_string()))
        .build();
    Package::new("bar", "1.0.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [dependencies]
                bar = "1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .build();
    let output = cargo(&p, "publish")
        .replace_crates_io(crates_io.index_url())
        .env("CARGO_HTTP_DEBUG", "true")
        .env("CARGO_LOG", "trace")
        .exec_with_output()
        .unwrap();
    let log = String::from_utf8(output.stderr).unwrap();
    let lines = "\
[UPDATING] crates.io index
[PACKAGING] foo v0.1.0 [..]
[VERIFYING] foo v0.1.0 [..]
[DOWNLOADING] crates ...
[DOWNLOADED] bar v1.0.0
[COMPILING] bar v1.0.0
[COMPILING] foo v0.1.0 [..]
[FINISHED] [..]
[PACKAGED] 3 files[..]
[UPLOADING] foo v0.1.0[..]
[UPLOADED] foo v0.1.0 to registry `crates-io`
note: Waiting [..]
";
    for line in lines.lines() {
        match_contains(line, &log, None).unwrap();
    }
    let authorizations: Vec<_> = log
        .lines()
        .filter(|line| {
            line.contains("http-debug:") && line.to_lowercase().contains("authorization")
        })
        .collect();
    assert!(authorizations.iter().all(|line| line.contains("REDACTED")));
    // Total authorizations:
    // 1. Initial config.json
    // 2. config.json again for verification
    // 3. /index/3/b/bar
    // 4. /dl/bar/1.0.0/download
    // 5. /api/v1/crates/new
    // 6. config.json for the "wait for publish"
    // 7. /index/3/f/foo for the "wait for publish"
    assert_eq!(authorizations.len(), 7);
    assert!(!log.contains("a-unique_token"));
}
