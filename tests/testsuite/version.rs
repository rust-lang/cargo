//! Tests for displaying the cargo version.

use cargo_test_support::prelude::*;
use cargo_test_support::{cargo_process, project};

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

#[cargo_test]
fn version_with_corrupted_config_dir() {
    let p = project()
        .file(
            ".cargo/config.toml",
            "[[[[invalid]]]]\nkey = \u{FFFF}", // Invalid TOML and invalid Unicode
        )
        .build();

    // Should still work even with corrupted config
    p.cargo("version")
        .with_stdout_data(&format!("cargo {}\n", cargo::version()))
        .run();
}

#[cargo_test]
fn version_with_custom_format() {
    let p = project()
        .file(
            ".cargo/config.toml",
            r#"
            [term]
            verbose = true
            "#,
        )
        .build();

    // Test with invalid flag
    p.cargo("version --invalid-flag")
        .with_status(1)
        .with_stderr_data(
            "[ERROR] unexpected argument '--invalid-flag' found\n\n\
             Usage: cargo[EXE] version [OPTIONS]\n\n\
             For more information, try '--help'.\n",
        )
        .run();
}

#[cargo_test]
fn version_with_long_path() {
    // Create a project with an extremely long path
    let long_dir = "a".repeat(200);
    let p = project()
        .file(format!(".cargo/{}/config.toml", long_dir), "")
        .build();

    // Should still work with long paths
    p.cargo("version")
        .with_stdout_data(&format!("cargo {}\n", cargo::version()))
        .run();
}
