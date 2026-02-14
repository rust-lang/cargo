use crate::prelude::*;
use cargo_test_support::file;

#[cargo_test]
fn case() {
    // Valid parent command `report`, but invalid subcommand `foo`
    snapbox::cmd::Command::cargo_ui()
        .arg("help")
        .arg("report")
        .arg("foo")
        .assert()
        .code(1)
        .stdout_eq(file!["stdout.term.txt"])
        .stderr_eq(file!["stderr.term.svg"]);
}
