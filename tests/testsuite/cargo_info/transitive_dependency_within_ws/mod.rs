use crate::prelude::*;
use cargo_test_support::{Project, compare::assert_ui, current_dir, file};

use super::init_registry_without_token;

#[cargo_test]
fn case() {
    init_registry_without_token();
    // 99.0.0 is unused
    for ver in ["1.0.0", "2.0.0", "3.0.0", "99.0.0"] {
        cargo_test_support::registry::Package::new("my-package", ver).publish();
    }
    // Dep1 depends on 3.0.0, Dep2 depends on 2.0.0, Dep3 depends on 1.0.0
    cargo_test_support::registry::Package::new("dep1", "1.0.0")
        .dep("my-package", "1.0.0")
        .publish();
    cargo_test_support::registry::Package::new("dep2", "1.0.0")
        .dep("my-package", "2.0.0")
        .publish();
    cargo_test_support::registry::Package::new("dep3", "1.0.0")
        .dep("my-package", "3.0.0")
        .publish();

    let project = Project::from_template(current_dir!().join("in"));
    let project_root = project.root();
    let transitive1_root = project_root.join("crates/transitive1");
    let transitive2_root = project_root.join("crates/transitive2");
    let direct1_root = project_root.join("crates/direct1");
    let direct2_root = project_root.join("crates/direct2");
    let ws_directory = &project_root;
    let transitive1_directory = &transitive1_root;
    let transitive2_directory = &transitive2_root;
    let direct1_directory = &direct1_root;
    let direct2_directory = &direct2_root;

    snapbox::cmd::Command::cargo_ui()
        .arg("info")
        .arg("my-package")
        .arg("--registry=dummy-registry")
        .current_dir(ws_directory)
        .assert()
        .stdout_eq(file!["ws-stdout.term.svg"])
        .stderr_eq(file!["ws-stderr.term.svg"]);
    snapbox::cmd::Command::cargo_ui()
        .arg("info")
        .arg("my-package")
        .arg("--registry=dummy-registry")
        .current_dir(transitive1_directory)
        .assert()
        .stdout_eq(file!["transitive1-stdout.term.svg"])
        .stderr_eq(file!["transitive1-stderr.term.svg"]);
    snapbox::cmd::Command::cargo_ui()
        .arg("info")
        .arg("my-package")
        .arg("--registry=dummy-registry")
        .current_dir(transitive2_directory)
        .assert()
        .stdout_eq(file!["transitive2-stdout.term.svg"])
        .stderr_eq(file!["transitive2-stderr.term.svg"]);
    snapbox::cmd::Command::cargo_ui()
        .arg("info")
        .arg("my-package")
        .arg("--registry=dummy-registry")
        .current_dir(direct1_directory)
        .assert()
        .stdout_eq(file!["direct1-stdout.term.svg"])
        .stderr_eq(file!["direct1-stderr.term.svg"]);
    snapbox::cmd::Command::cargo_ui()
        .arg("info")
        .arg("my-package")
        .arg("--registry=dummy-registry")
        .current_dir(direct2_directory)
        .assert()
        .stdout_eq(file!["direct2-stdout.term.svg"])
        .stderr_eq(file!["direct2-stderr.term.svg"]);

    assert_ui().subset_matches(current_dir!().join("out"), &project_root);
}
