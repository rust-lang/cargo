use super::project;

#[test]
fn no_changes_necessary() {
    let p = project().file("src/lib.rs", "").build();

    let stderr = "\
[CHECKING] foo v0.1.0 (CWD)
[FINISHED] dev [unoptimized + debuginfo]
";
    p.expect_cmd("cargo-fix fix")
        .stdout("")
        .stderr(stderr)
        .run();
}

#[test]
fn fixes_extra_mut() {
    let p = project()
        .file(
            "src/lib.rs",
            r#"
                pub fn foo() -> u32 {
                    let mut x = 3;
                    x
                }
            "#,
        )
        .build();

    let stderr = "\
[CHECKING] foo v0.1.0 (CWD)
[FIXING] src/lib.rs (1 fix)
[FINISHED] dev [unoptimized + debuginfo]
";
    p.expect_cmd("cargo-fix fix")
        .stdout("")
        .stderr(stderr)
        .run();
}

#[test]
fn fixes_two_missing_ampersands() {
    let p = project()
        .file(
            "src/lib.rs",
            r#"
                pub fn foo() -> u32 {
                    let mut x = 3;
                    let mut y = 3;
                    x + y
                }
            "#,
        )
        .build();

    let stderr = "\
[CHECKING] foo v0.1.0 (CWD)
[FIXING] src/lib.rs (2 fixes)
[FINISHED] dev [unoptimized + debuginfo]
";
    p.expect_cmd("cargo-fix fix")
        .stdout("")
        .stderr(stderr)
        .run();
}

#[test]
fn tricky() {
    let p = project()
        .file(
            "src/lib.rs",
            r#"
                pub fn foo() -> u32 {
                    let mut x = 3; let mut y = 3;
                    x + y
                }
            "#,
        )
        .build();

    let stderr = "\
[CHECKING] foo v0.1.0 (CWD)
[FIXING] src/lib.rs (2 fixes)
[FINISHED] dev [unoptimized + debuginfo]
";
    p.expect_cmd("cargo-fix fix")
        .stdout("")
        .stderr(stderr)
        .run();
}

#[test]
fn preserve_line_endings() {
    let p = project()
        .file(
            "src/lib.rs",
            "\
                 fn add(a: &u32) -> u32 { a + 1 }\r\n\
                 pub fn foo() -> u32 { let mut x = 3; add(&x) }\r\n\
             ",
        )
        .build();

    p.expect_cmd("cargo-fix fix").run();
    assert!(p.read("src/lib.rs").contains("\r\n"));
}

#[test]
fn fix_deny_warnings() {
    let p = project()
        .file(
            "src/lib.rs",
            "\
                #![deny(warnings)]
                pub fn foo() { let mut x = 3; drop(x); }
            ",
        )
        .build();

    p.expect_cmd("cargo-fix fix").run();
}

#[test]
fn fix_deny_warnings_but_not_others() {
    let p = project()
        .file(
            "src/lib.rs",
            "
                #![deny(warnings)]

                pub fn foo() -> u32 {
                    let mut x = 3;
                    x
                }

                fn bar() {}
            ",
        )
        .build();

    p.expect_cmd("cargo-fix fix").run();
    assert!(!p.read("src/lib.rs").contains("let mut x = 3;"));
    assert!(p.read("src/lib.rs").contains("fn bar() {}"));
}
