use cargo_test_support::file;
use cargo_test_support::prelude::*;
use cargo_test_support::project;
use cargo_test_support::registry::Package;

#[cargo_test]
fn case() {
    Package::new("a", "1.0.0").dep("b", "1.0").publish();
    Package::new("b", "1.0.0").dep("c", "1.0").publish();
    Package::new("c", "1.0.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"

            [dependencies]
            a = "1.0"
            b = "1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .file("build.rs", "fn main() {}")
        .build();

    snapbox::cmd::Command::cargo_ui()
        .arg("tree")
        .current_dir(p.root())
        .assert()
        .success()
        .stdout_eq(file!["stdout.term.svg"])
        .stderr_eq(file!["stderr.term.svg"]);
}
