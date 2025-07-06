use crate::prelude::*;
use cargo_test_support::compare::assert_ui;
use cargo_test_support::current_dir;
use cargo_test_support::file;
use cargo_test_support::str;
use cargo_test_support::{Project, t};

#[cargo_test]
fn case() {
    let project = Project::from_template(current_dir!().join("in"));
    let project_root = &project.root().join("test:ing");

    if !project_root.exists() {
        t!(std::fs::create_dir(&project_root));
    }

    snapbox::cmd::Command::cargo_ui()
        .arg_line("init --bin --vcs none --edition 2015 --name testing")
        .current_dir(project_root)
        .assert()
        .success()
        .stdout_eq(str![""])
        .stderr_eq(file!["stderr.term.svg"]);

    assert_ui().subset_matches(current_dir!().join("out"), project_root);
    assert!(!project_root.join(".gitignore").is_file());
}
