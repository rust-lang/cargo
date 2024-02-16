use cargo_test_support::file;
use cargo_test_support::paths;
use cargo_test_support::prelude::*;
use std::fs;

#[cargo_test]
fn case() {
    let foo = &paths::root().join("foo.bar");
    fs::create_dir_all(foo).unwrap();

    snapbox::cmd::Command::cargo_ui()
        .arg_line("init")
        .current_dir(foo)
        .assert()
        .code(101)
        .stdout_matches(file!["stdout.log"])
        .stderr_matches(file!["stderr.log"]);

    assert!(!foo.join("Cargo.toml").is_file());
}
