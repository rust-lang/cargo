use crate::prelude::*;
use cargo_test_support::file;
use cargo_test_support::paths;
use cargo_test_support::str;
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
        .stdout_eq(str![""])
        .stderr_eq(file!["stderr.term.svg"]);

    assert!(!foo.join("Cargo.toml").is_file());
}
