//! Tests for network configuration.

use crate::prelude::*;
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

    // Use libgit2 to ensure we exercise Cargo's retry handling
    p.cargo("check -v")
        .env("CARGO_NET_GIT_FETCH_WITH_CLI", "false")
        .with_status(101)
        .with_stderr_data(str![[r#"
...
[WARNING] spurious network error (1 try remaining): [..]
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

    // Use libgit2 to ensure we exercise Cargo's retry handling
    p.cargo("check -v -j 1")
        .env("CARGO_NET_GIT_FETCH_WITH_CLI", "false")
        .with_status(101)
        .with_stderr_data(str![[r#"
...
[WARNING] spurious network error (2 tries remaining): [..]
[WARNING] spurious network error (1 try remaining): [..]
...
"#]])
        .run();
}
