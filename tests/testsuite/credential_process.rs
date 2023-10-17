//! Tests for credential-process.

use cargo_test_support::registry::{Package, TestRegistry};
use cargo_test_support::{basic_manifest, cargo_process, paths, project, registry, Project};

fn toml_bin(proj: &Project, name: &str) -> String {
    proj.bin(name).display().to_string().replace('\\', "\\\\")
}

/// Setup for a test that will issue a command that needs to fetch a token.
///
/// This does the following:
///
/// * Spawn a thread that will act as an API server.
/// * Create a simple credential-process that will generate a fake token.
/// * Create a simple `foo` project to run the test against.
/// * Configure the credential-process config.
///
/// Returns the simple `foo` project to test against and the API server handle.
fn get_token_test() -> (Project, TestRegistry) {
    // API server that checks that the token is included correctly.
    let server = registry::RegistryBuilder::new()
        .no_configure_token()
        .token(cargo_test_support::registry::Token::Plaintext(
            "sekrit".to_string(),
        ))
        .alternative()
        .http_api()
        .http_index()
        .auth_required()
        .build();

    let provider = build_provider(
        "test-cred",
        r#"{"Ok":{"kind":"get","token":"sekrit","cache":"session","operation_independent":false}}"#,
    );

    let p = project()
        .file(
            ".cargo/config",
            &format!(
                r#"
                    [registries.alternative]
                    index = "{}"
                    credential-provider = ["{provider}"]
                "#,
                server.index_url(),
            ),
        )
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                description = "foo"
                license = "MIT"
                homepage = "https://example.com/"
            "#,
        )
        .file("src/lib.rs", "")
        .build();
    (p, server)
}

#[cargo_test]
fn publish() {
    // Checks that credential-process is used for `cargo publish`.
    let (p, _t) = get_token_test();

    p.cargo("publish --no-verify --registry alternative")
        .with_stderr(
            r#"[UPDATING] [..]
{"v":1,"registry":{"index-url":"[..]","name":"alternative","headers":[..]},"kind":"get","operation":"read"}
[PACKAGING] foo v0.1.0 [..]
[PACKAGED] [..]
{"v":1,"registry":{"index-url":"[..]","name":"alternative"},"kind":"get","operation":"publish","name":"foo","vers":"0.1.0","cksum":"[..]"}
[UPLOADING] foo v0.1.0 [..]
[UPLOADED] foo v0.1.0 [..]
note: Waiting [..]
You may press ctrl-c [..]
[PUBLISHED] foo v0.1.0 [..]
"#,
        )
        .run();
}

#[cargo_test]
fn basic_unsupported() {
    // Non-action commands don't support login/logout.
    let registry = registry::RegistryBuilder::new()
        .no_configure_token()
        .credential_provider(&["cargo:token-from-stdout", "false"])
        .build();

    cargo_process("login abcdefg")
        .replace_crates_io(registry.index_url())
        .with_status(101)
        .with_stderr(
            "\
[UPDATING] crates.io index
[ERROR] credential provider `cargo:token-from-stdout false` failed action `login`

Caused by:
  requested operation not supported
",
        )
        .run();

    cargo_process("logout")
        .replace_crates_io(registry.index_url())
        .with_status(101)
        .with_stderr(
            "\
[ERROR] credential provider `cargo:token-from-stdout false` failed action `logout`

Caused by:
  requested operation not supported
",
        )
        .run();
}

#[cargo_test]
fn login() {
    let registry = registry::RegistryBuilder::new()
        .no_configure_token()
        .credential_provider(&[
            &build_provider("test-cred", r#"{"Ok": {"kind": "login"}}"#),
            "cfg1",
            "--cfg2",
        ])
        .build();

    cargo_process("login abcdefg -- cmd3 --cmd4")
        .replace_crates_io(registry.index_url())
        .with_stderr(
            r#"[UPDATING] [..]
{"v":1,"registry":{"index-url":"https://github.com/rust-lang/crates.io-index","name":"crates-io"},"kind":"login","token":"abcdefg","login-url":"[..]","args":["cfg1","--cfg2","cmd3","--cmd4"]}
"#,
        )
        .run();
}

#[cargo_test]
fn logout() {
    let server = registry::RegistryBuilder::new()
        .no_configure_token()
        .credential_provider(&[&build_provider(
            "test-cred",
            r#"{"Ok": {"kind": "logout"}}"#,
        )])
        .build();

    cargo_process("logout")
        .replace_crates_io(server.index_url())
        .with_stderr(
            r#"{"v":1,"registry":{"index-url":"https://github.com/rust-lang/crates.io-index","name":"crates-io"},"kind":"logout"}
"#,
        )
        .run();
}

#[cargo_test]
fn yank() {
    let (p, _t) = get_token_test();

    p.cargo("yank --version 0.1.0 --registry alternative")
        .with_stderr(
            r#"[UPDATING] [..]
{"v":1,"registry":{"index-url":"[..]","name":"alternative","headers":[..]},"kind":"get","operation":"read"}
{"v":1,"registry":{"index-url":"[..]","name":"alternative"},"kind":"get","operation":"yank","name":"foo","vers":"0.1.0"}
[YANK] foo@0.1.0
"#,
        )
        .run();
}

#[cargo_test]
fn owner() {
    let (p, _t) = get_token_test();

    p.cargo("owner --add username --registry alternative")
        .with_stderr(
            r#"[UPDATING] [..]
{"v":1,"registry":{"index-url":"[..]","name":"alternative","headers":[..]},"kind":"get","operation":"read"}
{"v":1,"registry":{"index-url":"[..]","name":"alternative"},"kind":"get","operation":"owners","name":"foo"}
[OWNER] completed!
"#,
        )
        .run();
}

#[cargo_test]
fn invalid_token_output() {
    // Error when credential process does not output the expected format for a token.
    let cred_proj = project()
        .at("cred_proj")
        .file("Cargo.toml", &basic_manifest("test-cred", "1.0.0"))
        .file("src/main.rs", r#"fn main() { print!("a\nb\n"); } "#)
        .build();
    cred_proj.cargo("build").run();
    let _server = registry::RegistryBuilder::new()
        .alternative()
        .credential_provider(&[
            "cargo:token-from-stdout",
            &toml_bin(&cred_proj, "test-cred"),
        ])
        .no_configure_token()
        .build();

    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "1.0.0"))
        .file("src/lib.rs", "")
        .build();

    p.cargo("publish --no-verify --registry alternative")
        .with_status(101)
        .with_stderr(
            "\
[UPDATING] [..]
[ERROR] credential provider `[..]test-cred[EXE]` failed action `get`

Caused by:
  process `[..]` returned more than one line of output; expected a single token
",
        )
        .run();
}

/// Builds a credential provider that echos the request from cargo to stderr,
/// and prints the `response` to stdout.
fn build_provider(name: &str, response: &str) -> String {
    // The credential process to use.
    let cred_proj = project()
        .at(name)
        .file("Cargo.toml", &basic_manifest(name, "1.0.0"))
        .file(
            "src/main.rs",
            &r####"
                fn main() {
                    println!(r#"{{"v":[1]}}"#);
                    assert_eq!(std::env::args().skip(1).next().unwrap(), "--cargo-plugin");
                    let mut buffer = String::new();
                    std::io::stdin().read_line(&mut buffer).unwrap();
                    eprint!("{}", buffer);
                    use std::io::Write;
                    std::io::stdout().write_all(r###"[RESPONSE]"###.as_bytes()).unwrap();
                    println!();
                } "####
                .replace("[RESPONSE]", response),
        )
        .build();
    cred_proj.cargo("build").run();
    toml_bin(&cred_proj, name)
}

#[cargo_test]
fn not_found() {
    let registry = registry::RegistryBuilder::new()
        .no_configure_token()
        .http_index()
        .auth_required()
        .credential_provider(&[&build_provider(
            "not_found",
            r#"{"Err": {"kind": "not-found"}}"#,
        )])
        .build();

    // should not suggest a _TOKEN environment variable since the cargo:token provider isn't available.
    cargo_process("install -v foo")
        .replace_crates_io(registry.index_url())
        .with_status(101)
        .with_stderr(
            r#"[UPDATING] [..]
[CREDENTIAL] [..]not_found[..] get crates-io
{"v":1[..]
[ERROR] failed to query replaced source registry `crates-io`

Caused by:
  no token found, please run `cargo login`
"#,
        )
        .run();
}

#[cargo_test]
fn all_not_found() {
    let server = registry::RegistryBuilder::new()
        .no_configure_token()
        .auth_required()
        .http_index()
        .build();
    let not_found = build_provider("not_found", r#"{"Err": {"kind": "not-found"}}"#);
    cargo_util::paths::append(
        &paths::home().join(".cargo/config"),
        format!(
            r#"
                [registry]
                global-credential-providers = ["not_found"]
                [credential-alias]
                not_found = ["{not_found}"]
            "#,
        )
        .as_bytes(),
    )
    .unwrap();

    // should not suggest a _TOKEN environment variable since the cargo:token provider isn't available.
    cargo_process("install -v foo")
        .replace_crates_io(server.index_url())
        .with_status(101)
        .with_stderr(
            r#"[UPDATING] [..]
[CREDENTIAL] [..]not_found[..] get crates-io
{"v":1,"registry":{"index-url":"[..]","name":"crates-io","headers":[[..]"WWW-Authenticate: Cargo login_url=\"https://test-registry-login/me\""[..]]},"kind":"get","operation":"read"}
[ERROR] failed to query replaced source registry `crates-io`

Caused by:
  no token found, please run `cargo login`
"#,
        )
        .run();
}

#[cargo_test]
fn all_not_supported() {
    let server = registry::RegistryBuilder::new()
        .no_configure_token()
        .auth_required()
        .http_index()
        .build();
    let not_supported =
        build_provider("not_supported", r#"{"Err": {"kind": "url-not-supported"}}"#);
    cargo_util::paths::append(
        &paths::home().join(".cargo/config"),
        format!(
            r#"
                [registry]
                global-credential-providers = ["not_supported"]
                [credential-alias]
                not_supported = ["{not_supported}"]
            "#,
        )
        .as_bytes(),
    )
    .unwrap();

    cargo_process("install -v foo")
        .replace_crates_io(server.index_url())
        .with_status(101)
        .with_stderr(
            r#"[UPDATING] [..]
[CREDENTIAL] [..]not_supported[..] get crates-io
{"v":1,"registry":{"index-url":"[..]","name":"crates-io","headers":[[..]"WWW-Authenticate: Cargo login_url=\"https://test-registry-login/me\""[..]]},"kind":"get","operation":"read"}
[ERROR] failed to query replaced source registry `crates-io`

Caused by:
  no credential providers could handle the request
"#,
        )
        .run();
}

#[cargo_test]
fn multiple_providers() {
    let server = registry::RegistryBuilder::new()
        .no_configure_token()
        .build();

    // Set up two credential providers: the first will fail with "UrlNotSupported"
    // and Cargo should skip it. The second should succeed.
    let url_not_supported = build_provider(
        "url_not_supported",
        r#"{"Err": {"kind": "url-not-supported"}}"#,
    );

    let success_provider = build_provider("success_provider", r#"{"Ok": {"kind": "login"}}"#);

    cargo_util::paths::append(
        &paths::home().join(".cargo/config"),
        format!(
            r#"
                [registry]
                global-credential-providers = ["success_provider", "url_not_supported"]

                [credential-alias]
                success_provider = ["{success_provider}"]
                url_not_supported = ["{url_not_supported}"]
            "#,
        )
        .as_bytes(),
    )
    .unwrap();

    cargo_process("login -v abcdefg")
        .replace_crates_io(server.index_url())
        .with_stderr(
            r#"[UPDATING] [..]
[CREDENTIAL] [..]url_not_supported[..] login crates-io
{"v":1,"registry":{"index-url":"https://github.com/rust-lang/crates.io-index","name":"crates-io"},"kind":"login","token":"abcdefg","login-url":"[..]"}
[CREDENTIAL] [..]success_provider[..] login crates-io
{"v":1,"registry":{"index-url":"https://github.com/rust-lang/crates.io-index","name":"crates-io"},"kind":"login","token":"abcdefg","login-url":"[..]"}
"#,
        )
        .run();
}

#[cargo_test]
fn both_token_and_provider() {
    let server = registry::RegistryBuilder::new()
        .credential_provider(&["cargo:paseto"])
        .build();

    cargo_process("login -Z asymmetric-token")
        .masquerade_as_nightly_cargo(&["asymmetric-token"])
        .replace_crates_io(server.index_url())
        .with_stderr(
            r#"[UPDATING] [..]
[WARNING] registry `crates-io` has a token configured in [..] that will be ignored because this registry is configured to use credential-provider `cargo:paseto`
k3.public[..]
"#,
        )
        .run();
}

#[cargo_test]
fn registry_provider_overrides_global() {
    let server = registry::RegistryBuilder::new().build();
    cargo_util::paths::append(
        &paths::home().join(".cargo/config"),
        format!(
            r#"
                [registry]
                global-credential-providers = ["should-not-be-called"]
            "#,
        )
        .as_bytes(),
    )
    .unwrap();

    cargo_process("login -v abcdefg")
        .env("CARGO_REGISTRY_CREDENTIAL_PROVIDER", "cargo:token")
        .replace_crates_io(server.index_url())
        .with_stderr(
            r#"[UPDATING] [..]
[CREDENTIAL] cargo:token login crates-io
[LOGIN] token for `crates-io` saved
"#,
        )
        .run();
    let credentials =
        std::fs::read_to_string(paths::home().join(".cargo/credentials.toml")).unwrap();
    assert_eq!(credentials, "[registry]\ntoken = \"abcdefg\"\n");
}

#[cargo_test]
fn both_asymmetric_and_token() {
    let server = registry::RegistryBuilder::new().build();
    cargo_util::paths::append(
        &paths::home().join(".cargo/config"),
        format!(
            r#"
                [registry]
                token = "foo"
                secret-key = "bar"
            "#,
        )
        .as_bytes(),
    )
    .unwrap();

    cargo_process("login -Zasymmetric-token -v abcdefg")
        .masquerade_as_nightly_cargo(&["asymmetric-token"])
        .replace_crates_io(server.index_url())
        .with_stderr(
            r#"[UPDATING] [..]
[WARNING] registry `crates-io` has a `secret_key` configured in [..]config that will be ignored because a `token` is also configured, and the `cargo:token` provider is configured with higher precedence
[CREDENTIAL] cargo:token login crates-io
[LOGIN] token for `crates-io` saved
"#,
        )
        .run();
}

#[cargo_test]
fn token_caching() {
    let server = registry::RegistryBuilder::new()
        .no_configure_token()
        .no_configure_registry()
        .token(cargo_test_support::registry::Token::Plaintext(
            "sekrit".to_string(),
        ))
        .alternative()
        .http_api()
        .http_index()
        .build();

    // Token should not be re-used if it is expired
    let expired_provider = build_provider(
        "expired_provider",
        r#"{"Ok":{"kind":"get","token":"sekrit","cache":"expires","expiration":0,"operation_independent":true}}"#,
    );

    // Token should not be re-used for a different operation if it is not operation_independent
    let non_independent_provider = build_provider(
        "non_independent_provider",
        r#"{"Ok":{"kind":"get","token":"sekrit","cache":"session","operation_independent":false}}"#,
    );

    let p = project()
        .file(
            ".cargo/config",
            &format!(
                r#"
                    [registries.alternative]
                    index = "{}"
                    credential-provider = ["{expired_provider}"]
                "#,
                server.index_url(),
            ),
        )
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                description = "foo"
                license = "MIT"
                homepage = "https://example.com/"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    let output = r#"[UPDATING] `alternative` index
{"v":1,"registry":{"index-url":"[..]","name":"alternative"},"kind":"get","operation":"read"}
[PACKAGING] foo v0.1.0 [..]
[PACKAGED] [..]
{"v":1,"registry":{"index-url":"[..]","name":"alternative"},"kind":"get","operation":"publish","name":"foo","vers":"0.1.0","cksum":"[..]"}
[UPLOADING] foo v0.1.0 [..]
[UPLOADED] foo v0.1.0 [..]
note: Waiting [..]
You may press ctrl-c [..]
[PUBLISHED] foo v0.1.0 [..]
"#;

    // The output should contain two JSON messages from the provider in both cases:
    // The first because the credential is expired, the second because the provider
    // indicated that the token was non-operation-independent.
    p.cargo("publish --registry alternative --no-verify")
        .with_stderr(output)
        .run();

    p.change_file(
        ".cargo/config",
        &format!(
            r#"
                [registries.alternative]
                index = "{}"
                credential-provider = ["{non_independent_provider}"]
            "#,
            server.index_url(),
        ),
    );

    p.cargo("publish --registry alternative --no-verify")
        .with_stderr(output)
        .run();
}

#[cargo_test]
fn basic_provider() {
    let cred_proj = project()
        .at("cred_proj")
        .file("Cargo.toml", &basic_manifest("test-cred", "1.0.0"))
        .file("src/main.rs", r#"fn main() {
            eprintln!("CARGO={:?}", std::env::var("CARGO").ok());
            eprintln!("CARGO_REGISTRY_NAME_OPT={:?}", std::env::var("CARGO_REGISTRY_NAME_OPT").ok());
            eprintln!("CARGO_REGISTRY_INDEX_URL={:?}", std::env::var("CARGO_REGISTRY_INDEX_URL").ok());
            print!("sekrit"); 
        }"#)
        .build();
    cred_proj.cargo("build").run();

    let _server = registry::RegistryBuilder::new()
        .no_configure_token()
        .credential_provider(&[
            "cargo:token-from-stdout",
            &toml_bin(&cred_proj, "test-cred"),
        ])
        .token(cargo_test_support::registry::Token::Plaintext(
            "sekrit".to_string(),
        ))
        .alternative()
        .http_api()
        .auth_required()
        .build();

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

    p.cargo("check")
        .with_stderr(
            "\
[UPDATING] `alternative` index
CARGO=Some([..])
CARGO_REGISTRY_NAME_OPT=Some(\"alternative\")
CARGO_REGISTRY_INDEX_URL=Some([..])
[DOWNLOADING] crates ...
[DOWNLOADED] bar v0.0.1 (registry `alternative`)
[CHECKING] bar v0.0.1 (registry `alternative`)
[CHECKING] foo v0.0.1 ([..])
[FINISHED] [..]
",
        )
        .run();
}

#[cargo_test]
fn unsupported_version() {
    let cred_proj = project()
        .at("new-vers")
        .file("Cargo.toml", &basic_manifest("new-vers", "1.0.0"))
        .file(
            "src/main.rs",
            &r####"
                fn main() {
                    println!(r#"{{"v":[998, 999]}}"#);
                    assert_eq!(std::env::args().skip(1).next().unwrap(), "--cargo-plugin");
                    let mut buffer = String::new();
                    std::io::stdin().read_line(&mut buffer).unwrap();
                    std::thread::sleep(std::time::Duration::from_secs(1));
                    panic!("child process should have been killed before getting here");
                } "####,
        )
        .build();
    cred_proj.cargo("build").run();
    let provider = toml_bin(&cred_proj, "new-vers");

    let registry = registry::RegistryBuilder::new()
        .no_configure_token()
        .credential_provider(&[&provider])
        .build();

    cargo_process("login abcdefg")
        .replace_crates_io(registry.index_url())
        .with_status(101)
        .with_stderr(
            r#"[UPDATING] [..]
[ERROR] credential provider `[..]` failed action `login`

Caused by:
  credential provider supports protocol versions [998, 999], while Cargo supports [1]
"#,
        )
        .run();
}

#[cargo_test]
fn alias_builtin_warning() {
    let registry = registry::RegistryBuilder::new()
        .credential_provider(&[&"cargo:token"])
        .build();

    cargo_util::paths::append(
        &paths::home().join(".cargo/config"),
        format!(
            r#"
                [credential-alias]
                "cargo:token" = ["ignored"]
            "#,
        )
        .as_bytes(),
    )
    .unwrap();

    cargo_process("login abcdefg")
        .replace_crates_io(registry.index_url())
        .with_stderr(
            r#"[UPDATING] [..]
[WARNING] credential-alias `cargo:token` (defined in `[..]`) will be ignored because it would shadow a built-in credential-provider
[LOGIN] token for `crates-io` saved
"#,
        )
        .run();
}
