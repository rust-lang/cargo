use cargo_test_support::compare::assert_ui;
use cargo_test_support::prelude::*;
use cargo_test_support::Project;
use itertools::Itertools;

use cargo_test_support::curr_dir;

#[cargo_test]
fn case() {
    const MANY_FEATURES_COUNT: usize = 200;
    const ACTIVATED_FEATURES_COUNT: usize = 100;

    cargo_test_support::registry::init();
    let mut test_package =
        cargo_test_support::registry::Package::new("your-face", "99999.0.0+my-package");
    for i in 0..MANY_FEATURES_COUNT {
        test_package.feature(format!("eyes{i:03}").as_str(), &[]);
    }
    test_package.publish();

    let project = Project::from_template(curr_dir!().join("in"));
    let project_root = project.root();
    let cwd = &project_root;

    let features = (0..ACTIVATED_FEATURES_COUNT)
        .map(|i| format!("eyes{i:03}"))
        .join(",");
    snapbox::cmd::Command::cargo_ui()
        .arg("add")
        .arg_line(format!("your-face --features {features}").as_str())
        .current_dir(cwd)
        .assert()
        .success()
        .stdout_matches_path(curr_dir!().join("stdout.log"))
        .stderr_matches_path(curr_dir!().join("stderr.log"));

    assert_ui().subset_matches(curr_dir!().join("out"), &project_root);
}
