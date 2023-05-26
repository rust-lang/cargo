use cargo_test_support::compare::assert_ui;
use cargo_test_support::prelude::*;
use cargo_test_support::Project;

use cargo_test_support::curr_dir;

#[cargo_test]
fn case() {
    cargo_test_support::registry::init();

    let project = Project::from_template(curr_dir!().join("in"));
    let project_root = project.root();
    let cwd = &project_root;
    let git_url = url::Url::from_directory_path(cwd.join("does-not-exist"))
        .unwrap()
        .to_string();

    snapbox::cmd::Command::cargo_ui()
        .arg("add")
        .args(["fake-git", "--git", &git_url])
        .current_dir(cwd)
        .assert()
        .code(101)
        .stdout_matches_path(curr_dir!().join("stdout.log"))
        .stderr_matches_path(curr_dir!().join("stderr.log"));

    assert_ui().subset_matches(curr_dir!().join("out"), &project_root);
}
