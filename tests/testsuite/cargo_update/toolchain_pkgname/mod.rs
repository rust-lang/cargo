use cargo_test_support::current_dir;
use cargo_test_support::file;
use cargo_test_support::prelude::*;
use cargo_test_support::str;
use cargo_test_support::Project;

#[cargo_test]
fn case() {
    let project = Project::from_template(current_dir!().join("in"));
    let project_root = project.root();
    let cwd = &project_root;

    snapbox::cmd::Command::cargo_ui()
        .arg("update")
        .arg("+stable")
        .current_dir(cwd)
        .assert()
        .code(101)
        .stdout_matches(str![""])
        .stderr_matches(file!["stderr.term.svg"]);
}
