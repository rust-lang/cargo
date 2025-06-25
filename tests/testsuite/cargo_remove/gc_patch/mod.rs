use crate::prelude::*;
use cargo_test_support::basic_manifest;
use cargo_test_support::compare::assert_ui;
use cargo_test_support::current_dir;
use cargo_test_support::file;
use cargo_test_support::git;
use cargo_test_support::project;
use cargo_test_support::str;

#[cargo_test]
fn case() {
    cargo_test_support::registry::init();

    let git_project1 = git::new("bar1", |project| {
        project
            .file("Cargo.toml", &basic_manifest("bar", "0.1.0"))
            .file("src/lib.rs", "")
    })
    .url();

    let git_project2 = git::new("bar2", |project| {
        project
            .file("Cargo.toml", &basic_manifest("bar", "0.1.0"))
            .file("src/lib.rs", "")
    })
    .url();

    let git_project3 = git::new("bar3", |project| {
        project
            .file("Cargo.toml", &basic_manifest("bar", "0.1.0"))
            .file("src/lib.rs", "")
    })
    .url();

    let in_project = project()
        .file(
            "Cargo.toml",
            &format!(
                "[workspace]\n\
                 members = [ \"my-member\" ]\n\
                 \n\
                 [package]\n\
                 name = \"my-project\"\n\
                 version = \"0.1.0\"\n\
                 edition = \"2015\"\n\
                 \n\
                 [dependencies]\n\
                 bar = {{ git = \"{git_project1}\" }}\n\
                 \n\
                 [patch.\"{git_project1}\"]\n\
                 bar = {{ git = \"{git_project3}\" }}\n\
                 \n\
                 [patch.crates-io]\n\
                 bar = {{ git = \"{git_project2}\" }}\n",
            ),
        )
        .file("src/lib.rs", "")
        .file(
            "my-member/Cargo.toml",
            "[package]\n\
               name = \"my-member\"\n\
               version = \"0.1.0\"\n\
               \n\
               [dependencies]\n\
               bar = \"0.1.0\"\n",
        )
        .file("my-member/src/lib.rs", "")
        .build();

    snapbox::cmd::Command::cargo_ui()
        .arg("remove")
        .args(["bar"])
        .current_dir(&in_project.root())
        .assert()
        .success()
        .stdout_eq(str![""])
        .stderr_eq(file!["stderr.term.svg"]);

    assert_ui().subset_matches(current_dir!().join("out"), &in_project.root());
}
