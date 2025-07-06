use crate::prelude::*;
use cargo_test_support::Project;
use cargo_test_support::current_dir;
use cargo_test_support::file;

#[cargo_test]
fn case() {
    let project = Project::from_template(current_dir!().join("in"));
    let project_root = project.root();
    let cwd = &project_root;

    cargo_test_support::registry::Package::new("my-package", "0.1.0+my-package")
        .feature("bar", &[])
        .publish();

    snapbox::cmd::Command::cargo_ui()
        .arg("add")
        .arg_line("my-package --features baz")
        .current_dir(cwd)
        .assert()
        .failure()
        .stderr_eq(file!["stderr.term.svg"]);
}
