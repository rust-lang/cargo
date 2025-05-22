use cargo_test_support::compare::assert_ui;
use cargo_test_support::current_dir;
use cargo_test_support::file;
use cargo_test_support::prelude::*;
use cargo_test_support::str;
use cargo_test_support::Project;

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

    let project = Project::from_template(current_dir!().join("in"));
    let project_root = project.root();
    let cwd = &project_root;

    snapbox::cmd::Command::cargo_ui()
        .arg("add")
        .arg_line("my-package1 my-package2 --target wasm32-unknown-unknown")
        .current_dir(cwd)
        .assert()
        .success()
        .stdout_eq(str![""])
        .stderr_eq(file!["stderr.term.svg"]);

    assert_ui().subset_matches(current_dir!().join("out"), &project_root);
}
