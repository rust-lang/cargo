use cargo_test_support::compare::assert_ui;
use cargo_test_support::prelude::*;
use cargo_test_support::Project;

use cargo_test_support::curr_dir;

#[cargo_test]
fn case() {
    cargo_test_support::registry::init();
    cargo_test_support::registry::Package::new("rust-version-user", "0.1.0")
        .rust_version("1.66")
        .publish();
    cargo_test_support::registry::Package::new("rust-version-user", "0.2.1")
        .rust_version("1.72")
        .publish();

    let project = Project::from_template(curr_dir!().join("in"));
    let project_root = project.root();
    let cwd = &project_root;

    snapbox::cmd::Command::cargo_ui()
        .arg("-Zmsrv-policy")
        .arg("add")
        .arg("--ignore-rust-version")
        .arg_line("rust-version-user")
        .current_dir(cwd)
        .masquerade_as_nightly_cargo(&["msrv-policy"])
        .assert()
        .code(101)
        .stdout_matches_path(curr_dir!().join("stdout.log"))
        .stderr_matches_path(curr_dir!().join("stderr.log"));

    assert_ui().subset_matches(curr_dir!().join("out"), &project_root);
}
