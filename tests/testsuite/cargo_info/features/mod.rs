use crate::prelude::*;
use cargo_test_support::file;

use super::init_registry_without_token;

#[cargo_test]
fn case() {
    init_registry_without_token();
    cargo_test_support::registry::Package::new("my-package", "0.1.1")
        .feature("default", &["feature1", "feature2"])
        .feature("feature1", &[])
        .feature("feature2", &[])
        .publish();

    snapbox::cmd::Command::cargo_ui()
        .arg("info")
        .arg("my-package")
        .arg("--registry=dummy-registry")
        .assert()
        .success()
        .stdout_eq(file!["stdout.term.svg"])
        .stderr_eq(file!["stderr.term.svg"]);
}
