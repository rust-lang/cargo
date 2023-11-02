use cargo_test_support::compare::assert_ui;
use cargo_test_support::prelude::*;
use cargo_test_support::Project;

#[cargo_test]
fn case() {
    cargo_test_support::registry::alt_init();
    for ver in [
        "0.1.1+my-package",
        "0.2.0+my-package",
        "0.2.3+my-package",
        "0.4.1+my-package",
        "20.0.0+my-package",
        "99999.0.0+my-package",
        "99999.0.0-alpha.1+my-package",
    ] {
        cargo_test_support::registry::Package::new("cargo-list-test-fixture-dependency", ver)
            .alternative(true)
            .publish();
    }

    let project =
        Project::from_template("tests/testsuite/cargo_add/build_prefer_existing_version/in");
    let project_root = project.root();
    let cwd = &project_root;

    snapbox::cmd::Command::cargo_ui()
        .arg("add")
        .arg_line("cargo-list-test-fixture-dependency --build")
        .current_dir(cwd)
        .assert()
        .success()
        .stdout_matches_path("tests/testsuite/cargo_add/build_prefer_existing_version/stdout.log")
        .stderr_matches_path("tests/testsuite/cargo_add/build_prefer_existing_version/stderr.log");

    assert_ui().subset_matches(
        "tests/testsuite/cargo_add/build_prefer_existing_version/out",
        &project_root,
    );
}
