use cargo_test_support::compare::assert_ui;
use cargo_test_support::prelude::*;
use cargo_test_support::Project;

use cargo_test_support::curr_dir;

#[cargo_test]
fn case() {
    cargo_test_support::registry::init();
    for name in ["my-package1", "my-package2"] {
        for ver in [
            "0.1.1+my-package",
            "0.2.0+my-package",
            "0.2.3+my-package",
            "0.4.1+my-package",
            "20.0.0+my-package",
            "99999.0.0+my-package",
            "99999.0.0-alpha.1+my-package",
        ] {
            cargo_test_support::registry::Package::new(name, ver).publish();
        }
    }

    let project = Project::from_template(curr_dir!().join("in"));
    let project_root = project.root();
    let cwd = &project_root;
    let git_dep = cargo_test_support::git::new("git-package", |project| {
        project
            .file(
                "p1/Cargo.toml",
                &cargo_test_support::basic_manifest("my-package1", "0.3.0+my-package1"),
            )
            .file("p1/src/lib.rs", "")
            .file(
                "p2/Cargo.toml",
                &cargo_test_support::basic_manifest("my-package2", "0.3.0+my-package2"),
            )
            .file("p2/src/lib.rs", "")
    });
    let git_url = git_dep.url().to_string();

    snapbox::cmd::Command::cargo_ui()
        .arg("add")
        .args(["my-package1", "my-package2", "--git", &git_url])
        .current_dir(cwd)
        .assert()
        .success()
        .stdout_matches_path(curr_dir!().join("stdout.log"))
        .stderr_matches_path(curr_dir!().join("stderr.log"));

    assert_ui().subset_matches(curr_dir!().join("out"), &project_root);
}
