use std::fs;

use crate::prelude::*;
use cargo_test_support::file;
use cargo_test_support::paths;
use cargo_test_support::str;

#[cargo_test]
fn case() {
    let project_root = &paths::root().join("test");
    fs::create_dir_all(project_root).unwrap();

    snapbox::cmd::Command::cargo_ui()
        .arg_line("init")
        .current_dir(project_root)
        .assert()
        .code(101)
        .stdout_eq(str![""])
        .stderr_eq(file!["stderr.term.svg"]);

    assert!(!project_root.join("Cargo.toml").is_file());
}
