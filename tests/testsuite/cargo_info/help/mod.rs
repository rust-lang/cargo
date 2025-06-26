use crate::prelude::*;
use cargo_test_support::file;

#[cargo_test]
fn case() {
    snapbox::cmd::Command::cargo_ui()
        .arg("info")
        .arg("--help")
        .assert()
        .success()
        .stdout_eq(file!["stdout.term.svg"])
        .stderr_eq("");
}
