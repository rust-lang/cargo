//! Tests for credential-process.

use cargo_test_support::{basic_manifest, cargo_process, paths, project, registry, Project};
use std::fs;
use std::thread;

fn toml_bin(proj: &Project, name: &str) -> String {
    proj.bin(name).display().to_string().replace('\\', "\\\\")
}

#[cargo_test]
fn gated() {
    registry::RegistryBuilder::new()
        .alternative(true)
        .add_tokens(false)
        .build();

    let p = project()
        .file(
            ".cargo/config",
            r#"
                [registry]
                credential-process = "false"
            "#,
        )
        .file("Cargo.toml", &basic_manifest("foo", "1.0.0"))
        .file("src/lib.rs", "")
        .build();

    p.cargo("publish --no-verify")
        .masquerade_as_nightly_cargo()
        .with_status(101)
        .with_stderr(
            "\
[UPDATING] [..]
[ERROR] no upload token found, please run `cargo login` or pass `--token`
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
        .masquerade_as_nightly_cargo()
        .with_status(101)
        .with_stderr(
            "\
[UPDATING] [..]
[ERROR] no upload token found, please run `cargo login` or pass `--token`
",
        )
        .run();
}

#[cargo_test]
fn warn_both_token_and_process() {
    // Specifying both credential-process and a token in config should issue a warning.
    registry::RegistryBuilder::new()
        .alternative(true)
        .add_tokens(false)
        .build();
    let p = project()
        .file(
            ".cargo/config",
            r#"
                [registries.alternative]
                token = "sekrit"
                credential-process = "false"
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

    p.cargo("publish --no-verify --registry alternative -Z credential-process")
        .masquerade_as_nightly_cargo()
        .with_status(101)
        .with_stderr(
            "\
[ERROR] both `registries.alternative.token` and `registries.alternative.credential-process` \
were specified in the config\n\
Only one of these values may be set, remove one or the other to proceed.
",
        )
        .run();

    // Try with global credential-process, and registry-specific `token`.
    // This should silently use the config token, and not run the "false" exe.
    p.change_file(
        ".cargo/config",
        r#"
            [registry]
            credential-process = "false"

            [registries.alternative]
            token = "sekrit"
        "#,
    );
    p.cargo("publish --no-verify --registry alternative -Z credential-process")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[UPDATING] [..]
[PACKAGING] foo v0.1.0 [..]
[UPLOADING] foo v0.1.0 [..]
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
/// Returns a thread handle for the API server, the test should join it when
/// finished. Also returns the simple `foo` project to test against.
fn get_token_test() -> (Project, thread::JoinHandle<()>) {
    // API server that checks that the token is included correctly.
    let server = registry::RegistryBuilder::new()
        .add_tokens(false)
        .build_api_server(&|headers| {
            assert!(headers
                .iter()
                .any(|header| header == "Authorization: sekrit"));

            (200, &r#"{"ok": true, "msg": "completed!"}"#)
        });

    // The credential process to use.
    let cred_proj = project()
        .at("cred_proj")
        .file("Cargo.toml", &basic_manifest("test-cred", "1.0.0"))
        .file("src/main.rs", r#"fn main() { println!("sekrit"); } "#)
        .build();
    cred_proj.cargo("build").run();

    let p = project()
        .file(
            ".cargo/config",
            &format!(
                r#"
                    [registries.alternative]
                    index = "{}"
                    credential-process = ["{}"]
                "#,
                registry::alt_registry_url(),
                toml_bin(&cred_proj, "test-cred")
            ),
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
    (p, server)
}

#[cargo_test]
fn publish() {
    // Checks that credential-process is used for `cargo publish`.
    let (p, t) = get_token_test();

    p.cargo("publish --no-verify --registry alternative -Z credential-process")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[UPDATING] [..]
[PACKAGING] foo v0.1.0 [..]
[UPLOADING] foo v0.1.0 [..]
",
        )
        .run();

    t.join().ok().unwrap();
}

#[cargo_test]
fn basic_unsupported() {
    // Non-action commands don't support login/logout.
    registry::RegistryBuilder::new().add_tokens(false).build();
    cargo_util::paths::append(
        &paths::home().join(".cargo/config"),
        br#"
            [registry]
            credential-process = "false"
        "#,
    )
    .unwrap();

    cargo_process("login -Z credential-process abcdefg")
        .masquerade_as_nightly_cargo()
        .with_status(101)
        .with_stderr(
            "\
[UPDATING] [..]
[ERROR] credential process `false` cannot be used to log in, \
the credential-process configuration value must pass the \
`{action}` argument in the config to support this command
",
        )
        .run();

    cargo_process("logout -Z credential-process")
        .masquerade_as_nightly_cargo()
        .with_status(101)
        .with_stderr(
            "\
[ERROR] credential process `false` cannot be used to log out, \
the credential-process configuration value must pass the \
`{action}` argument in the config to support this command
",
        )
        .run();
}

#[cargo_test]
fn login() {
    registry::init();
    // The credential process to use.
    let cred_proj = project()
        .at("cred_proj")
        .file("Cargo.toml", &basic_manifest("test-cred", "1.0.0"))
        .file(
            "src/main.rs",
            &r#"
                use std::io::Read;

                fn main() {
                    assert_eq!(std::env::var("CARGO_REGISTRY_NAME").unwrap(), "crates-io");
                    assert_eq!(std::env::var("CARGO_REGISTRY_API_URL").unwrap(), "__API__");
                    assert_eq!(std::env::args().skip(1).next().unwrap(), "store");
                    let mut buffer = String::new();
                    std::io::stdin().read_to_string(&mut buffer).unwrap();
                    assert_eq!(buffer, "abcdefg\n");
                    std::fs::write("token-store", buffer).unwrap();
                }
            "#
            .replace("__API__", &registry::api_url().to_string()),
        )
        .build();
    cred_proj.cargo("build").run();

    cargo_util::paths::append(
        &paths::home().join(".cargo/config"),
        format!(
            r#"
                [registry]
                credential-process = ["{}", "{{action}}"]
            "#,
            toml_bin(&cred_proj, "test-cred")
        )
        .as_bytes(),
    )
    .unwrap();

    cargo_process("login -Z credential-process abcdefg")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[UPDATING] [..]
[LOGIN] token for `crates.io` saved
",
        )
        .run();
    assert_eq!(
        fs::read_to_string(paths::root().join("token-store")).unwrap(),
        "abcdefg\n"
    );
}

#[cargo_test]
fn logout() {
    registry::RegistryBuilder::new().add_tokens(false).build();
    // The credential process to use.
    let cred_proj = project()
        .at("cred_proj")
        .file("Cargo.toml", &basic_manifest("test-cred", "1.0.0"))
        .file(
            "src/main.rs",
            r#"
                use std::io::Read;

                fn main() {
                    assert_eq!(std::env::var("CARGO_REGISTRY_NAME").unwrap(), "crates-io");
                    assert_eq!(std::env::args().skip(1).next().unwrap(), "erase");
                    std::fs::write("token-store", "").unwrap();
                    eprintln!("token for `{}` has been erased!",
                        std::env::var("CARGO_REGISTRY_NAME").unwrap());
                }
            "#,
        )
        .build();
    cred_proj.cargo("build").run();

    cargo_util::paths::append(
        &paths::home().join(".cargo/config"),
        format!(
            r#"
                [registry]
                credential-process = ["{}", "{{action}}"]
            "#,
            toml_bin(&cred_proj, "test-cred")
        )
        .as_bytes(),
    )
    .unwrap();

    cargo_process("logout -Z credential-process")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[UPDATING] [..]
token for `crates-io` has been erased!
[LOGOUT] token for `crates.io` has been removed from local storage
",
        )
        .run();
    assert_eq!(
        fs::read_to_string(paths::root().join("token-store")).unwrap(),
        ""
    );
}

#[cargo_test]
fn yank() {
    let (p, t) = get_token_test();

    p.cargo("yank --vers 0.1.0 --registry alternative -Z credential-process")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[UPDATING] [..]
[YANK] foo:0.1.0
",
        )
        .run();

    t.join().ok().unwrap();
}

#[cargo_test]
fn owner() {
    let (p, t) = get_token_test();

    p.cargo("owner --add username --registry alternative -Z credential-process")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[UPDATING] [..]
[OWNER] completed!
",
        )
        .run();

    t.join().ok().unwrap();
}

#[cargo_test]
fn libexec_path() {
    // cargo: prefixed names use the sysroot
    registry::RegistryBuilder::new().add_tokens(false).build();
    cargo_util::paths::append(
        &paths::home().join(".cargo/config"),
        br#"
            [registry]
            credential-process = "cargo:doesnotexist"
        "#,
    )
    .unwrap();

    cargo_process("login -Z credential-process abcdefg")
        .masquerade_as_nightly_cargo()
        .with_status(101)
        .with_stderr(
            // FIXME: Update "Caused by" error message once rust/pull/87704 is merged.
            // On Windows, changing to a custom executable resolver has changed the
            // error messages.
            &format!("\
[UPDATING] [..]
[ERROR] failed to execute `[..]libexec/cargo-credential-doesnotexist[EXE]` to store authentication token for registry `crates-io`

Caused by:
  [..]
"),
        )
        .run();
}

#[cargo_test]
fn invalid_token_output() {
    // Error when credential process does not output the expected format for a token.
    registry::RegistryBuilder::new()
        .alternative(true)
        .add_tokens(false)
        .build();
    let cred_proj = project()
        .at("cred_proj")
        .file("Cargo.toml", &basic_manifest("test-cred", "1.0.0"))
        .file("src/main.rs", r#"fn main() { print!("a\nb\n"); } "#)
        .build();
    cred_proj.cargo("build").run();

    cargo_util::paths::append(
        &paths::home().join(".cargo/config"),
        format!(
            r#"
                [registry]
                credential-process = ["{}"]
            "#,
            toml_bin(&cred_proj, "test-cred")
        )
        .as_bytes(),
    )
    .unwrap();

    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "1.0.0"))
        .file("src/lib.rs", "")
        .build();

    p.cargo("publish --no-verify --registry alternative -Z credential-process")
        .masquerade_as_nightly_cargo()
        .with_status(101)
        .with_stderr(
            "\
[UPDATING] [..]
[ERROR] credential process `[..]test-cred[EXE]` returned more than one line of output; expected a single token
",
        )
        .run();
}
