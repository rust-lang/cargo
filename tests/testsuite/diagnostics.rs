use cargo_test_support::project;

#[cargo_test]
fn dont_panic_on_render() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
[package]
name = "foo"
version = "0.1.0"
edition = "2021"
[[bench.foo]]
"#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_contains("attempt to subtract with overflow")
        .run();
}
