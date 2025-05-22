use cargo_test_support::file;
use cargo_test_support::prelude::*;
use cargo_test_support::str;

#[cargo_test]
fn case() {
    snapbox::cmd::Command::cargo_ui()
        .arg("login")
        .arg("--help")
        .assert()
        .success()
        .stdout_eq(file!["stdout.term.svg"])
        .stderr_eq(str![""]);
}
