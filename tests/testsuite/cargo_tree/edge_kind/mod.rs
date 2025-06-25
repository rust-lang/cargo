use crate::prelude::*;
use cargo_test_support::file;
use cargo_test_support::project;
use cargo_test_support::registry::Package;

#[cargo_test]
fn case() {
    Package::new("normal_a", "1.0.0")
        .dep("normal_b", "1.0")
        .publish();
    Package::new("normal_b", "1.0.0")
        .dep("normal_c", "1.0")
        .build_dep("normal_b_build_a", "1.0.0")
        .dev_dep("normal_b_dev_a", "1.0.0")
        .publish();
    Package::new("normal_c", "1.0.0").publish();
    Package::new("normal_b_build_a", "1.0.0")
        .dep("normal_b_build_a_normal_a", "1.0.0")
        .publish();
    Package::new("normal_b_build_a_normal_a", "1.0.0").publish();
    Package::new("normal_b_dev_a", "1.0.0")
        .dep("normal_b_dev_a_normal_a", "1.0.0")
        .publish();
    Package::new("normal_b_dev_a_normal_a", "1.0.0").publish();
    Package::new("normal_d", "1.0.0").publish();

    Package::new("build_a", "1.0.0")
        .dep("build_b", "1.0")
        .publish();
    Package::new("build_b", "1.0.0")
        .dep("build_c", "1.0")
        .build_dep("build_b_build_a", "1.0.0")
        .dev_dep("build_b_dev_a", "1.0.0")
        .publish();
    Package::new("build_c", "1.0.0").publish();
    Package::new("build_b_build_a", "1.0.0")
        .dep("build_b_build_a_normal_a", "1.0.0")
        .publish();
    Package::new("build_b_build_a_normal_a", "1.0.0").publish();
    Package::new("build_b_dev_a", "1.0.0")
        .dep("build_b_dev_a_normal_a", "1.0.0")
        .publish();
    Package::new("build_b_dev_a_normal_a", "1.0.0").publish();
    Package::new("build_d", "1.0.0").publish();

    Package::new("dev_a", "1.0.0").dep("dev_b", "1.0").publish();
    Package::new("dev_b", "1.0.0")
        .dep("dev_c", "1.0")
        .build_dep("dev_b_build_a", "1.0.0")
        .dev_dep("dev_b_dev_a", "1.0.0")
        .publish();
    Package::new("dev_c", "1.0.0").publish();
    Package::new("dev_b_build_a", "1.0.0")
        .dep("dev_b_build_a_normal_a", "1.0.0")
        .publish();
    Package::new("dev_b_build_a_normal_a", "1.0.0").publish();
    Package::new("dev_b_dev_a", "1.0.0")
        .dep("dev_b_dev_a_normal_a", "1.0.0")
        .publish();
    Package::new("dev_b_dev_a_normal_a", "1.0.0").publish();
    Package::new("dev_d", "1.0.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"

            [features]
            default = ["foo"]
            foo = ["dep:normal_a"]

            [dependencies]
            normal_a = { version = "1.0", optional = true }
            normal_d = "1.0"

            [build-dependencies]
            build_a = "1.0"
            build_d = "1.0"

            [dev-dependencies]
            dev_a = "1.0"
            dev_d = "1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .file("build.rs", "fn main() {}")
        .build();

    snapbox::cmd::Command::cargo_ui()
        .arg("tree")
        .arg("--edges=features")
        .current_dir(p.root())
        .assert()
        .success()
        .stdout_eq(file!["stdout.term.svg"])
        .stderr_eq(file!["stderr.term.svg"]);
}
