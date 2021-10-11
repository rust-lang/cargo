//! Tests for network configuration.

use cargo_test_support::project;

#[cargo_test]
fn net_retry_loads_from_config() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies.bar]
                git = "http://127.0.0.1:11/foo/bar"
            "#,
        )
        .file("src/main.rs", "")
        .file(
            ".cargo/config",
            r#"
           [net]
           retry=1
           retry-delay="10ms"
           [http]
           timeout=1
            "#,
        )
        .build();

    p.cargo("build -v")
        .with_status(101)
        .with_stderr_contains(
            "[WARNING] spurious network error \
             (1 tries remaining): [..]",
        )
        .run();
}

#[cargo_test]
fn net_retry_git_outputs_warning() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies.bar]
                git = "http://127.0.0.1:11/foo/bar"
            "#,
        )
        .file(
            ".cargo/config",
            r#"
           [net]
           retry-delay= "10ms"
           [http]
           timeout=1
            "#,
        )
        .file("src/main.rs", "")
        .build();

    p.cargo("build -v -j 1")
        .with_status(101)
        .with_stderr_contains(
            "[WARNING] spurious network error \
             (2 tries remaining): [..]",
        )
        .with_stderr_contains("[WARNING] spurious network error (1 tries remaining): [..]")
        .run();
}

#[cargo_test]
fn net_retry_backoff_time() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies.bar]
                git = "http://127.0.0.1:11/foo/bar"
            "#,
        )
        .file(
            ".cargo/config",
            r#"
           [net]
           retry-delay="10ms"
           [http]
           timeout=1
            "#,
        )
        .file("src/main.rs", "")
        .build();

    p.cargo("build -v -j 1")
        .with_status(101)
        .with_stderr_contains("[..] backing off for 10 ms")
        .env("CARGO_LOG", "DEBUG")
        .run();
}

#[cargo_test]
fn net_retry_config_bad_unit() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies.bar]
                git = "http://127.0.0.1:11/foo/bar"
            "#,
        )
        .file(
            ".cargo/config",
            r#"
           [net]
           retry-delay="10m"
           [http]
           timeout=1
            "#,
        )
        .file("src/main.rs", "")
        .build();

    p.cargo("build -v -j 1")
        .with_status(101)
        .with_stderr_contains("[..] unknown variant `m`, expected `s` or `ms`")
        .run();
}

#[cargo_test]
fn net_retry_config_bad_format() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies.bar]
                git = "http://127.0.0.1:11/foo/bar"
            "#,
        )
        .file(
            ".cargo/config",
            r#"
           [net]
           retry-delay="10m1"
           [http]
           timeout=1
            "#,
        )
        .file("src/main.rs", "")
        .build();

    p.cargo("build -v -j 1")
        .with_status(101)
        .with_stderr_contains("[..] unknown variant `m1`, expected `s` or `ms`")
        .run();
}

#[cargo_test]
fn net_retry_config_bad_format2() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies.bar]
                git = "http://127.0.0.1:11/foo/bar"
            "#,
        )
        .file(
            ".cargo/config",
            r#"
           [net]
           retry-delay="m10"
           [http]
           timeout=1
            "#,
        )
        .file("src/main.rs", "")
        .build();

    p.cargo("build -v -j 1")
        .with_status(101)
        .with_stderr_contains(
            "[..] invalid value format: `m10`, expecting a non-negative number followed by unit suffix",
        )
        .run();
}

#[cargo_test]
fn net_retry_config_bad_format3() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies.bar]
                git = "http://127.0.0.1:11/foo/bar"
            "#,
        )
        .file(
            ".cargo/config",
            r#"
           [net]
           retry-delay="10"
           [http]
           timeout=1
            "#,
        )
        .file("src/main.rs", "")
        .build();

    p.cargo("build -v -j 1")
        .with_status(101)
        .with_stderr_contains(
            "[..] no unit is found on value: `10`, expected an `s` or `ms` suffix",
        )
        .run();
}
