//! Tests for credential-process.

use cargo_test_support::registry::{Package, TestRegistry};
use cargo_test_support::{basic_manifest, cargo_process, paths, project, registry, Project};

fn toml_bin(proj: &Project, name: &str) -> String {
    proj.bin(name).display().to_string().replace('\\', "\\\\")
}

#[cargo_test]
fn gated() {
    let _alternative = registry::RegistryBuilder::new()
        .alternative()
        .no_configure_token()
        .build();

    let cratesio = registry::RegistryBuilder::new()
        .no_configure_token()
        .build();

    let p = project()
        .file(
            ".cargo/config",
            r#"
                [registry]
                credential-provider = ["false"]
            "#,
        )
        .file("Cargo.toml", &basic_manifest("foo", "1.0.0"))
        .file("src/lib.rs", "")
        .build();

    p.cargo("publish --no-verify")
        .replace_crates_io(cratesio.index_url())
        .masquerade_as_nightly_cargo(&["credential-process"])
        .with_status(101)
        .with_stderr(
            "\
[UPDATING] [..]
[ERROR] no token found, please run `cargo login`
or use environment variable CARGO_REGISTRY_TOKEN
",
        )
        .run();

    p.change_file(
        ".cargo/config",
        r#"
            [registry.alternative]
            credential-process = "false"
        "#,
    );

    p.cargo("publish --no-verify --registry alternative")
        .masquerade_as_nightly_cargo(&["credential-process"])
        .with_status(101)
        .with_stderr(
            "\
[UPDATING] [..]
[ERROR] no token found for `alternative`, please run `cargo login --registry alternative`
or use environment variable CARGO_REGISTRIES_ALTERNATIVE_TOKEN
",
        )
        .run();
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

    p.cargo("publish --no-verify --registry alternative -Z credential-process -Z registry-auth")
        .masquerade_as_nightly_cargo(&["credential-process"])
        .with_stderr(
            r#"[UPDATING] [..]
{"v":1,"registry":{"index-url":"[..]","name":"alternative","headers":[..]},"kind":"get","operation":"read","args":[]}
[PACKAGING] foo v0.1.0 [..]
[PACKAGED] [..]
{"v":1,"registry":{"index-url":"[..]","name":"alternative"},"kind":"get","operation":"publish","name":"foo","vers":"0.1.0","cksum":"[..]","args":[]}
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
        .credential_provider(&["cargo:basic", "false"])
        .build();

    cargo_process("login -Z credential-process abcdefg")
        .replace_crates_io(registry.index_url())
        .masquerade_as_nightly_cargo(&["credential-process"])
        .with_status(101)
        .with_stderr(
            "\
[UPDATING] crates.io index
[ERROR] credential provider `cargo:basic false` failed action `login`

Caused by:
  credential provider does not support the requested operation
",
        )
        .run();

    cargo_process("logout -Z credential-process")
        .replace_crates_io(registry.index_url())
        .masquerade_as_nightly_cargo(&["credential-process"])
        .with_status(101)
        .with_stderr(
            "\
[ERROR] credential provider `cargo:basic false` failed action `logout`

Caused by:
  credential provider does not support the requested operation
",
        )
        .run();
}

#[cargo_test]
fn login() {
    let registry = registry::RegistryBuilder::new()
        .no_configure_token()
        .credential_provider(&[&build_provider("test-cred", r#"{"Ok": {"kind": "login"}}"#)])
        .build();

    cargo_process("login -Z credential-process abcdefg")
        .masquerade_as_nightly_cargo(&["credential-process"])
        .replace_crates_io(registry.index_url())
        .with_stderr(
            r#"[UPDATING] [..]
{"v":1,"registry":{"index-url":"https://github.com/rust-lang/crates.io-index","name":"crates-io"},"kind":"login","token":"abcdefg","login-url":"[..]","args":[]}
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

    cargo_process("logout -Z credential-process")
        .masquerade_as_nightly_cargo(&["credential-process"])
        .replace_crates_io(server.index_url())
        .with_stderr(
            r#"{"v":1,"registry":{"index-url":"https://github.com/rust-lang/crates.io-index","name":"crates-io"},"kind":"logout","args":[]}
"#,
        )
        .run();
}

#[cargo_test]
fn yank() {
    let (p, _t) = get_token_test();

    p.cargo("yank --version 0.1.0 --registry alternative -Zcredential-process -Zregistry-auth")
        .masquerade_as_nightly_cargo(&["credential-process"])
        .with_stderr(
            r#"[UPDATING] [..]
{"v":1,"registry":{"index-url":"[..]","name":"alternative","headers":[..]},"kind":"get","operation":"read","args":[]}
{"v":1,"registry":{"index-url":"[..]","name":"alternative"},"kind":"get","operation":"yank","name":"foo","vers":"0.1.0","args":[]}
[YANK] foo@0.1.0
"#,
        )
        .run();
}

#[cargo_test]
fn owner() {
    let (p, _t) = get_token_test();

    p.cargo("owner --add username --registry alternative -Zcredential-process -Zregistry-auth")
        .masquerade_as_nightly_cargo(&["credential-process"])
        .with_stderr(
            r#"[UPDATING] [..]
{"v":1,"registry":{"index-url":"[..]","name":"alternative","headers":[..]},"kind":"get","operation":"read","args":[]}
{"v":1,"registry":{"index-url":"[..]","name":"alternative"},"kind":"get","operation":"owners","name":"foo","args":[]}
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
        .credential_provider(&["cargo:basic", &toml_bin(&cred_proj, "test-cred")])
        .no_configure_token()
        .build();

    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "1.0.0"))
        .file("src/lib.rs", "")
        .build();

    p.cargo("publish --no-verify --registry alternative -Z credential-process")
        .masquerade_as_nightly_cargo(&["credential-process"])
        .with_status(101)
        .with_stderr(
            "\
[UPDATING] [..]
[ERROR] credential provider `[..]test-cred[EXE]` failed action `get`

Caused by:
  error: process `[..]` returned more than one line of output; expected a single token
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

    cargo_process("login -Z credential-process -v abcdefg")
        .masquerade_as_nightly_cargo(&["credential-process"])
        .replace_crates_io(server.index_url())
        .with_stderr(
            r#"[UPDATING] [..]
[CREDENTIAL] [..]url_not_supported[..] login crates-io
{"v":1,"registry":{"index-url":"https://github.com/rust-lang/crates.io-index","name":"crates-io"},"kind":"login","token":"abcdefg","login-url":"[..]","args":[]}
[CREDENTIAL] [..]success_provider[..] login crates-io
{"v":1,"registry":{"index-url":"https://github.com/rust-lang/crates.io-index","name":"crates-io"},"kind":"login","token":"abcdefg","login-url":"[..]","args":[]}
"#,
        )
        .run();
}

#[cargo_test]
fn both_token_and_provider() {
    let server = registry::RegistryBuilder::new().build();
    cargo_util::paths::append(
        &paths::home().join(".cargo/config"),
        format!(
            r#"
                [registry]
                credential-provider = ["cargo:token"]
            "#,
        )
        .as_bytes(),
    )
    .unwrap();

    cargo_process("login -Z credential-process -v abcdefg")
        .masquerade_as_nightly_cargo(&["credential-process"])
        .replace_crates_io(server.index_url())
        .with_stderr(
            r#"[UPDATING] [..]
[WARNING] registry `crates-io` has a token configured in [..]credentials.toml that will be ignored because a credential-provider is configured for this registry`
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

    cargo_process("login -Z credential-process -v abcdefg")
        .masquerade_as_nightly_cargo(&["credential-process"])
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
        "test-cred",
        r#"{"Ok":{"kind":"get","token":"sekrit","cache":{"expires":0},"operation_independent":true}}"#,
    );

    // Token should not be re-used for a different operation if it is not operation_independent
    let non_independent_provider = build_provider(
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
{"v":1,"registry":{"index-url":"[..]","name":"alternative"},"kind":"get","operation":"read","args":[]}
[PACKAGING] foo v0.1.0 [..]
[PACKAGED] [..]
{"v":1,"registry":{"index-url":"[..]","name":"alternative"},"kind":"get","operation":"publish","name":"foo","vers":"0.1.0","cksum":"[..]","args":[]}
[UPLOADING] foo v0.1.0 [..]
[UPLOADED] foo v0.1.0 [..]
note: Waiting [..]
You may press ctrl-c [..]
[PUBLISHED] foo v0.1.0 [..]
"#;

    // The output should contain two JSON messages from the provider in boths cases:
    // The first because the credential is expired, the second because the provider
    // indicated that the token was non-operation-independent.
    p.cargo("publish -Z credential-process --registry alternative --no-verify")
        .masquerade_as_nightly_cargo(&["credential-process"])
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

    p.cargo("publish -Z credential-process --registry alternative --no-verify")
        .masquerade_as_nightly_cargo(&["credential-process"])
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
        .credential_provider(&["cargo:basic", &toml_bin(&cred_proj, "test-cred")])
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

    p.cargo("check -Z credential-process -Z registry-auth")
        .masquerade_as_nightly_cargo(&["credential-process", "registry-auth"])
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
