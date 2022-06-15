//! Tests for the `safe.directories` configuration option.

use cargo_test_support::paths::{self, CargoPathExt};
use cargo_test_support::{basic_manifest, cargo_process, project, Project};
use std::fs;

fn sample_config_project() -> Project {
    project()
        .file("src/lib.rs", "")
        .file(".cargo/config.toml", "job.builds = 1")
        .build()
}

#[cargo_test]
fn gated() {
    // Checks that it only works on nightly.
    let p = sample_config_project();
    let cfg = p.root().join(".cargo/config.toml");
    p.cargo("check")
        // This is ignored when no masquerade.
        .env("CARGO_UNSTABLE_SAFE_DIRECTORIES", "true")
        .env("__CARGO_TEST_OWNERSHIP", &cfg)
        .with_stderr(
            "\
[CHECKING] foo [..]
[FINISHED] [..]
",
        )
        .run();
    eprintln!("safe path is {}", cfg.display());
    p.cargo("check")
        .env("CARGO_UNSTABLE_SAFE_DIRECTORIES", "true")
        .env("__CARGO_TEST_OWNERSHIP", &cfg)
        .masquerade_as_nightly_cargo()
        .with_stderr_contains("[..]`[ROOT]/foo/.cargo/config.toml` is owned[..]")
        .with_status(101)
        .run();
    let manifest = p.root().join("Cargo.toml");
    p.cargo("check")
        // This is ignored when no masquerade.
        .env("CARGO_UNSTABLE_SAFE_DIRECTORIES", "true")
        .env("__CARGO_TEST_OWNERSHIP", &manifest)
        .with_stderr("[FINISHED] [..]")
        .run();
    p.cargo("check")
        .env("CARGO_UNSTABLE_SAFE_DIRECTORIES", "true")
        .env("__CARGO_TEST_OWNERSHIP", &manifest)
        .masquerade_as_nightly_cargo()
        .with_stderr_contains("[..]`[ROOT]/foo/Cargo.toml` is owned[..]")
        .with_status(101)
        .run();
}

#[cargo_test]
fn unsafe_config() {
    // Checks that untrusted configs are rejected.
    let p = sample_config_project();
    let cfg = p.root().join(".cargo/config.toml");
    p.cargo("check")
        .env("CARGO_UNSTABLE_SAFE_DIRECTORIES", "true")
        .env("__CARGO_TEST_OWNERSHIP", &cfg)
        .env("RUSTUP_TOOLCHAIN", "does-not-exist")
        .masquerade_as_nightly_cargo()
        .with_stderr(&format!(
            "\
error: could not load Cargo configuration

Caused by:
  `[ROOT]/foo/.cargo/config.toml` is owned by a different user
  For safety reasons, Cargo does not allow opening config files by
  a different user, unless explicitly approved.

  To approve this directory, run:

      rustup set safe-directories add [..][ROOT]/foo[..]

  See https://rust-lang.github.io/rustup/safe-directories.html for more information.

  Current user: [..]
  Owner of file: [..]
"
        ))
        .with_status(101)
        .run();

    // Same check without rustup installed.
    p.cargo("check")
        .env("CARGO_UNSTABLE_SAFE_DIRECTORIES", "true")
        .env("__CARGO_TEST_OWNERSHIP", &cfg)
        .env_remove("RUSTUP_TOOLCHAIN")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
error: could not load Cargo configuration

Caused by:
  `[ROOT]/foo/.cargo/config.toml` is owned by a different user
  For safety reasons, Cargo does not allow opening config files by
  a different user, unless explicitly approved.

  To approve this directory, set the CARGO_SAFE_DIRECTORIES environment
  variable to \"[ROOT]/foo\" or edit
  `[ROOT]/home/.cargo/config.toml` and add:

      [safe]
      directories = [[..][ROOT]/foo[..]]

  See https://doc.rust-lang.org/nightly/cargo/reference/config.html#safedirectories
  for more information.

  Current user: [..]
  Owner of file: [..]
",
        )
        .with_status(101)
        .run();

    // Add the config in the home directory.
    let cargo_home = paths::home().join(".cargo");
    cargo_home.mkdir_p();
    fs::write(
        cargo_home.join("config.toml"),
        &format!(
            "
                [safe]
                directories = [{}]
            ",
            toml_edit::easy::Value::String(p.root().to_str().unwrap().to_string())
        ),
    )
    .unwrap();

    p.cargo("check")
        .env("CARGO_UNSTABLE_SAFE_DIRECTORIES", "true")
        .env("__CARGO_TEST_OWNERSHIP", &cfg)
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[CHECKING] foo [..]
[FINISHED] dev [..]
",
        )
        .run();
}

#[cargo_test]
fn asterisk() {
    // Checks that * allows all.
    let p = sample_config_project();
    let cfg = p.root().join(".cargo/config.toml");
    p.cargo("check")
        .env("CARGO_UNSTABLE_SAFE_DIRECTORIES", "true")
        .env("__CARGO_TEST_OWNERSHIP", &cfg)
        .masquerade_as_nightly_cargo()
        .with_stderr_contains("[..]`[ROOT]/foo/.cargo/config.toml` is owned[..]")
        .with_status(101)
        .run();
    let cargo_home = paths::home().join(".cargo");
    cargo_home.mkdir_p();
    fs::write(
        cargo_home.join("config.toml"),
        "
            [safe]
            directories = ['*']
        ",
    )
    .unwrap();
    p.cargo("check")
        .env("CARGO_UNSTABLE_SAFE_DIRECTORIES", "true")
        .env("__CARGO_TEST_OWNERSHIP", &cfg)
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[CHECKING] foo [..]
[FINISHED] dev [..]
",
        )
        .run();
}

#[cargo_test]
fn config_in_home_only() {
    // Checks that safe.directories can only be set in the home directory.
    let p = project()
        .file("src/lib.rs", "")
        .file(".cargo/config.toml", "safe.directories = ['*']")
        .build();
    p.cargo("check")
        .env("CARGO_UNSTABLE_SAFE_DIRECTORIES", "true")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
error: could not load Cargo configuration

Caused by:
  safe.directories may only be configured from Cargo's home directory
  Found `safe.directories` in [ROOT]/foo/.cargo/config.toml
  Cargo's home directory is [ROOT]/home/.cargo
",
        )
        .with_status(101)
        .run();
}

#[cargo_test]
fn home_config_not_checked() {
    // home config ownership doesn't matter.
    let p = project().file("src/lib.rs", "").build();
    let cargo_home = paths::home().join(".cargo");
    cargo_home.mkdir_p();
    let home_config = cargo_home.join("config.toml");
    fs::write(&home_config, "build.jobs = 1").unwrap();
    p.cargo("check")
        .env("CARGO_UNSTABLE_SAFE_DIRECTORIES", "true")
        .env("__CARGO_TEST_OWNERSHIP", &home_config)
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[CHECKING] foo [..]
[FINISHED] dev [..]
",
        )
        .run();
}

#[cargo_test]
fn environment_variables() {
    // Check that environment variables are supported.
    let p = sample_config_project();
    let cfg = p.root().join(".cargo/config.toml");
    p.cargo("check")
        .env("CARGO_UNSTABLE_SAFE_DIRECTORIES", "true")
        .env("__CARGO_TEST_OWNERSHIP", &cfg)
        .masquerade_as_nightly_cargo()
        .with_stderr_contains("[..]`[ROOT]/foo/.cargo/config.toml` is owned[..]")
        .with_status(101)
        .run();
    p.cargo("check")
        .env("CARGO_UNSTABLE_SAFE_DIRECTORIES", "true")
        .env("__CARGO_TEST_OWNERSHIP", &cfg)
        .env("CARGO_SAFE_DIRECTORIES", p.root())
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[CHECKING] foo [..]
[FINISHED] dev [..]
",
        )
        .run();
    p.cargo("check")
        .env("CARGO_UNSTABLE_SAFE_DIRECTORIES", "true")
        .env("__CARGO_TEST_OWNERSHIP", &cfg)
        .env("RUSTUP_SAFE_DIRECTORIES", p.root())
        .masquerade_as_nightly_cargo()
        .with_stderr("[FINISHED] dev [..]")
        .run();
}

#[cargo_test]
fn not_allowed_on_cli() {
    // safe.directories is not supported on the CLI
    let p = sample_config_project();
    p.cargo("check -Zunstable-options --config")
        .env("CARGO_UNSTABLE_SAFE_DIRECTORIES", "true")
        .arg("safe.directories=['foo']")
        .masquerade_as_nightly_cargo()
        .with_stderr("error: safe.directories cannot be set via the CLI")
        .with_status(101)
        .run();
}

#[cargo_test]
fn cli_path_is_allowed() {
    // Allow loading config files owned by another user passed on the path.
    // This is based on the idea that the user has explicitly listed the path,
    // and that is regarded as intentional trust.
    let p = project()
        .file("src/lib.rs", "")
        .file("myconfig.toml", "build.jobs = 1")
        .build();
    p.cargo("check --config myconfig.toml -Zunstable-options")
        .env("CARGO_UNSTABLE_SAFE_DIRECTORIES", "true")
        .env("__CARGO_TEST_OWNERSHIP", p.root().join("myconfig.toml"))
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[CHECKING] foo [..]
[FINISHED] [..]
",
        )
        .run();
}

#[cargo_test]
fn included_path_is_allowed() {
    // Allow loading configs from includes that are owned by another user.
    // This is based on the idea that the a config owned by the user has
    // explicitly listed the path, and that is regarded as intentional trust.
    let p = project()
        .file("src/lib.rs", "")
        .file(".cargo/config.toml", "include = 'other.toml'")
        .file(".cargo/other.toml", "build.jobs = 1")
        .build();
    p.cargo("check -Z config-include")
        .env("CARGO_UNSTABLE_SAFE_DIRECTORIES", "true")
        .env("__CARGO_TEST_OWNERSHIP", p.root().join(".cargo/other.toml"))
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[CHECKING] foo [..]
[FINISHED] [..]
",
        )
        .run();
}

#[cargo_test]
fn aliases_are_handled() {
    // Checks that aliases can trigger the check, and can be overridden.
    // This is here because aliases are different and are handled very early.
    let cargo_home = paths::home().join(".cargo");
    cargo_home.mkdir_p();
    let home_config = cargo_home.join("config.toml");
    fs::write(&home_config, "alias.x = 'new'").unwrap();

    let p = sample_config_project();
    let cfg = p.root().join(".cargo/config.toml");
    p.cargo("x foo")
        .env("CARGO_UNSTABLE_SAFE_DIRECTORIES", "true")
        .env("__CARGO_TEST_OWNERSHIP", &cfg)
        .masquerade_as_nightly_cargo()
        .with_stderr_contains("[..]`[ROOT]/foo/.cargo/config.toml` is owned[..]")
        .with_status(101)
        .run();

    // Use * to allow
    p.cargo("x foo")
        .env("CARGO_UNSTABLE_SAFE_DIRECTORIES", "true")
        .env("__CARGO_TEST_OWNERSHIP", &cfg)
        .env("CARGO_SAFE_DIRECTORIES", "*")
        .masquerade_as_nightly_cargo()
        .with_stderr("[CREATED][..]")
        .run();

    // Try with alias defined within project.
    p.change_file(".cargo/config.toml", "alias.y = 'new'");
    p.cargo("y foo")
        .env("CARGO_UNSTABLE_SAFE_DIRECTORIES", "true")
        .env("__CARGO_TEST_OWNERSHIP", &cfg)
        .masquerade_as_nightly_cargo()
        .with_stderr_contains("[..]`[ROOT]/foo/.cargo/config.toml` is owned[..]")
        .with_status(101)
        .run();

    // Use * to allow
    p.cargo("y bar")
        .env("CARGO_UNSTABLE_SAFE_DIRECTORIES", "true")
        .env("__CARGO_TEST_OWNERSHIP", &cfg)
        .env("CARGO_SAFE_DIRECTORIES", "*")
        .masquerade_as_nightly_cargo()
        .with_stderr("[CREATED][..]")
        .run();
}

#[cargo_test]
fn unsafe_manifest() {
    // "current" Cargo.toml owned by a different user
    let p = project().file("src/lib.rs", "").build();
    let manifest = p.root().join("Cargo.toml");
    p.cargo("tree")
        .env("CARGO_UNSTABLE_SAFE_DIRECTORIES", "true")
        .env("__CARGO_TEST_OWNERSHIP", &manifest)
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
error: `[ROOT]/foo/Cargo.toml` is owned by a different user
For safety reasons, Cargo does not allow opening manifests by
a different user, unless explicitly approved.

To approve this directory, run:

    rustup set safe-directories add [ROOT]/foo

See https://rust-lang.github.io/rustup/safe-directories.html for more information.

Current user: [..]
Owner of file: [..]
",
        )
        .with_status(101)
        .run();
    p.cargo("tree")
        .env("CARGO_UNSTABLE_SAFE_DIRECTORIES", "true")
        .env("__CARGO_TEST_OWNERSHIP", &manifest)
        .env("CARGO_SAFE_DIRECTORIES", &manifest)
        .masquerade_as_nightly_cargo()
        .with_stdout("foo [..]")
        .with_stderr("")
        .run();
}

#[cargo_test]
fn explicit_manifest_is_ok() {
    // --manifest-path doesn't check, as it is an explicit path
    let p = project().file("src/lib.rs", "").build();
    let manifest = p.root().join("Cargo.toml");
    cargo_process("check --manifest-path")
        .arg(&manifest)
        .env("CARGO_UNSTABLE_SAFE_DIRECTORIES", "true")
        .env("__CARGO_TEST_OWNERSHIP", &manifest)
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[CHECKING] foo [..]
[FINISHED] [..]
",
        )
        .run();
}

#[cargo_test]
fn unsafe_workspace() {
    // Workspace Cargo.toml owned by a different user.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["member1"]
            "#,
        )
        .file("member1/Cargo.toml", &basic_manifest("member1", "1.0.0"))
        .file("member1/src/lib.rs", "")
        .build();
    let manifest = p.root().join("Cargo.toml");
    p.cargo("tree")
        .env("CARGO_UNSTABLE_SAFE_DIRECTORIES", "true")
        .env("__CARGO_TEST_OWNERSHIP", &manifest)
        .masquerade_as_nightly_cargo()
        // Make it search upwards.
        .cwd(p.root().join("member1"))
        .with_stderr_contains("error: `[ROOT]/foo/Cargo.toml` is owned by a different user")
        .with_status(101)
        .run();
}

#[cargo_test]
fn workspace_via_link() {
    // package.workspace isn't a "search" and should be safe.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["member1"]
            "#,
        )
        .file(
            "member1/Cargo.toml",
            r#"
                [package]
                workspace = ".."
                name = "member1"
                version = "1.0.0"
            "#,
        )
        .file("member1/src/lib.rs", "")
        .build();
    let ws_manifest = p.root().join("Cargo.toml");
    let project_manifest = p.root().join("member1/Cargo.toml");
    cargo_process("check --manifest-path")
        .arg(&project_manifest)
        .env("CARGO_UNSTABLE_SAFE_DIRECTORIES", "true")
        .env("__CARGO_TEST_OWNERSHIP", &ws_manifest)
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[CHECKING] member1 [..]
[FINISHED] [..]
",
        )
        .run();
    // Implied workspace root should fail.
    cargo_process("check")
        .cwd(p.root())
        .env("CARGO_UNSTABLE_SAFE_DIRECTORIES", "true")
        .env("__CARGO_TEST_OWNERSHIP", &ws_manifest)
        .masquerade_as_nightly_cargo()
        .with_stderr_contains("error: `[ROOT]/foo/Cargo.toml` is owned by a different user")
        .with_status(101)
        .run();
}

#[cargo_test]
fn path_dependency() {
    // path dependencies should be ok
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [dependencies]
                bar = { path="bar" }
            "#,
        )
        .file("src/lib.rs", "")
        .file("bar/Cargo.toml", &basic_manifest("bar", "1.0.0"))
        .file("bar/src/lib.rs", "")
        .build();
    let bar_manifest = p.root().join("bar/Cargo.toml");
    p.cargo("tree")
        .env("CARGO_UNSTABLE_SAFE_DIRECTORIES", "true")
        .env("__CARGO_TEST_OWNERSHIP", &bar_manifest)
        .masquerade_as_nightly_cargo()
        .with_stderr("")
        .with_stdout(
            "\
foo v0.1.0 [..]
└── bar v1.0.0 [..]
",
        )
        .run();

    // Discovery from the `bar` directory is not ok.
    p.cargo("tree")
        .cwd("bar")
        .env("CARGO_UNSTABLE_SAFE_DIRECTORIES", "true")
        .env("__CARGO_TEST_OWNERSHIP", &bar_manifest)
        .masquerade_as_nightly_cargo()
        .with_stderr_contains("error: `[ROOT]/foo/bar/Cargo.toml` is owned by a different user")
        .with_status(101)
        .run();
}
