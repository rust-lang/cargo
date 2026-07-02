use crate::prelude::*;
use cargo_test_support::Project;
use cargo_test_support::compare::assert_ui;
use cargo_test_support::current_dir;
use cargo_test_support::registry;

#[cargo_test]
fn case() {
    registry::init();
    registry::Package::new("foo", "1.0.0").publish();

    let project = Project::from_template(current_dir!().join("in"));
    let project_root = project.root();
    let cwd = &project_root;

    snapbox::cmd::Command::cargo_ui()
        .arg_line("init --bin --vcs none --edition 2024 --name cargo-list-test-fixture")
        .current_dir(cwd)
        .assert()
        .success();

    snapbox::cmd::Command::cargo_ui()
        .arg("add")
        .arg_line("foo@1.0 --default-features")
        .current_dir(cwd)
        .assert()
        .success();

    assert_ui().subset_matches(current_dir!().join("out"), &project_root);
}
