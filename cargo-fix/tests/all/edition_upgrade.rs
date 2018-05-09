//! Test that we can use cargo-fix to upgrade our code to work with the 2018
//! edition.
//!
//! We'll trigger the `absolute_path_starting_with_module` lint which should
//! transform a `use ::foo;` where `foo` is local to `use crate::foo;`.

use super::project;

#[test]
fn prepare_for_2018() {
    let p = project()
        .file(
            "src/lib.rs",
            r#"
                #![feature(crate_in_paths)]

                mod foo {
                    pub const FOO: &str = "fooo";
                }

                mod bar {
                    use ::foo::FOO;
                }

                fn main() {
                    let x = ::foo::FOO;
                }
            "#,
        )
        .build();

    let stderr = "\
[CHECKING] foo v0.1.0 (CWD)
[FIXING] src/lib.rs (2 fixes)
[FINISHED] dev [unoptimized + debuginfo]
";
    p.expect_cmd("cargo-fix fix --prepare-for 2018")
        .stdout("")
        .stderr(stderr)
        .run();

    println!("{}", p.read("src/lib.rs"));
    assert!(p.read("src/lib.rs").contains("use crate::foo::FOO;"));
    assert!(p.read("src/lib.rs").contains("let x = crate::foo::FOO;"));
}
