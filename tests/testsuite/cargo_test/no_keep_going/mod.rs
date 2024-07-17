use cargo_test_support::current_dir;
use cargo_test_support::file;
use cargo_test_support::prelude::*;
use cargo_test_support::str;
use cargo_test_support::CargoCommandExt;
use cargo_test_support::Project;

#[cargo_test]
fn case() {
    let project = Project::from_template(current_dir!().join("in"));
    let project_root = project.root();
    let cwd = &project_root;

    snapbox::cmd::Command::cargo_ui()
        .arg("test")
        .arg("--keep-going")
        .current_dir(cwd)
        .assert()
        .code(1)
        .stdout_eq(str![""])
        .stderr_eq(file!["stderr.term.svg"]);
}
