use crate::prelude::*;
use cargo_test_support::file;
use cargo_test_support::paths;
use cargo_test_support::str;

#[cfg(not(windows))]
#[cargo_test]
fn case() {
    snapbox::cmd::Command::cargo_ui()
        .arg_line("init /")
        .current_dir(paths::root())
        .assert()
        .code(101)
        .stdout_eq(str![""])
        .stderr_eq(file!["stderr.term.svg"]);
}
