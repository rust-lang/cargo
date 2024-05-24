use cargo_test_support::file;
use cargo_test_support::prelude::*;
use cargo_test_support::str;

#[cargo_test]
fn case() {
    snapbox::cmd::Command::cargo_ui()
        .arg("update")
        .arg("--help")
        .assert()
        .success()
        .stdout_eq_(file!["stdout.term.svg"])
        .stderr_eq_(str![""]);
}
