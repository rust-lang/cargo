use crate::prelude::*;
use cargo_test_support::Project;
use cargo_test_support::compare::assert_ui;
use cargo_test_support::current_dir;
use cargo_test_support::file;
use cargo_test_support::str;

#[cargo_test]
fn case() {
    cargo_test_support::registry::init();

    let main_manifest = r#"
    [package]
    name = "main-package"
    version = "0.1.1+main-package"
    authors = []

    [workspace]
    members = ["package-wo-feature", "package-with-feature"]
    "#;

    let manifest_feature = r#"
    [package]
    name = "package-with-feature"
    version = "0.1.3+package-with-feature"
    [features]
    target_feature = []
    "#;

    let project = Project::from_template(current_dir!().join("in"));
    let project_root = project.root();
    let cwd = &project_root;
    let git_dep = cargo_test_support::git::new("git-package", |project| {
        project
            .file("Cargo.toml", &main_manifest)
            .file(
                "package-wo-feature/Cargo.toml",
                &cargo_test_support::basic_manifest(
                    "package-wo-feature",
                    "0.1.1+package-wo-feature",
                ),
            )
            .file("package-wo-feature/src/lib.rs", "")
            .file("package-with-feature/Cargo.toml", &manifest_feature)
            .file("package-with-feature/src/lib.rs", "")
    });
    let git_url = git_dep.url().to_string();

    snapbox::cmd::Command::cargo_ui()
        .arg("add")
        .args([
            "--git",
            &git_url,
            "package-with-feature",
            "--features=target_feature",
        ])
        .current_dir(cwd)
        .assert()
        .success()
        .stdout_eq(str![""])
        .stderr_eq(file!["stderr.term.svg"]);

    assert_ui().subset_matches(current_dir!().join("out"), &project_root);
}
