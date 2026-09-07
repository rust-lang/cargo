use crate::prelude::*;
use cargo_test_support::Project;
use cargo_test_support::compare::assert_ui;
use cargo_test_support::current_dir;
use cargo_test_support::file;
use cargo_test_support::str;

// Test that creating a new crate inside a workspace member correctly respects `workspace.exclude`
// and does not add the new crate to the workspace members array if it is excluded.
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
