//! Tests for displaying the cargo version.

use cargo_test_support::{cargo_process, project};

#[cargo_test]
fn simple() {
    let p = project().build();

    p.cargo("version")
        .with_stdout(&format!("cargo {}\n", cargo::version()))
        .run();

    p.cargo("--version")
        .with_stdout(&format!("cargo {}\n", cargo::version()))
        .run();

    p.cargo("-V")
        .with_stdout(&format!("cargo {}\n", cargo::version()))
        .run();
}

#[cargo_test]
fn version_works_without_rustc() {
    let p = project().build();
    p.cargo("version").env("PATH", "").run();
}

#[cargo_test]
fn version_works_with_bad_config() {
    let p = project().file(".cargo/config", "this is not toml").build();
    p.cargo("version").run();
}

#[cargo_test]
fn version_works_with_bad_target_dir() {
    let p = project()
        .file(
            ".cargo/config",
            r#"
                [build]
                target-dir = 4
            "#,
        )
        .build();
    p.cargo("version").run();
}

#[cargo_test]
fn verbose() {
    // This is mainly to check that it doesn't explode.
    cargo_process("-vV")
        .with_stdout_contains(&format!("cargo {}", cargo::version()))
        .with_stdout_contains("host: [..]")
        .with_stdout_contains("libgit2: [..]")
        .with_stdout_contains("libcurl: [..]")
        .with_stdout_contains("os: [..]")
        .run();
}
