use crate::prelude::*;
use cargo_test_support::Project;
use cargo_test_support::compare::assert_ui;
use cargo_test_support::current_dir;
use cargo_test_support::file;
use cargo_test_support::str;

// Test that `cargo init` from inside an existing nested member directory
// fails to update the true workspace root's members array.
#[cargo_test]
fn case() {
    let project = Project::from_template(current_dir!().join("in"));
    let project_root = project.root();
    let cwd = project_root.join("a-crate").join("macros");
    std::fs::create_dir_all(&cwd).unwrap();

    snapbox::cmd::Command::cargo_ui()
        .arg_line("init --bin --vcs none")
        .current_dir(&cwd)
        .assert()
        .success()
        .stdout_eq(str![""])
        .stderr_eq(file!["stderr.term.svg"]);

    assert_ui().subset_matches(current_dir!().join("out"), &project_root);
}
