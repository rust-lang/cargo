use cargo_test_support::compare::assert_ui;
use cargo_test_support::paths;
use cargo_test_support::prelude::*;
use std::fs;

use cargo_test_support::curr_dir;

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
        .stdout_matches_path(curr_dir!().join("stdout.log"))
        .stderr_matches_path(curr_dir!().join("stderr.log"));

    assert_ui().subset_matches(curr_dir!().join("out"), project_root);
    assert!(project_root.join(".git").is_dir());
}
