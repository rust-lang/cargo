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

    p.expect_cmd("cargo fix -- --all-targets")
        .stdout("")
        .stderr_contains("[COMPILING] foo v0.1.0 (CWD)")
        .stderr_contains("[FIXING] build.rs (1 fix)")
        // Don't assert number of fixes for this one, as we don't know if we're
        // fixing it once or twice! We run this all concurrently, and if we
        // compile (and fix) in `--test` mode first, we get two fixes. Otherwise
        // we'll fix one non-test thing, and then fix another one later in
        // test mode.
        .stderr_contains("[FIXING] src/lib.rs")
        .stderr_contains("[FIXING] src/main.rs (1 fix)")
        .stderr_contains("[FIXING] examples/foo.rs (1 fix)")
        .stderr_contains("[FIXING] tests/a.rs (1 fix)")
        .stderr_contains("[FINISHED] dev [unoptimized + debuginfo]")
        .run();
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
