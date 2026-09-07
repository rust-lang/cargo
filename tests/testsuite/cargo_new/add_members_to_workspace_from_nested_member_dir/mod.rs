use crate::prelude::*;
use cargo_test_support::Project;
use cargo_test_support::compare::assert_ui;
use cargo_test_support::current_dir;
use cargo_test_support::file;
use cargo_test_support::str;

// Test that when `cargo new` is run from inside a workspace member (not the root),
// the new crate is created but the workspace root's `members` array is NOT updated,
// demonstrating the buggy behavior where the walker stops at the first Cargo.toml.
#[cargo_test]
fn case() {
    let project = Project::from_template(current_dir!().join("in"));
    let project_root = project.root();
    let cwd = project_root.join("a-crate");

    snapbox::cmd::Command::cargo_ui()
        .arg("new")
        .args(["macros"])
        .current_dir(&cwd)
        .assert()
        .success()
        .stdout_eq(str![""])
        .stderr_eq(file!["stderr.term.svg"]);

    assert_ui().subset_matches(current_dir!().join("out"), &project_root);
}
