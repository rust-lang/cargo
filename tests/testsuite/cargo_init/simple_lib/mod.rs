use crate::prelude::*;
use cargo_test_support::Project;
use cargo_test_support::compare::assert_ui;
use cargo_test_support::current_dir;
use cargo_test_support::file;
use cargo_test_support::str;

#[cargo_test]
fn case() {
    let project = Project::from_template(current_dir!().join("in"));
    let project_root = &project.root();

    snapbox::cmd::Command::cargo_ui()
        .arg_line("init --lib --vcs none --edition 2015")
        .current_dir(project_root)
        .assert()
        .success()
        .stdout_eq(str![""])
        .stderr_eq(file!["stderr.term.svg"]);

    assert_ui().subset_matches(current_dir!().join("out"), project_root);
    assert!(!project_root.join(".gitignore").is_file());

    snapbox::cmd::Command::cargo_ui()
        .current_dir(project_root)
        .arg("build")
        .assert()
        .success();
    assert!(!project.bin("foo").is_file());
}
