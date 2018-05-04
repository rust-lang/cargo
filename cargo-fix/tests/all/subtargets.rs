use super::project;

#[test]
fn fixes_missing_ampersand() {
    let p = project()
        .file("src/main.rs", r#"
            fn add(a: &u32) -> u32 { a + 1 }
            fn main() { add(1); }
        "#)
        .file("src/lib.rs", r#"
            fn add(a: &u32) -> u32 { a + 1 }
            pub fn foo() -> u32 { add(1) }

            #[test]
            pub fn foo2() { add(1); }
        "#)
        .file("tests/a.rs", r#"
            fn add(a: &u32) -> u32 { a + 1 }
            #[test]
            pub fn foo() { add(1); }
        "#)
        .file("examples/foo.rs", r#"
            fn add(a: &u32) -> u32 { a + 1 }
            fn main() { add(1); }
        "#)
        .file("build.rs", r#"
            fn add(a: &u32) -> u32 { a + 1 }
            fn main() { add(1); }
        "#)
        .build();

    let stderr = "\
[COMPILING] foo v0.1.0 (CWD)
[FINISHED] dev [unoptimized + debuginfo]
";
    p.expect_cmd("cargo fix --all-targets").stdout("").stderr(stderr).run();
    p.expect_cmd("cargo build").run();
    p.expect_cmd("cargo test").run();
}

#[test]
fn fix_features() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [features]
                bar = []

                [workspace]
            "#
        )
        .file("src/lib.rs", r#"
            fn add(a: &u32) -> u32 { a + 1 }

            #[cfg(feature = "bar")]
            pub fn foo() -> u32 { add(1) }
        "#)
        .build();

    p.expect_cmd("cargo fix").run();
    p.expect_cmd("cargo build").run();
    p.expect_cmd("cargo fix --features bar").run();
    p.expect_cmd("cargo build --features bar").run();
}
