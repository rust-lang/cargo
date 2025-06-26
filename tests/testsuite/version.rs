//! Tests for displaying the cargo version.

use crate::prelude::*;
use crate::utils::cargo_process;
use cargo_test_support::project;

#[cargo_test]
fn simple() {
    let p = project().build();

    p.cargo("version")
        .with_stdout_data(&format!("cargo {}\n", cargo::version()))
        .run();

    p.cargo("--version")
        .with_stdout_data(&format!("cargo {}\n", cargo::version()))
        .run();

    p.cargo("-V")
        .with_stdout_data(&format!("cargo {}\n", cargo::version()))
        .run();
}

#[cargo_test]
fn version_works_without_rustc() {
    let p = project().build();
    p.cargo("version").env("PATH", "").run();
}

#[cargo_test]
fn version_works_with_bad_config() {
    let p = project()
        .file(".cargo/config.toml", "this is not toml")
        .build();
    p.cargo("version").run();
}

#[cargo_test]
fn version_works_with_bad_target_dir() {
    let p = project()
        .file(
            ".cargo/config.toml",
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
        .with_stdout_data(format!(
            "\
cargo {}
release: [..]
commit-hash: [..]
commit-date: [..]
host: [HOST_TARGET]
libgit2: [..] (sys:[..] [..])
libcurl: [..] (sys:[..] [..])
...
os: [..]
",
            cargo::version()
        ))
        .run();
}
