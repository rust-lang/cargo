use crate::prelude::*;
use cargo_test_support::Project;
use cargo_test_support::compare::assert_ui;
use cargo_test_support::current_dir;
use cargo_test_support::file;
use cargo_test_support::str;

#[cargo_test]
fn case() {
    cargo_test_support::registry::init();
    cargo_test_support::registry::Package::new("rust-version-user", "0.1.0")
        .rust_version("1.30")
        .publish();
    cargo_test_support::registry::Package::new("rust-version-user", "0.1.1")
        .rust_version("1.30")
        .publish();
    cargo_test_support::registry::Package::new("rust-version-user", "0.2.1")
        .rust_version("1.2345")
        .publish();

    let project = Project::from_template(current_dir!().join("in"));
    let project_root = project.root();
    let cwd = &project_root;

    snapbox::cmd::Command::cargo_ui()
        .arg("add")
        .arg("--ignore-rust-version")
        .arg_line("rust-version-user")
        .current_dir(cwd)
        .env("CARGO_RESOLVER_INCOMPATIBLE_RUST_VERSIONS", "fallback")
        .assert()
        .code(0)
        .stdout_eq(str![""])
        .stderr_eq(file!["stderr.term.svg"]);

    assert_ui().subset_matches(current_dir!().join("out"), &project_root);
}
