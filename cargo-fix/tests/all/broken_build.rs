use super::project;

#[test]
fn do_not_fix_broken_builds() {
    let p = project()
        .file(
            "src/lib.rs",
            r#"
                pub fn foo() {
                    let mut x = 3;
                    drop(x);
                }

                pub fn foo2() {
                    let _x: u32 = "a";
                }
            "#
        )
        .build();

    p.expect_cmd("cargo-fix fix")
        .status(101)
        .run();
    assert!(p.read("src/lib.rs").contains("let mut x = 3;"));
}

