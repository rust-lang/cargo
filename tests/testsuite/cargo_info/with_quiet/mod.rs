use cargo_test_support::file;
use cargo_test_support::prelude::*;

use super::init_registry_without_token;

#[cargo_test]
fn case() {
    init_registry_without_token();
    for ver in [
        "0.1.1+my-package",
        "0.2.0+my-package",
        "0.2.3+my-package",
        "0.4.1+my-package",
        "20.0.0+my-package",
        "99999.0.0+my-package",
        "99999.0.0-alpha.1+my-package",
    ] {
        cargo_test_support::registry::Package::new("my-package", ver).publish();
    }

    snapbox::cmd::Command::cargo_ui()
        .arg("info")
        .arg("my-package")
        .arg("--quiet")
        .arg("--registry=dummy-registry")
        .assert()
        .success()
        .stdout_eq(file!["stdout.term.svg"])
        .stderr_eq("");
}
