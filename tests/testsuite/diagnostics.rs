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
        .with_stderr(
            "\
error: invalid type: map, expected a sequence
 --> Cargo.toml:6:3
  |
6 | [[bench.foo]]
  |   ^^^^^
  |
",
        )
        .run();
}
