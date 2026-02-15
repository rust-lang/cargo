use crate::prelude::*;
use cargo_test_support::paths;
use cargo_test_support::str;
use std::fs;

#[cargo_test]
fn init_with_reserved_name_core() {
    let project_root = paths::root().join("core");
    fs::create_dir_all(&project_root).unwrap();

    snapbox::cmd::Command::cargo_ui()
        .arg_line("init --color=never")
        .current_dir(&project_root)
        .assert()
        .stderr_eq(str![[r#"
    Creating binary (application) package
warning: package name `core` may be confused with the package with that name in Rust's standard library
It is recommended to use a different name to avoid problems.
note: the directory name is used as the package name
help: to override the package name, pass `--name <pkgname>`
help: to name the binary "core", use a valid package name, and set the binary name to be different from the package. This can be done by setting the binary filename to `src/bin/core.rs` or change the name in Cargo.toml with:

    [[bin]]
    name = "core"
    path = "src/main.rs"

note: see more `Cargo.toml` keys and their definitions at https://doc.rust-lang.org/cargo/reference/manifest.html

"#]]);
}
