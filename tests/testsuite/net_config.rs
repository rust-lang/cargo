//! Tests for network configuration.

use cargo_test_support::project;
use cargo_test_support::str;

#[cargo_test]
fn net_retry_loads_from_config() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies.bar]
                git = "http://127.0.0.1:11/foo/bar"
            "#,
        )
        .file("src/main.rs", "")
        .file(
            ".cargo/config.toml",
            r#"
           [net]
           retry=1
           [http]
           timeout=1
            "#,
        )
        .build();

    p.cargo("check -v")
        .with_status(101)
        .with_stderr_data(str![[r#"
...
[WARNING] spurious network error (1 tries remaining): [7] Couldn't connect to server (Failed to connect to 127.0.0.1 port 11 after [..] ms: Couldn't connect to server)[..]
...
"#]])
        .run();
}

#[cargo_test]
fn net_retry_git_outputs_warning() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies.bar]
                git = "http://127.0.0.1:11/foo/bar"
            "#,
        )
        .file(
            ".cargo/config.toml",
            r#"
           [http]
           timeout=1
            "#,
        )
        .file("src/main.rs", "")
        .build();

    p.cargo("check -v -j 1")
        .with_status(101)
        .with_stderr_data(str![[r#"
...
[WARNING] spurious network error (2 tries remaining): [7] Couldn't connect to server (Failed to connect to 127.0.0.1 port 11 after [..] ms: Couldn't connect to server)[..]
[WARNING] spurious network error (1 tries remaining): [7] Couldn't connect to server (Failed to connect to 127.0.0.1 port 11 after [..] ms: Couldn't connect to server)[..]
...
"#]])
        .run();
}
