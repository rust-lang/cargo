use super::project;

#[test]
fn no_changes_necessary() {
    let p = project()
        .file("src/lib.rs", "")
        .build();

    let stderr = "\
[CHECKING] foo v0.1.0 (CWD)
[FINISHED] dev [unoptimized + debuginfo]
";
    p.expect_cmd("cargo fix")
        .stdout("")
        .stderr(stderr)
        .run();
}

#[test]
fn fixes_missing_ampersand() {
    let p = project()
        .file("src/lib.rs", r#"
            fn add(a: &u32) -> u32 {
                a + 1
            }

            pub fn foo() -> u32 {
                add(1)
            }
        "#)
        .build();

    let stderr = "\
[CHECKING] foo v0.1.0 (CWD)
[FINISHED] dev [unoptimized + debuginfo]
";
    p.expect_cmd("cargo fix")
        .stdout("")
        .stderr(stderr)
        .run();
}

#[test]
fn fixes_two_missing_ampersands() {
    let p = project()
        .file("src/lib.rs", r#"
            fn add(a: &u32) -> u32 {
                a + 1
            }

            pub fn foo() -> u32 {
                add(1) + add(1)
            }
        "#)
        .build();

    let stderr = "\
[CHECKING] foo v0.1.0 (CWD)
[FINISHED] dev [unoptimized + debuginfo]
";
    p.expect_cmd("cargo fix")
        .stdout("")
        .stderr(stderr)
        .run();
}

#[test]
fn tricky_ampersand() {
    let p = project()
        .file("src/lib.rs", r#"
            fn add(a: &u32) -> u32 {
                a + 1
            }

            pub fn foo() -> u32 {
                add(1) + add(1)
            }
        "#)
        .build();

    let stderr = "\
[CHECKING] foo v0.1.0 (CWD)
[FINISHED] dev [unoptimized + debuginfo]
";
    p.expect_cmd("cargo fix")
        .stdout("")
        .stderr(stderr)
        .run();
}

#[test]
fn preserve_line_endings() {
    let p = project()
        .file("src/lib.rs", "\
            fn add(a: &u32) -> u32 { a + 1 }\r\n\
            pub fn foo() -> u32 { add(1) }\r\n\
        ")
        .build();

    p.expect_cmd("cargo fix").run();
    assert!(p.read("src/lib.rs").contains("\r\n"));
}

#[test]
fn multiple_suggestions_for_the_same_thing() {
    let p = project()
        .file("src/lib.rs", "\
            fn main() {
                let xs = vec![String::from(\"foo\")];
                // error: no diplay in scope, and there are two options
                // (std::path::Display and std::fmt::Display)
                let d: &Display = &xs;
                println!(\"{}\", d);
            }
        ")
        .build();

    let stderr = "\
[CHECKING] foo v0.1.0 (CWD)
error: Cannot replace slice of data that was already replaced
error: Could not compile `foo`.

To learn more, run the command again with --verbose.
";

    p.expect_cmd("cargo fix")
        .stderr(stderr)
        .status(101)
        .run();
}
