use crate::prelude::*;
use cargo_test_support::{basic_manifest, file, git, project};

use super::init_registry_without_token;

#[cargo_test]
fn case() {
    init_registry_without_token();
    let baz = git::new("baz", |project| {
        project
            .file("Cargo.toml", &basic_manifest("baz", "0.1.0"))
            .file("src/lib.rs", "")
    });

    let foo = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"
                    version = "0.1.0"

                    [dependencies]
                    baz = {{ git = '{}' }}
                "#,
                baz.url()
            ),
        )
        .file("src/lib.rs", "")
        .build();

    let project_root = foo.root();
    let cwd = &project_root;

    snapbox::cmd::Command::cargo_ui()
        .arg("info")
        .arg_line("--verbose foo")
        .current_dir(cwd)
        .assert()
        .success()
        .stdout_eq(file!["stdout.term.svg"])
        .stderr_eq("");
}
