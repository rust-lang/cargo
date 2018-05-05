use super::project;

#[test]
fn fixes_missing_ampersand() {
    let p = project()
        .file("src/main.rs", r#"
            fn main() { let mut x = 3; drop(x); }
        "#)
        .file("src/lib.rs", r#"
            pub fn foo() { let mut x = 3; drop(x); }

            #[test]
            pub fn foo2() { let mut x = 3; drop(x); }
        "#)
        .file("tests/a.rs", r#"
            #[test]
            pub fn foo() { let mut x = 3; drop(x); }
        "#)
        .file("examples/foo.rs", r#"
            fn main() { let mut x = 3; drop(x); }
        "#)
        .file("build.rs", r#"
            fn main() { let mut x = 3; drop(x); }
        "#)
        .build();

    let stderr = "\
[COMPILING] foo v0.1.0 (CWD)
[FINISHED] dev [unoptimized + debuginfo]
";
    p.expect_cmd("cargo fix -- --all-targets").stdout("").stderr(stderr).run();
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
            #[cfg(feature = "bar")]
            pub fn foo() -> u32 { let mut x = 3; x }
        "#)
        .build();

    p.expect_cmd("cargo fix").run();
    p.expect_cmd("cargo build").run();
    p.expect_cmd("cargo fix -- --features bar").run();
    p.expect_cmd("cargo build --features bar").run();
}
