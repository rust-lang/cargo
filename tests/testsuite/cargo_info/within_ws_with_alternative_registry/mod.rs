use crate::prelude::*;
use cargo_test_support::{Project, compare::assert_ui, registry::RegistryBuilder};
use cargo_test_support::{current_dir, file};

use super::init_registry_without_token;

#[cargo_test]
fn case() {
    init_registry_without_token();
    let _ = RegistryBuilder::new()
        .alternative()
        .no_configure_token()
        .build();
    cargo_test_support::registry::Package::new("my-package", "99999.0.0-alpha.1+my-package")
        .alternative(true)
        .publish();

    let project = Project::from_template(current_dir!().join("in"));
    let project_root = project.root();
    let cwd = &project_root;

    snapbox::cmd::Command::cargo_ui()
        .arg("info")
        .arg("my-package")
        .arg("--registry=alternative")
        .current_dir(cwd)
        .assert()
        .success()
        .stdout_eq(file!["stdout.term.svg"])
        .stderr_eq(file!["stderr.term.svg"]);

    assert_ui().subset_matches(current_dir!().join("out"), &project_root);
}
