use cargo_test_support::compare::assert_ui;
use cargo_test_support::current_dir;
use cargo_test_support::file;
use cargo_test_support::prelude::*;
use cargo_test_support::str;
use cargo_test_support::Project;

#[cargo_test]
fn case() {
    cargo_test_support::registry::init();
    cargo_test_support::registry::Package::new("my-package", "0.1.0").publish();

    let project = Project::from_template(current_dir!().join("in"));
    let project_root = project.root();
    let cwd = &project_root;

    snapbox::cmd::Command::cargo_ui()
        .arg("add")
        .arg_line("my-package")
        .current_dir(cwd)
        .assert()
        .success()
        .stdout_eq(str![""])
        .stderr_eq(file!["stderr.term.svg"]);

    // Verify the content matches first (for nicer error output)
    assert_ui().subset_matches(current_dir!().join("out"), &project_root);

    // Snapbox normalizes lines so we also need to do a string comparision to verify line endings
    let expected = current_dir!().join("out/Cargo.toml");
    let actual = project_root.join("Cargo.toml");
    assert_eq!(
        std::fs::read_to_string(expected).unwrap(),
        std::fs::read_to_string(actual).unwrap()
    );
}
