use cargo_test_support::file;
use cargo_test_support::prelude::*;
use cargo_test_support::str;

#[cargo_test]
fn case() {
    snapbox::cmd::Command::cargo_ui()
        .masquerade_as_nightly_cargo(&["-Z help"])
        .args(["-Z", "help"])
        .assert()
        .success()
        .stdout_eq(file!["stdout.term.svg"])
        .stderr_eq(str![""]);
}
