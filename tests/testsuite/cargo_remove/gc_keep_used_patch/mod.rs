use cargo_test_support::compare::assert_ui;
use cargo_test_support::curr_dir;
use cargo_test_support::CargoCommand;
use cargo_test_support::Project;

#[cargo_test]
fn case() {
    cargo_test_support::registry::init();
    cargo_test_support::registry::Package::new("serde", "1.0.0").publish();
    cargo_test_support::registry::Package::new("serde_json", "1.0.0")
        .dep("serde", "1.0.0")
        .publish();

    let project = Project::from_template(curr_dir!().join("in"));
    let project_root = project.root();

    snapbox::cmd::Command::cargo_ui()
        .current_dir(&project_root)
        .arg("remove")
        .args(["--package", "serde", "serde_derive"])
        .assert()
        .code(0)
        .stdout_matches_path(curr_dir!().join("stdout.log"))
        .stderr_matches_path(curr_dir!().join("stderr.log"));

    assert_ui().subset_matches(curr_dir!().join("out"), &project_root);
}
