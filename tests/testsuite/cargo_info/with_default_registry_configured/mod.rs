use crate::prelude::*;
use cargo_test_support::{Project, compare::assert_ui, current_dir, file};

use super::init_registry_without_token;

#[cargo_test]
fn case() {
    init_registry_without_token();

    let project = Project::from_template(current_dir!().join("in"));
    let project_root = project.root();
    let cwd = &project_root;

    snapbox::cmd::Command::cargo_ui()
        .arg("info")
        .arg_line("--verbose foo")
        .current_dir(cwd)
        .assert()
        .success()
        .stdout_eq(file!["stdout.term.svg"])
        .stderr_eq("");

    assert_ui().subset_matches(current_dir!().join("out"), &project_root);
}
