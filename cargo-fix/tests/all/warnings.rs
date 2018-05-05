use super::project;

#[test]
fn shows_warnings() {
    let p = project()
        .file(
            "src/lib.rs",
            r#"
            use std::default::Default;

            pub fn foo() {
            }
        "#,
        )
        .build();

    p.expect_cmd("cargo fix")
        .stderr_contains("warning: unused import")
        .run();
}
