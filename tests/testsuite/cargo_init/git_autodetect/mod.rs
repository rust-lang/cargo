use crate::prelude::*;
use cargo_test_support::compare::assert_ui;
use cargo_test_support::current_dir;
use cargo_test_support::file;
use cargo_test_support::paths;
use cargo_test_support::str;
use std::fs;

#[cargo_test]
fn case() {
    let project_root = &paths::root().join("foo");
    // Need to create `.git` dir manually because it cannot be tracked under a git repo
    fs::create_dir_all(project_root.join(".git")).unwrap();

    snapbox::cmd::Command::cargo_ui()
        .arg_line("init --lib")
        .current_dir(project_root)
        .assert()
        .success()
        .stdout_eq(str![""])
        .stderr_eq(file!["stderr.term.svg"]);

    assert_ui().subset_matches(current_dir!().join("out"), project_root);
    assert!(project_root.join(".git").is_dir());
}
