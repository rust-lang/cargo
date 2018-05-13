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
                #![allow(unused)]
                #![feature(rust_2018_preview)]

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

#[test]
fn local_paths() {
    let p = project()
        .file(
            "src/lib.rs",
            r#"
                #![feature(rust_2018_preview)]

                use test::foo;

                mod test {
                    pub fn foo() {}
                }

                pub fn f() {
                    foo();
                }
            "#,
        )
        .build();

    let stderr = "\
[CHECKING] foo v0.1.0 (CWD)
[FIXING] src/lib.rs (1 fix)
[FINISHED] dev [unoptimized + debuginfo]
";
    p.expect_cmd("cargo-fix fix --prepare-for 2018")
        .stdout("")
        .stderr(stderr)
        .run();

    println!("{}", p.read("src/lib.rs"));
    assert!(p.read("src/lib.rs").contains("use crate::test::foo;"));
}

#[test]
fn local_paths_no_fix() {
    let p = project()
        .file(
            "src/lib.rs",
            r#"
                use test::foo;

                mod test {
                    pub fn foo() {}
                }

                pub fn f() {
                    foo();
                }
            "#,
        )
        .build();

    let stderr = "\
[CHECKING] foo v0.1.0 (CWD)
[FINISHED] dev [unoptimized + debuginfo]
";
    p.expect_cmd("cargo-fix fix --prepare-for 2018")
        .stdout("")
        .stderr(stderr)
        .run();
}

#[test]
fn upgrade_extern_crate() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["edition"]

                [package]
                name = "foo"
                version = "0.1.0"
                edition = '2018'

                [workspace]

                [dependencies]
                bar = { path = 'bar' }
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
                #![warn(rust_2018_idioms)]

                extern crate bar;

                use bar::bar;

                pub fn foo() {
                    ::bar::bar();
                    bar();
                }
            "#,
        )
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.1.0"
            "#,
        )
        .file("bar/src/lib.rs", "pub fn bar() {}")
        .build();

    let stderr = "\
[CHECKING] bar v0.1.0 (CWD/bar)
[CHECKING] foo v0.1.0 (CWD)
[FIXING] src/lib.rs (1 fix)
[FINISHED] dev [unoptimized + debuginfo]
";
    p.expect_cmd("cargo-fix fix")
        .stdout("")
        .stderr(stderr)
        .run();

    println!("{}", p.read("src/lib.rs"));
    assert!(!p.read("src/lib.rs").contains("extern crate"));
}
