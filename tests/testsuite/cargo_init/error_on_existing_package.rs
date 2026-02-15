use crate::prelude::*;
use cargo_test_support::paths;
use cargo_test_support::str;
use std::fs;

#[cargo_test]
fn init_error_on_existing_package() {
    let project_root = paths::root().join("foo");
    fs::create_dir_all(&project_root).unwrap();
    fs::write(project_root.join("Cargo.toml"), "").unwrap();

    snapbox::cmd::Command::cargo_ui()
        .arg_line("init --color=never")
        .current_dir(&project_root)
        .assert()
        .code(101)
        .stderr_eq(str![[r#"
    Creating binary (application) package
error: `cargo init` cannot be run on existing Cargo packages
help: use `cargo new` to create a package in a new subdirectory

"#]]);
}
