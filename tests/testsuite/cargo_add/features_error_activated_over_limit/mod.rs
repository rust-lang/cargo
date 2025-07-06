use crate::prelude::*;
use cargo_test_support::Project;
use cargo_test_support::compare::assert_ui;
use cargo_test_support::current_dir;
use cargo_test_support::file;
use cargo_test_support::str;

#[cargo_test]
fn case() {
    const MANY_FEATURES_COUNT: usize = 50;

    cargo_test_support::registry::init();
    let mut test_package =
        cargo_test_support::registry::Package::new("your-face", "99999.0.0+my-package");
    for i in 0..MANY_FEATURES_COUNT {
        test_package.feature(format!("eyes{i:03}").as_str(), &[]);
    }
    test_package.publish();

    let project = Project::from_template(current_dir!().join("in"));
    let project_root = project.root();
    let cwd = &project_root;

    let features = "eees100,eees101";
    snapbox::cmd::Command::cargo_ui()
        .arg("add")
        .arg_line(format!("your-face --features {features}").as_str())
        .current_dir(cwd)
        .assert()
        .failure()
        .stdout_eq(str![""])
        .stderr_eq(file!["stderr.term.svg"]);

    assert_ui().subset_matches(current_dir!().join("out"), &project_root);
}
