use cargo_test_support::prelude::*;
use cargo_test_support::Project;
use cargo_test_support::curr_dir;

use crate::cargo_add::init_registry;

#[cargo_test]
fn add_hidden_unactivated() {
    init_registry();
    let project = Project::from_template(curr_dir!().join("in"));
    let cwd = &project.root();

    snapbox::cmd::Command::cargo_ui()
        .current_dir(cwd)
        .args(&["add", "hidden-feature-test"])
        .assert()
        .success()
        .stdout_matches_path(curr_dir!().join("stdout.log"))
        .stderr_matches_path(curr_dir!().join("stderr.log"));
}
