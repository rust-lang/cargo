use crate::prelude::*;
use cargo_test_support::Project;
use cargo_test_support::compare::assert_ui;
use cargo_test_support::current_dir;
use cargo_test_support::file;
use cargo_test_support::str;

#[cargo_test]
fn case() {
    let project = Project::from_template(current_dir!().join("in"));
    let project_root = project.root();
    let cwd = &project_root;
    let cargo_home = project_root.join("cargo-home");

    snapbox::cmd::Command::cargo_ui()
        .arg_line("report timings -Zbuild-analysis")
        .masquerade_as_nightly_cargo(&["build-analysis"])
        .current_dir(cwd)
        .env("CARGO_HOME", cargo_home)
        .env("__CARGO_TEST_REPORT_TIMINGS_TEMPDIR", cwd)
        .assert()
        .success()
        .stdout_eq(str![""])
        .stderr_eq(file!["stderr.term.svg"]);

    assert_ui().subset_matches(current_dir!().join("out"), &project_root);
}
