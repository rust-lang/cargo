use crate::prelude::*;
use cargo_test_support::file;

#[cargo_test]
fn case() {
    // Should be `generate-lockfile` but typed wrong as two commands.
    snapbox::cmd::Command::cargo_ui()
        .env("__CARGO_TEST_FORCE_HELP_TXT", "1")
        .arg("help")
        .arg("generate")
        .arg("lockfile")
        .assert()
        .code(101)
        .stdout_eq(file!["stdout.term.txt"])
        .stderr_eq(file!["stderr.term.svg"]);
}
