use crate::prelude::*;
use cargo_test_support::file;
use cargo_test_support::paths;
use cargo_test_support::str;

#[cargo_test]
fn case() {
    let cwd = paths::root();

    snapbox::cmd::Command::cargo_ui()
        .arg_line("init --lib --bin")
        .current_dir(&cwd)
        .assert()
        .code(101)
        .stdout_eq(str![""])
        .stderr_eq(file!["stderr.term.svg"]);

    assert!(!cwd.join("Cargo.toml").is_file());
}
