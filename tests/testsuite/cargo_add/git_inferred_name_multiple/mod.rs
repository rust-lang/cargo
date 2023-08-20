use cargo_test_support::compare::assert_ui;
use cargo_test_support::prelude::*;
use cargo_test_support::Project;

use cargo_test_support::curr_dir;

#[cargo_test]
fn case() {
    cargo_test_support::registry::init();

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
            .file(
                "p3/Cargo.toml",
                &cargo_test_support::basic_manifest("my-package3", "0.3.0+my-package2"),
            )
            .file("p3/src/lib.rs", "")
            .file(
                "p4/Cargo.toml",
                &cargo_test_support::basic_manifest("my-package4", "0.3.0+my-package2"),
            )
            .file("p4/src/lib.rs", "")
            .file(
                "p5/Cargo.toml",
                &cargo_test_support::basic_manifest("my-package5", "0.3.0+my-package2"),
            )
            .file("p5/src/lib.rs", "")
            .file(
                "p6/Cargo.toml",
                &cargo_test_support::basic_manifest("my-package6", "0.3.0+my-package2"),
            )
            .file("p6/src/lib.rs", "")
            .file(
                "p7/Cargo.toml",
                &cargo_test_support::basic_manifest("my-package7", "0.3.0+my-package2"),
            )
            .file("p7/src/lib.rs", "")
            .file(
                "p8/Cargo.toml",
                &cargo_test_support::basic_manifest("my-package8", "0.3.0+my-package2"),
            )
            .file("p8/src/lib.rs", "")
            .file(
                "p9/Cargo.toml",
                &cargo_test_support::basic_manifest("my-package9", "0.3.0+my-package2"),
            )
            .file("p9/src/lib.rs", "")
    });
    let git_url = git_dep.url().to_string();

    snapbox::cmd::Command::cargo_ui()
        .arg("add")
        .args(["--git", &git_url])
        .current_dir(cwd)
        .assert()
        .code(101)
        .stdout_matches_path(curr_dir!().join("stdout.log"))
        .stderr_matches_path(curr_dir!().join("stderr.log"));

    assert_ui().subset_matches(curr_dir!().join("out"), &project_root);
}
