use cargo_test_support::file;
use cargo_test_support::paths;
use cargo_test_support::prelude::*;

#[cargo_test]
fn case() {
    let cwd = paths::root();

    snapbox::cmd::Command::cargo_ui()
        .arg_line("init --lib --bin")
        .current_dir(&cwd)
        .assert()
        .code(101)
        .stdout_matches(file!["stdout.log"])
        .stderr_matches(file!["stderr.log"]);

    assert!(!cwd.join("Cargo.toml").is_file());
}
