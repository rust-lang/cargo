//! Tests for cargo's help output.

use std::fs;
use std::path::Path;
use std::str::from_utf8;

use cargo_test_support::prelude::*;
use cargo_test_support::registry::Package;
use cargo_test_support::str;
use cargo_test_support::{basic_manifest, cargo_process, paths, project};

#[cargo_test]
fn help() {
    cargo_process("").run();
    cargo_process("help").run();
    cargo_process("-h").run();
    cargo_process("help build").run();
    cargo_process("build -h").run();
    cargo_process("help help").run();
}

#[cargo_test]
fn help_external_subcommand() {
    // Check that `help external-subcommand` forwards the --help flag to the
    // given subcommand.
    Package::new("cargo-fake-help", "1.0.0")
        .file(
            "src/main.rs",
            r#"
            fn main() {
                if ::std::env::args().nth(2) == Some(String::from("--help")) {
                    println!("fancy help output");
                }
            }
            "#,
        )
        .publish();
    cargo_process("install cargo-fake-help").run();
    cargo_process("help fake-help")
        .with_stdout_data(str![[r#"
fancy help output

"#]])
        .run();
}

fn help_with_man(display_command: &str) {
    // Build a "man" process that just echoes the contents.
    let p = project()
        .at(display_command)
        .file("Cargo.toml", &basic_manifest(display_command, "1.0.0"))
        .file(
            "src/main.rs",
            &r#"
                fn main() {
                    eprintln!("custom __COMMAND__");
                    let path = std::env::args().skip(1).next().unwrap();
                    let mut f = std::fs::File::open(path).unwrap();
                    std::io::copy(&mut f, &mut std::io::stdout()).unwrap();
                }
            "#
            .replace("__COMMAND__", display_command),
        )
        .build();
    p.cargo("build").run();

    help_with_man_and_path(display_command, "build", "build", &p.target_debug_dir());
}

fn help_with_man_and_path(
    display_command: &str,
    subcommand: &str,
    actual_subcommand: &str,
    path: &Path,
) {
    let contents = if display_command == "man" {
        fs::read_to_string(format!("src/etc/man/cargo-{}.1", actual_subcommand)).unwrap()
    } else {
        fs::read_to_string(format!(
            "src/doc/man/generated_txt/cargo-{}.txt",
            actual_subcommand
        ))
        .unwrap()
    };

    let output = cargo_process(&format!("help {subcommand}"))
        .env("PATH", path)
        .run();
    let stderr = from_utf8(&output.stderr).unwrap();
    if display_command.is_empty() {
        assert_eq!(stderr, "");
    } else {
        assert_eq!(stderr, format!("custom {}\n", display_command));
    }
    let stdout = from_utf8(&output.stdout).unwrap();
    assert_eq!(stdout, contents);
}

fn help_with_stdout_and_path(subcommand: &str, path: &Path) -> String {
    let output = cargo_process(&format!("help {subcommand}"))
        .env("PATH", path)
        .run();
    let stderr = from_utf8(&output.stderr).unwrap();
    assert_eq!(stderr, "");
    let stdout = from_utf8(&output.stdout).unwrap();
    stdout.to_string()
}

#[cargo_test]
fn help_man() {
    // Checks that `help command` displays the man page using the given command.
    help_with_man("man");
    help_with_man("less");
    help_with_man("more");

    // Check with no commands in PATH.
    help_with_man_and_path("", "build", "build", Path::new(""));
}

#[cargo_test]
fn help_alias() {
    // Check that `help some_alias` will resolve.
    help_with_man_and_path("", "b", "build", Path::new(""));

    let config = paths::root().join(".cargo/config.toml");
    fs::create_dir_all(config.parent().unwrap()).unwrap();
    fs::write(
        config,
        r#"
            [alias]
            empty-alias   = ""
            simple-alias  = "build"
            complex-alias = ["build", "--release"]
        "#,
    )
    .unwrap();

    // The `empty-alias` returns an error.
    cargo_process("help empty-alias")
        .env("PATH", Path::new(""))
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] no such command: `empty-alias`

[HELP] a command with a similar name exists: `empty-alias`

[HELP] view all installed commands with `cargo --list`
[HELP] find a package to install `empty-alias` with `cargo search cargo-empty-alias`

"#]])
        .run();

    // Because `simple-alias` aliases a subcommand with no arguments, help shows the manpage.
    help_with_man_and_path("", "simple-alias", "build", Path::new(""));

    // Help for `complex-alias` displays the full alias command.
    let out = help_with_stdout_and_path("complex-alias", Path::new(""));
    assert_eq!(out, "`complex-alias` is aliased to `build --release`\n");
}
