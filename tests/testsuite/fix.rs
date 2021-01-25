//! Tests for the `cargo fix` command.

use cargo_test_support::git;
use cargo_test_support::paths;
use cargo_test_support::{basic_manifest, project};

#[cargo_test]
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

    p.cargo("fix --allow-no-vcs")
        .env("__CARGO_FIX_YOLO", "1")
        .with_status(101)
        .with_stderr_contains("[ERROR] could not compile `foo`")
        .run();
    assert!(p.read_file("src/lib.rs").contains("let mut x = 3;"));
}

#[cargo_test]
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

    p.cargo("fix --allow-no-vcs --broken-code")
        .env("__CARGO_FIX_YOLO", "1")
        .run();
}

#[cargo_test]
fn broken_fixes_backed_out() {
    // This works as follows:
    // - Create a `rustc` shim (the "foo" project) which will pretend that the
    //   verification step fails.
    // - There is an empty build script so `foo` has `OUT_DIR` to track the steps.
    // - The first "check", `foo` creates a file in OUT_DIR, and it completes
    //   successfully with a warning diagnostic to remove unused `mut`.
    // - rustfix removes the `mut`.
    // - The second "check" to verify the changes, `foo` swaps out the content
    //   with something that fails to compile. It creates a second file so it
    //   won't do anything in the third check.
    // - cargo fix discovers that the fix failed, and it backs out the changes.
    // - The third "check" is done to display the original diagnostics of the
    //   original code.
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
            r#"
                use std::env;
                use std::fs;
                use std::io::Write;
                use std::path::{Path, PathBuf};
                use std::process::{self, Command};

                fn main() {
                    // Ignore calls to things like --print=file-names and compiling build.rs.
                    let is_lib_rs = env::args_os()
                        .map(PathBuf::from)
                        .any(|l| l == Path::new("src/lib.rs"));
                    if is_lib_rs {
                        let path = PathBuf::from(env::var_os("OUT_DIR").unwrap());
                        let first = path.join("first");
                        let second = path.join("second");
                        if first.exists() && !second.exists() {
                            fs::write("src/lib.rs", b"not rust code").unwrap();
                            fs::File::create(&second).unwrap();
                        } else {
                            fs::File::create(&first).unwrap();
                        }
                    }

                    let status = Command::new("rustc")
                        .args(env::args().skip(1))
                        .status()
                        .expect("failed to run rustc");
                    process::exit(status.code().unwrap_or(2));
                }
            "#,
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
    p.cargo("build").cwd("foo").run();

    // Attempt to fix code, but our shim will always fail the second compile
    p.cargo("fix --allow-no-vcs --lib")
        .cwd("bar")
        .env("__CARGO_FIX_YOLO", "1")
        .env("RUSTC", p.root().join("foo/target/debug/foo"))
        .with_stderr_contains(
            "warning: failed to automatically apply fixes suggested by rustc \
             to crate `bar`\n\
             \n\
             after fixes were automatically applied the compiler reported \
             errors within these files:\n\
             \n  \
             * src/lib.rs\n\
             \n\
             This likely indicates a bug in either rustc or cargo itself,\n\
             and we would appreciate a bug report! You're likely to see \n\
             a number of compiler warnings after this message which cargo\n\
             attempted to fix but failed. If you could open an issue at\n\
             [..]\n\
             quoting the full output of this command we'd be very appreciative!\n\
             Note that you may be able to make some more progress in the near-term\n\
             fixing code with the `--broken-code` flag\n\
             \n\
             The following errors were reported:\n\
             error: expected one of `!` or `::`, found `rust`\n\
             ",
        )
        .with_stderr_contains("Original diagnostics will follow.")
        .with_stderr_contains("[WARNING] variable does not need to be mutable")
        .with_stderr_does_not_contain("[..][FIXING][..]")
        .run();

    // Make sure the fix which should have been applied was backed out
    assert!(p.read_file("bar/src/lib.rs").contains("let mut x = 3;"));
}

#[cargo_test]
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
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
                extern crate bar;

                pub fn foo() -> u32 {
                    let mut x = 3;
                    x
                }
            "#,
        )
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file(
            "bar/src/lib.rs",
            r#"
                pub fn foo() -> u32 {
                    let mut x = 3;
                    x
                }
            "#,
        )
        .build();

    p.cargo("fix --allow-no-vcs -p foo -p bar")
        .env("__CARGO_FIX_YOLO", "1")
        .with_stdout("")
        .with_stderr_unordered(
            "\
[CHECKING] bar v0.1.0 ([..])
[FIXING] bar/src/lib.rs (1 fix)
[CHECKING] foo v0.1.0 ([..])
[FIXING] src/lib.rs (1 fix)
[FINISHED] [..]
",
        )
        .run();
}

#[cargo_test]
fn do_not_fix_non_relevant_deps() {
    let p = project()
        .no_manifest()
        .file(
            "foo/Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [dependencies]
                bar = { path = '../bar' }

                [workspace]
            "#,
        )
        .file("foo/src/lib.rs", "")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file(
            "bar/src/lib.rs",
            r#"
                pub fn foo() -> u32 {
                    let mut x = 3;
                    x
                }
            "#,
        )
        .build();

    p.cargo("fix --allow-no-vcs")
        .env("__CARGO_FIX_YOLO", "1")
        .cwd("foo")
        .run();

    assert!(p.read_file("bar/src/lib.rs").contains("mut"));
}

#[cargo_test]
fn prepare_for_2018() {
    let p = project()
        .file(
            "src/lib.rs",
            r#"
                #![allow(unused)]

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
[CHECKING] foo v0.0.1 ([..])
[FIXING] src/lib.rs (2 fixes)
[FINISHED] [..]
";
    p.cargo("fix --edition --allow-no-vcs")
        .with_stderr(stderr)
        .with_stdout("")
        .run();

    println!("{}", p.read_file("src/lib.rs"));
    assert!(p.read_file("src/lib.rs").contains("use crate::foo::FOO;"));
    assert!(p
        .read_file("src/lib.rs")
        .contains("let x = crate::foo::FOO;"));
}

#[cargo_test]
fn local_paths() {
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
[CHECKING] foo v0.0.1 ([..])
[FIXING] src/lib.rs (1 fix)
[FINISHED] [..]
";

    p.cargo("fix --edition --allow-no-vcs")
        .with_stderr(stderr)
        .with_stdout("")
        .run();

    println!("{}", p.read_file("src/lib.rs"));
    assert!(p.read_file("src/lib.rs").contains("use crate::test::foo;"));
}

#[cargo_test]
fn upgrade_extern_crate() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
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
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "pub fn bar() {}")
        .build();

    let stderr = "\
[CHECKING] bar v0.1.0 ([..])
[CHECKING] foo v0.1.0 ([..])
[FIXING] src/lib.rs (1 fix)
[FINISHED] [..]
";
    p.cargo("fix --allow-no-vcs")
        .env("__CARGO_FIX_YOLO", "1")
        .with_stderr(stderr)
        .with_stdout("")
        .run();
    println!("{}", p.read_file("src/lib.rs"));
    assert!(!p.read_file("src/lib.rs").contains("extern crate"));
}

#[cargo_test]
fn specify_rustflags() {
    let p = project()
        .file(
            "src/lib.rs",
            r#"
                #![allow(unused)]

                mod foo {
                    pub const FOO: &str = "fooo";
                }

                fn main() {
                    let x = ::foo::FOO;
                }
            "#,
        )
        .build();

    let stderr = "\
[CHECKING] foo v0.0.1 ([..])
[FIXING] src/lib.rs (1 fix)
[FINISHED] [..]
";
    p.cargo("fix --edition --allow-no-vcs")
        .env("RUSTFLAGS", "-C linker=cc")
        .with_stderr(stderr)
        .with_stdout("")
        .run();
}

#[cargo_test]
fn no_changes_necessary() {
    let p = project().file("src/lib.rs", "").build();

    let stderr = "\
[CHECKING] foo v0.0.1 ([..])
[FINISHED] [..]
";
    p.cargo("fix --allow-no-vcs")
        .with_stderr(stderr)
        .with_stdout("")
        .run();
}

#[cargo_test]
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
[CHECKING] foo v0.0.1 ([..])
[FIXING] src/lib.rs (1 fix)
[FINISHED] [..]
";
    p.cargo("fix --allow-no-vcs")
        .env("__CARGO_FIX_YOLO", "1")
        .with_stderr(stderr)
        .with_stdout("")
        .run();
}

#[cargo_test]
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
[CHECKING] foo v0.0.1 ([..])
[FIXING] src/lib.rs (2 fixes)
[FINISHED] [..]
";
    p.cargo("fix --allow-no-vcs")
        .env("__CARGO_FIX_YOLO", "1")
        .with_stderr(stderr)
        .with_stdout("")
        .run();
}

#[cargo_test]
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
[CHECKING] foo v0.0.1 ([..])
[FIXING] src/lib.rs (2 fixes)
[FINISHED] [..]
";
    p.cargo("fix --allow-no-vcs")
        .env("__CARGO_FIX_YOLO", "1")
        .with_stderr(stderr)
        .with_stdout("")
        .run();
}

#[cargo_test]
fn preserve_line_endings() {
    let p = project()
        .file(
            "src/lib.rs",
            "fn add(a: &u32) -> u32 { a + 1 }\r\n\
             pub fn foo() -> u32 { let mut x = 3; add(&x) }\r\n\
             ",
        )
        .build();

    p.cargo("fix --allow-no-vcs")
        .env("__CARGO_FIX_YOLO", "1")
        .run();
    assert!(p.read_file("src/lib.rs").contains("\r\n"));
}

#[cargo_test]
fn fix_deny_warnings() {
    let p = project()
        .file(
            "src/lib.rs",
            "#![deny(warnings)]
             pub fn foo() { let mut x = 3; drop(x); }
            ",
        )
        .build();

    p.cargo("fix --allow-no-vcs")
        .env("__CARGO_FIX_YOLO", "1")
        .run();
}

#[cargo_test]
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

    p.cargo("fix --allow-no-vcs")
        .env("__CARGO_FIX_YOLO", "1")
        .run();
    assert!(!p.read_file("src/lib.rs").contains("let mut x = 3;"));
    assert!(p.read_file("src/lib.rs").contains("fn bar() {}"));
}

#[cargo_test]
fn fix_two_files() {
    let p = project()
        .file(
            "src/lib.rs",
            "
                pub mod bar;

                pub fn foo() -> u32 {
                    let mut x = 3;
                    x
                }
            ",
        )
        .file(
            "src/bar.rs",
            "
                pub fn foo() -> u32 {
                    let mut x = 3;
                    x
                }

            ",
        )
        .build();

    p.cargo("fix --allow-no-vcs")
        .env("__CARGO_FIX_YOLO", "1")
        .with_stderr_contains("[FIXING] src/bar.rs (1 fix)")
        .with_stderr_contains("[FIXING] src/lib.rs (1 fix)")
        .run();
    assert!(!p.read_file("src/lib.rs").contains("let mut x = 3;"));
    assert!(!p.read_file("src/bar.rs").contains("let mut x = 3;"));
}

#[cargo_test]
fn fixes_missing_ampersand() {
    let p = project()
        .file("src/main.rs", "fn main() { let mut x = 3; drop(x); }")
        .file(
            "src/lib.rs",
            r#"
                pub fn foo() { let mut x = 3; drop(x); }

                #[test]
                pub fn foo2() { let mut x = 3; drop(x); }
            "#,
        )
        .file(
            "tests/a.rs",
            r#"
                #[test]
                pub fn foo() { let mut x = 3; drop(x); }
            "#,
        )
        .file("examples/foo.rs", "fn main() { let mut x = 3; drop(x); }")
        .file("build.rs", "fn main() { let mut x = 3; drop(x); }")
        .build();

    p.cargo("fix --all-targets --allow-no-vcs")
        .env("__CARGO_FIX_YOLO", "1")
        .with_stdout("")
        .with_stderr_contains("[COMPILING] foo v0.0.1 ([..])")
        .with_stderr_contains("[FIXING] build.rs (1 fix)")
        // Don't assert number of fixes for this one, as we don't know if we're
        // fixing it once or twice! We run this all concurrently, and if we
        // compile (and fix) in `--test` mode first, we get two fixes. Otherwise
        // we'll fix one non-test thing, and then fix another one later in
        // test mode.
        .with_stderr_contains("[FIXING] src/lib.rs[..]")
        .with_stderr_contains("[FIXING] src/main.rs (1 fix)")
        .with_stderr_contains("[FIXING] examples/foo.rs (1 fix)")
        .with_stderr_contains("[FIXING] tests/a.rs (1 fix)")
        .with_stderr_contains("[FINISHED] [..]")
        .run();
    p.cargo("build").run();
    p.cargo("test").run();
}

#[cargo_test]
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
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
                #[cfg(feature = "bar")]
                pub fn foo() -> u32 { let mut x = 3; x }
            "#,
        )
        .build();

    p.cargo("fix --allow-no-vcs").run();
    p.cargo("build").run();
    p.cargo("fix --features bar --allow-no-vcs").run();
    p.cargo("build --features bar").run();
}

#[cargo_test]
fn shows_warnings() {
    let p = project()
        .file(
            "src/lib.rs",
            "#[deprecated] fn bar() {} pub fn foo() { let _ = bar(); }",
        )
        .build();

    p.cargo("fix --allow-no-vcs")
        .with_stderr_contains("[..]warning: use of deprecated[..]")
        .run();
}

#[cargo_test]
fn warns_if_no_vcs_detected() {
    let p = project().file("src/lib.rs", "pub fn foo() {}").build();

    p.cargo("fix")
        .with_status(101)
        .with_stderr(
            "error: no VCS found for this package and `cargo fix` can potentially perform \
             destructive changes; if you'd like to suppress this error pass `--allow-no-vcs`\
             ",
        )
        .run();
    p.cargo("fix --allow-no-vcs").run();
}

#[cargo_test]
fn warns_about_dirty_working_directory() {
    let p = git::new("foo", |p| p.file("src/lib.rs", "pub fn foo() {}"));

    p.change_file("src/lib.rs", "");

    p.cargo("fix")
        .with_status(101)
        .with_stderr(
            "\
error: the working directory of this package has uncommitted changes, \
and `cargo fix` can potentially perform destructive changes; if you'd \
like to suppress this error pass `--allow-dirty`, `--allow-staged`, or \
commit the changes to these files:

  * src/lib.rs (dirty)


",
        )
        .run();
    p.cargo("fix --allow-dirty").run();
}

#[cargo_test]
fn warns_about_staged_working_directory() {
    let (p, repo) = git::new_repo("foo", |p| p.file("src/lib.rs", "pub fn foo() {}"));

    p.change_file("src/lib.rs", "pub fn bar() {}");
    git::add(&repo);

    p.cargo("fix")
        .with_status(101)
        .with_stderr(
            "\
error: the working directory of this package has uncommitted changes, \
and `cargo fix` can potentially perform destructive changes; if you'd \
like to suppress this error pass `--allow-dirty`, `--allow-staged`, or \
commit the changes to these files:

  * src/lib.rs (staged)


",
        )
        .run();
    p.cargo("fix --allow-staged").run();
}

#[cargo_test]
fn does_not_warn_about_clean_working_directory() {
    let p = git::new("foo", |p| p.file("src/lib.rs", "pub fn foo() {}"));
    p.cargo("fix").run();
}

#[cargo_test]
fn does_not_warn_about_dirty_ignored_files() {
    let p = git::new("foo", |p| {
        p.file("src/lib.rs", "pub fn foo() {}")
            .file(".gitignore", "bar\n")
    });

    p.change_file("bar", "");

    p.cargo("fix").run();
}

#[cargo_test]
fn fix_all_targets_by_default() {
    let p = project()
        .file("src/lib.rs", "pub fn foo() { let mut x = 3; drop(x); }")
        .file("tests/foo.rs", "pub fn foo() { let mut x = 3; drop(x); }")
        .build();
    p.cargo("fix --allow-no-vcs")
        .env("__CARGO_FIX_YOLO", "1")
        .run();
    assert!(!p.read_file("src/lib.rs").contains("let mut x"));
    assert!(!p.read_file("tests/foo.rs").contains("let mut x"));
}

#[cargo_test]
fn prepare_for_and_enable() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = 'foo'
                version = '0.1.0'
                edition = '2018'
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    let stderr = "\
error: cannot prepare for the 2018 edition when it is enabled, so cargo cannot
automatically fix errors in `src/lib.rs`

To prepare for the 2018 edition you should first remove `edition = '2018'` from
your `Cargo.toml` and then rerun this command. Once all warnings have been fixed
then you can re-enable the `edition` key in `Cargo.toml`. For some more
information about transitioning to the 2018 edition see:

  https://[..]

";
    p.cargo("fix --edition --allow-no-vcs")
        .with_stderr_contains(stderr)
        .with_status(101)
        .run();
}

#[cargo_test]
fn fix_overlapping() {
    let p = project()
        .file(
            "src/lib.rs",
            r#"
                pub fn foo<T>() {}
                pub struct A;

                pub mod bar {
                    pub fn baz() {
                        ::foo::<::A>();
                    }
                }
            "#,
        )
        .build();

    let stderr = "\
[CHECKING] foo [..]
[FIXING] src/lib.rs (2 fixes)
[FINISHED] dev [..]
";

    p.cargo("fix --allow-no-vcs --prepare-for 2018 --lib")
        .with_stderr(stderr)
        .run();

    let contents = p.read_file("src/lib.rs");
    println!("{}", contents);
    assert!(contents.contains("crate::foo::<crate::A>()"));
}

#[cargo_test]
fn fix_idioms() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = 'foo'
                version = '0.1.0'
                edition = '2018'
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
                use std::any::Any;
                pub fn foo() {
                    let _x: Box<Any> = Box::new(3);
                }
            "#,
        )
        .build();

    let stderr = "\
[CHECKING] foo [..]
[FIXING] src/lib.rs (1 fix)
[FINISHED] [..]
";
    p.cargo("fix --edition-idioms --allow-no-vcs")
        .with_stderr(stderr)
        .run();

    assert!(p.read_file("src/lib.rs").contains("Box<dyn Any>"));
}

#[cargo_test]
fn idioms_2015_ok() {
    let p = project().file("src/lib.rs", "").build();

    p.cargo("fix --edition-idioms --allow-no-vcs").run();
}

#[cargo_test]
fn both_edition_migrate_flags() {
    let p = project().file("src/lib.rs", "").build();

    let stderr = "\
error: The argument '--edition' cannot be used with '--prepare-for <prepare-for>'

USAGE:
    cargo[..] fix --edition

For more information try --help
";

    p.cargo("fix --prepare-for 2018 --edition")
        .with_status(1)
        .with_stderr(stderr)
        .run();
}

#[cargo_test]
fn shows_warnings_on_second_run_without_changes() {
    let p = project()
        .file(
            "src/lib.rs",
            r#"
                #[deprecated]
                fn bar() {}

                pub fn foo() {
                    let _ = bar();
                }
            "#,
        )
        .build();

    p.cargo("fix --allow-no-vcs")
        .with_stderr_contains("[..]warning: use of deprecated[..]")
        .run();

    p.cargo("fix --allow-no-vcs")
        .with_stderr_contains("[..]warning: use of deprecated[..]")
        .run();
}

#[cargo_test]
fn shows_warnings_on_second_run_without_changes_on_multiple_targets() {
    let p = project()
        .file(
            "src/lib.rs",
            r#"
                #[deprecated]
                fn bar() {}

                pub fn foo() {
                    let _ = bar();
                }
            "#,
        )
        .file(
            "src/main.rs",
            r#"
                #[deprecated]
                fn bar() {}

                fn main() {
                    let _ = bar();
                }
            "#,
        )
        .file(
            "tests/foo.rs",
            r#"
                #[deprecated]
                fn bar() {}

                #[test]
                fn foo_test() {
                    let _ = bar();
                }
            "#,
        )
        .file(
            "tests/bar.rs",
            r#"
                #[deprecated]
                fn bar() {}

                #[test]
                fn foo_test() {
                    let _ = bar();
                }
            "#,
        )
        .file(
            "examples/fooxample.rs",
            r#"
                #[deprecated]
                fn bar() {}

                fn main() {
                    let _ = bar();
                }
            "#,
        )
        .build();

    p.cargo("fix --allow-no-vcs --all-targets")
        .with_stderr_contains(" --> examples/fooxample.rs:6:29")
        .with_stderr_contains(" --> src/lib.rs:6:29")
        .with_stderr_contains(" --> src/main.rs:6:29")
        .with_stderr_contains(" --> tests/bar.rs:7:29")
        .with_stderr_contains(" --> tests/foo.rs:7:29")
        .run();

    p.cargo("fix --allow-no-vcs --all-targets")
        .with_stderr_contains(" --> examples/fooxample.rs:6:29")
        .with_stderr_contains(" --> src/lib.rs:6:29")
        .with_stderr_contains(" --> src/main.rs:6:29")
        .with_stderr_contains(" --> tests/bar.rs:7:29")
        .with_stderr_contains(" --> tests/foo.rs:7:29")
        .run();
}

#[cargo_test]
fn doesnt_rebuild_dependencies() {
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
            "#,
        )
        .file("src/lib.rs", "extern crate bar;")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "")
        .build();

    p.cargo("fix --allow-no-vcs -p foo")
        .env("__CARGO_FIX_YOLO", "1")
        .with_stdout("")
        .with_stderr(
            "\
[CHECKING] bar v0.1.0 ([..])
[CHECKING] foo v0.1.0 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();

    p.cargo("fix --allow-no-vcs -p foo")
        .env("__CARGO_FIX_YOLO", "1")
        .with_stdout("")
        .with_stderr(
            "\
[CHECKING] foo v0.1.0 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cargo_test]
#[cfg(unix)]
fn does_not_crash_with_rustc_wrapper() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("fix --allow-no-vcs")
        .env("RUSTC_WRAPPER", "/usr/bin/env")
        .run();
}

#[cargo_test]
#[cfg(unix)]
fn does_not_crash_with_rustc_workspace_wrapper() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("fix --allow-no-vcs --verbose")
        .env("RUSTC_WORKSPACE_WRAPPER", "/usr/bin/env")
        .run();
}

#[cargo_test]
fn uses_workspace_wrapper_and_primary_wrapper_override() {
    // We don't have /usr/bin/env on Windows.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("fix --allow-no-vcs --verbose")
        .env("RUSTC_WORKSPACE_WRAPPER", paths::echo_wrapper())
        .with_stderr_contains("WRAPPER CALLED: rustc src/lib.rs --crate-name foo [..]")
        .run();
}

#[cargo_test]
fn only_warn_for_relevant_crates() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [dependencies]
                a = { path = 'a' }
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "a/Cargo.toml",
            r#"
                [package]
                name = "a"
                version = "0.1.0"
            "#,
        )
        .file(
            "a/src/lib.rs",
            "
                pub fn foo() {}
                pub mod bar {
                    use foo;
                    pub fn baz() { foo() }
                }
            ",
        )
        .build();

    p.cargo("fix --allow-no-vcs --edition")
        .with_stderr(
            "\
[CHECKING] a v0.1.0 ([..])
[CHECKING] foo v0.1.0 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cargo_test]
fn fix_to_broken_code() {
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
            r#"
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
                            panic!()
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
            "#,
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
        .file("bar/src/lib.rs", "pub fn foo() { let mut x = 3; drop(x); }")
        .build();

    // Build our rustc shim
    p.cargo("build").cwd("foo").run();

    // Attempt to fix code, but our shim will always fail the second compile
    p.cargo("fix --allow-no-vcs --broken-code")
        .cwd("bar")
        .env("RUSTC", p.root().join("foo/target/debug/foo"))
        .with_status(101)
        .with_stderr_contains("[WARNING] failed to automatically apply fixes [..]")
        .run();

    assert_eq!(
        p.read_file("bar/src/lib.rs"),
        "pub fn foo() { let x = 3; drop(x); }"
    );
}

#[cargo_test]
fn fix_with_common() {
    let p = project()
        .file("src/lib.rs", "")
        .file(
            "tests/t1.rs",
            "mod common; #[test] fn t1() { common::try(); }",
        )
        .file(
            "tests/t2.rs",
            "mod common; #[test] fn t2() { common::try(); }",
        )
        .file("tests/common/mod.rs", "pub fn try() {}")
        .build();

    p.cargo("fix --edition --allow-no-vcs").run();

    assert_eq!(p.read_file("tests/common/mod.rs"), "pub fn r#try() {}");
}

#[cargo_test]
fn fix_in_existing_repo_weird_ignore() {
    // Check that ignore doesn't ignore the repo itself.
    let p = git::new("foo", |project| {
        project
            .file("src/lib.rs", "")
            .file(".gitignore", "foo\ninner\n")
            .file("inner/file", "")
    });

    p.cargo("fix").run();
    // This is questionable about whether it is the right behavior. It should
    // probably be checking if any source file for the current project is
    // ignored.
    p.cargo("fix")
        .cwd("inner")
        .with_stderr_contains("[ERROR] no VCS found[..]")
        .with_status(101)
        .run();
    p.cargo("fix").cwd("src").run();
}

#[cargo_test]
fn fix_color_message() {
    // Check that color appears in diagnostics.
    let p = project()
        .file("src/lib.rs", "std::compile_error!{\"color test\"}")
        .build();

    p.cargo("fix --allow-no-vcs --color=always")
        .with_stderr_contains("[..]\x1b[[..]")
        .with_status(101)
        .run();

    p.cargo("fix --allow-no-vcs --color=never")
        .with_stderr_contains("error: color test")
        .with_stderr_does_not_contain("[..]\x1b[[..]")
        .with_status(101)
        .run();
}
