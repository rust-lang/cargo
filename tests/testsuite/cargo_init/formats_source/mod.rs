use crate::prelude::*;
use cargo_test_support::compare::assert_ui;
use cargo_test_support::current_dir;
use cargo_test_support::file;
use cargo_test_support::str;
use cargo_test_support::{Project, process};

#[cargo_test]
fn case() {
    // This cannot use `requires_rustfmt` because rustfmt is not available in
    // the rust-lang/rust environment. Additionally, if running cargo without
    // rustup (but with rustup installed), this test also fails due to HOME
    // preventing the proxy from choosing a toolchain.
    if let Err(e) = process("rustfmt").arg("-V").exec_with_output() {
        eprintln!("skipping test, rustfmt not available:\n{e:?}");
        return;
    }
    let project = Project::from_template(current_dir!().join("in"));
    let project_root = &project.root();

    snapbox::cmd::Command::cargo_ui()
        .arg_line("init --lib --vcs none")
        .current_dir(project_root)
        .assert()
        .success()
        .stdout_eq(str![""])
        .stderr_eq(file!["stderr.term.svg"]);

    assert_ui().subset_matches(current_dir!().join("out"), project_root);
}
