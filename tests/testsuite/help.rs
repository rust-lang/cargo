//! Tests for cargo's help output.

use cargo_test_support::cargo_process;
use cargo_test_support::registry::Package;

#[cargo_test]
fn help() {
    cargo_process("").run();
    cargo_process("help").run();
    cargo_process("-h").run();
    cargo_process("help build").run();
    cargo_process("build -h").run();
    cargo_process("help help").run();
    // Ensure that help output goes to stdout, not stderr.
    cargo_process("search --help").with_stderr("").run();
    cargo_process("search --help")
        .with_stdout_contains("[..] --frozen [..]")
        .run();
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
            }"#,
        )
        .publish();
    cargo_process("install cargo-fake-help").run();
    cargo_process("help fake-help")
        .with_stdout("fancy help output\n")
        .run();
}

#[cargo_test]
fn z_flags_help() {
    // Test that the output of `cargo -Z help` shows a different help screen with
    // all the `-Z` flags.
    cargo_process("-Z help")
        .with_stdout_contains("    -Z unstable-options -- Allow the usage of unstable options")
        .run();
}
