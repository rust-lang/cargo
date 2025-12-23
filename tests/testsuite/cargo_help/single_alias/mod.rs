use crate::prelude::*;
use cargo_test_support::file;

#[cargo_test]
fn case() {
    snapbox::cmd::Command::cargo_ui()
        .env("__CARGO_TEST_FORCE_HELP_TXT", "1")
        .arg("help")
        .arg("rm")
        .assert()
        .success()
        .stdout_eq(file!["stdout.term.txt"])
        .stderr_eq(file!["stderr.term.svg"]);
}
