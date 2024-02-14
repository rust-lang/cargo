use cargo_test_support::file;
use cargo_test_support::paths;
use cargo_test_support::prelude::*;

#[cargo_test]
fn case() {
    snapbox::cmd::Command::cargo_ui()
        .arg_line("init foo --flag")
        .current_dir(paths::root())
        .assert()
        .code(1)
        .stdout_matches(file!["stdout.log"])
        .stderr_matches(file!["stderr.log"]);
}
