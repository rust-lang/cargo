use crate::prelude::*;
use cargo_test_support::file;
use cargo_test_support::str;

#[cargo_test]
fn case() {
    snapbox::cmd::Command::cargo_ui()
        .args(["report", "timings"])
        .arg("--help")
        .assert()
        .failure()
        .stdout_eq(str![""])
        .stderr_eq(file!["stderr.term.svg"]);
}
