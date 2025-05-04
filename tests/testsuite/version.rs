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
    let p = project().file(".cargo/config.toml", "").build();

    // Make the config directory unreadable
    p.change_file(
        ".cargo/config.toml",
        &[0xFF, 0xFE, 0xFF, 0xFF], // Invalid UTF-8
    );

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

    // Test with various format flags
    p.cargo("version --format=json")
        .with_status(101)
        .with_stderr("[ERROR] unsupported format flag `json` for `version` command\n")
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

#[cargo_test]
fn version_with_no_permission() {
    let p = project().build();

    // Create a directory without read permissions
    let no_perm_dir = p.root().join(".cargo/no_perm");
    std::fs::create_dir_all(&no_perm_dir).unwrap();

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let metadata = std::fs::metadata(&no_perm_dir).unwrap();
        let mut perms = metadata.permissions();
        perms.set_mode(0o000);
        std::fs::set_permissions(&no_perm_dir, perms).unwrap();
    }

    // Version command should work even with permission issues
    p.cargo("version")
        .with_stdout_data(&format!("cargo {}\n", cargo::version()))
        .run();
}
