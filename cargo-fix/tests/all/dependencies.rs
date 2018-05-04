use super::project;

#[test]
fn fix_path_deps() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [dependencies]
                bar = { path = 'bar' }

                [workspace]
            "#
        )
        .file("src/lib.rs", r#"
            extern crate bar;

            fn add(a: &u32) -> u32 {
                a + 1
            }

            pub fn foo() -> u32 {
                add(1) + add(1)
            }
        "#)
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.1.0"
            "#
        )
        .file("bar/src/lib.rs", r#"
            fn add(a: &u32) -> u32 {
                a + 1
            }

            pub fn foo() -> u32 {
                add(1) + add(1)
            }
        "#)
        .build();

    let stderr = "\
[CHECKING] bar v0.1.0 (CWD/bar)
[CHECKING] foo v0.1.0 (CWD)
[FINISHED] dev [unoptimized + debuginfo]
";
    p.expect_cmd("cargo fix")
        .stdout("")
        .stderr(stderr)
        .run();
}

#[test]
fn do_not_fix_non_relevant_deps() {
    let p = project()
        .file(
            "foo/Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [dependencies]
                bar = { path = '../bar' }

                [workspace]
            "#
        )
        .file("foo/src/lib.rs", "")
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.1.0"
            "#
        )
        .file("bar/src/lib.rs", r#"
            fn add(a: &u32) -> u32 {
                a + 1
            }

            pub fn foo() -> u32 {
                add(1) + add(1)
            }
        "#)
        .build();

    p.expect_cmd("cargo fix")
        .cwd("foo")
        .status(101)
        .run();
}
