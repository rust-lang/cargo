use cargo_test_support::compare::assert_ui;
use cargo_test_support::current_dir;
use cargo_test_support::file;
use cargo_test_support::prelude::*;
use cargo_test_support::str;
use cargo_test_support::Project;

#[cargo_test]
fn case() {
    cargo_test_support::registry::init();
    cargo_test_support::registry::Package::new("test_cyclic_features", "0.1.1")
        .feature("default", &["feature-one", "feature-two"])
        .feature("feature-one", &["feature-two"])
        .feature("feature-two", &["feature-one"])
        .publish();

    let project = Project::from_template(current_dir!().join("in"));
    let project_root = project.root();
    let cwd = &project_root;

    snapbox::cmd::Command::cargo_ui()
        .arg("add")
        .arg_line("test_cyclic_features")
        .current_dir(cwd)
        .assert()
        .success()
        .stdout_eq(str![""])
        .stderr_eq(file!["stderr.term.svg"]);

    assert_ui().subset_matches(current_dir!().join("out"), &project_root);
}
