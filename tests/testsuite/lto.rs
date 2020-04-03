use cargo_test_support::project;
use cargo_test_support::registry::Package;

#[cargo_test]
fn with_deps() {
    Package::new("bar", "0.0.1").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "test"
                version = "0.0.0"

                [dependencies]
                bar = "*"

                [profile.release]
                lto = true
            "#,
        )
        .file("src/main.rs", "extern crate bar; fn main() {}")
        .build();
    p.cargo("build -v --release").run();
}
