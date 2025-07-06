use crate::prelude::*;
use cargo_test_support::Project;
use cargo_test_support::compare::assert_ui;
use cargo_test_support::current_dir;
use cargo_test_support::file;
use cargo_test_support::str;

#[cargo_test]
fn case() {
    cargo_test_support::registry::init();
    cargo_test_support::registry::Package::new("serde", "1.0.0").publish();
    cargo_test_support::registry::Package::new("serde_json", "1.0.0")
        .dep("serde", "1.0.0")
        .publish();

    let project = Project::from_template(current_dir!().join("in"));
    let project_root = project.root();

    snapbox::cmd::Command::cargo_ui()
        .current_dir(&project_root)
        .arg("remove")
        .args(["--package", "serde", "serde_derive"])
        .assert()
        .code(0)
        .stdout_eq(str![""])
        .stderr_eq(file!["stderr.term.svg"]);

    assert_ui().subset_matches(current_dir!().join("out"), &project_root);
}
