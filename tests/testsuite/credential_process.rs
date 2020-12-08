//! Tests for credential-process.

use cargo_test_support::paths::CargoPathExt;
use cargo_test_support::{basic_manifest, cargo_process, paths, project, registry, Project};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::thread;
use url::Url;

fn toml_bin(proj: &Project, name: &str) -> String {
    proj.bin(name).display().to_string().replace('\\', "\\\\")
}

#[cargo_test]
fn gated() {
    registry::init();

    paths::home().join(".cargo/credentials").rm_rf();

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
    registry::init();
    paths::home().join(".cargo/credentials").rm_rf();
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
    let server = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = server.local_addr().unwrap();
    let api_url = format!("http://{}", addr);

    registry::init_registry(
        registry::alt_registry_path(),
        registry::alt_dl_url(),
        Url::parse(&api_url).unwrap(),
        registry::alt_api_path(),
    );

    // API server that checks that the token is included correctly.
    let t = thread::spawn(move || {
        let mut conn = BufReader::new(server.accept().unwrap().0);
        let headers: Vec<_> = (&mut conn)
            .lines()
            .map(|s| s.unwrap())
            .take_while(|s| s.len() > 2)
            .map(|s| s.trim().to_string())
            .collect();
        assert!(headers
            .iter()
            .any(|header| header == "Authorization: sekrit"));
        conn.get_mut()
            .write_all(
                b"HTTP/1.1 200\r\n\
                  Content-Length: 33\r\n\
                  \r\n\
                  {\"ok\": true, \"msg\": \"completed!\"}\r\n",
            )
            .unwrap();
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
    (p, t)
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
    registry::init();
    // If both `credential-process` and `token` are specified, it will ignore
    // `credential-process`, so remove the default tokens.
    paths::home().join(".cargo/credentials").rm_rf();
    cargo::util::paths::append(
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

    cargo::util::paths::append(
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
    registry::init();
    // If both `credential-process` and `token` are specified, it will ignore
    // `credential-process`, so remove the default tokens.
    paths::home().join(".cargo/credentials").rm_rf();
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

    cargo::util::paths::append(
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
    registry::init();

    paths::home().join(".cargo/credentials").rm_rf();
    cargo::util::paths::append(
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
            &format!("\
[UPDATING] [..]
[ERROR] failed to execute `[..]libexec/cargo-credential-doesnotexist[EXE]` to store authentication token for registry `crates-io`

Caused by:
  {}
", cargo_test_support::no_such_file_err_msg()),
        )
        .run();
}

#[cargo_test]
fn invalid_token_output() {
    // Error when credential process does not output the expected format for a token.
    registry::init();
    paths::home().join(".cargo/credentials").rm_rf();
    let cred_proj = project()
        .at("cred_proj")
        .file("Cargo.toml", &basic_manifest("test-cred", "1.0.0"))
        .file("src/main.rs", r#"fn main() { print!("a\nb\n"); } "#)
        .build();
    cred_proj.cargo("build").run();

    cargo::util::paths::append(
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
