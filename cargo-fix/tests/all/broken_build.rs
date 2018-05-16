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
            "#,
        )
        .build();

    p.expect_cmd("cargo-fix fix").status(101).run();
    assert!(p.read("src/lib.rs").contains("let mut x = 3;"));
}

#[test]
fn fix_broken_if_requested() {
    let p = project()
        .file(
            "src/lib.rs",
            r#"
                fn foo(a: &u32) -> u32 { a + 1 }
                pub fn bar() {
                    foo(1);
                }
            "#,
        )
        .build();

    p.expect_cmd("cargo-fix fix --broken-code").status(0).run();
}

#[test]
fn broken_fixes_backed_out() {
    let p = project()
        .file(
            "foo/Cargo.toml",
            r#"
                [package]
                name = 'foo'
                version = '0.1.0'
                [workspace]
            "#,
        )
        .file(
            "foo/src/main.rs",
            r##"
                use std::env;
                use std::fs;
                use std::io::Write;
                use std::path::{Path, PathBuf};
                use std::process::{self, Command};

                fn main() {
                    let is_lib_rs = env::args_os()
                        .map(PathBuf::from)
                        .any(|l| l == Path::new("src/lib.rs"));
                    if is_lib_rs {
                        let path = PathBuf::from(env::var_os("OUT_DIR").unwrap());
                        let path = path.join("foo");
                        if path.exists() {
                            fs::File::create("src/lib.rs")
                                .unwrap()
                                .write_all(b"not rust code")
                                .unwrap();
                        } else {
                            fs::File::create(&path).unwrap();
                        }
                    }

                    let status = Command::new("rustc")
                        .args(env::args().skip(1))
                        .status()
                        .expect("failed to run rustc");
                    process::exit(status.code().unwrap_or(2));
                }
            "##,
        )
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = 'bar'
                version = '0.1.0'
                [workspace]
            "#,
        )
        .file("bar/build.rs", "fn main() {}")
        .file(
            "bar/src/lib.rs",
            r#"
                pub fn foo() {
                    let mut x = 3;
                    drop(x);
                }
            "#,
        )
        .build();

    // Build our rustc shim
    p.expect_cmd("cargo build").cwd("foo").run();

    // Attempt to fix code, but our shim will always fail the second compile
    p.expect_cmd("cargo-fix fix")
        .cwd("bar")
        .env("RUSTC", p.root.join("foo/target/debug/foo"))
        .stderr_contains("not rust code")
        .stderr_contains(
            "\
             warning: failed to automatically apply fixes suggested by rustc \
             to crate `bar`\n\
             \n\
             after fixes were automatically applied the compiler reported \
             errors within these files:\n\
             \n  \
             * src/lib.rs\n\
             \n\
             This likely indicates a bug in either rustc or rustfix itself,\n\
             and we would appreciate a bug report! You're likely to see \n\
             a number of compiler warnings after this message which rustfix\n\
             attempted to fix but failed. If you could open an issue at\n\
             https://github.com/rust-lang-nursery/rustfix/issues\n\
             quoting the full output of this command we'd be very appreciative!\n\n\
             ",
        )
        .stderr_not_contains("[FIXING]")
        .status(101)
        .run();
}
