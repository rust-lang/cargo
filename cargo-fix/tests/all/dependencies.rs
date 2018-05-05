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

            pub fn foo() -> u32 {
                let mut x = 3;
                x
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
            pub fn foo() -> u32 {
                let mut x = 3;
                x
            }
        "#)
        .build();

    let stderr = "\
[CHECKING] bar v0.1.0 (CWD/bar)
[CHECKING] foo v0.1.0 (CWD)
[FINISHED] dev [unoptimized + debuginfo]
";
    p.expect_cmd("cargo-fix fix")
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
            pub fn foo() -> u32 {
                let mut x = 3;
                x
            }
        "#)
        .build();

    p.expect_cmd("cargo-fix fix")
        .cwd("foo")
        .status(0)
        .run();
    assert!(p.read("bar/src/lib.rs").contains("mut"));
}
