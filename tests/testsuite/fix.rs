//! Tests for the `cargo fix` command.

use cargo::core::Edition;
use cargo_test_support::compare::assert_match_exact;
use cargo_test_support::git::{self, init};
use cargo_test_support::paths::{self, CargoPathExt};
use cargo_test_support::registry::{Dependency, Package};
use cargo_test_support::{basic_manifest, is_nightly, project, Project};
use cargo_test_support::{tools, wrapped_clippy_driver};

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
        .with_stderr_contains("[ERROR] could not compile `foo` (lib) due to previous error")
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

fn rustc_shim_for_cargo_fix() -> Project {
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
                    // Also compatible for rustc invocations with `@path` argfile.
                    let is_lib_rs = env::args_os()
                        .map(PathBuf::from)
                        .flat_map(|p| if let Some(p) = p.to_str().unwrap_or_default().strip_prefix("@") {
                            fs::read_to_string(p).unwrap().lines().map(PathBuf::from).collect()
                        } else {
                            vec![p]
                        })
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

    p
}

#[cargo_test]
fn broken_fixes_backed_out() {
    let p = rustc_shim_for_cargo_fix();
    // Attempt to fix code, but our shim will always fail the second compile.
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
             https://github.com/rust-lang/rust/issues\n\
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
        .with_stderr_does_not_contain("[..][FIXED][..]")
        .run();

    // Make sure the fix which should have been applied was backed out
    assert!(p.read_file("bar/src/lib.rs").contains("let mut x = 3;"));
}

#[cargo_test]
fn broken_clippy_fixes_backed_out() {
    let p = rustc_shim_for_cargo_fix();
    // Attempt to fix code, but our shim will always fail the second compile.
    // Also, we use `clippy` as a workspace wrapper to make sure that we properly
    // generate the report bug text.
    p.cargo("fix --allow-no-vcs --lib")
        .cwd("bar")
        .env("__CARGO_FIX_YOLO", "1")
        .env("RUSTC", p.root().join("foo/target/debug/foo"))
        //  We can't use `clippy` so we use a `rustc` workspace wrapper instead
        .env("RUSTC_WORKSPACE_WRAPPER", wrapped_clippy_driver())
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
             https://github.com/rust-lang/rust-clippy/issues\n\
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
        .with_stderr_does_not_contain("[..][FIXED][..]")
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
[FIXED] bar/src/lib.rs (1 fix)
[CHECKING] foo v0.1.0 ([..])
[FIXED] src/lib.rs (1 fix)
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
[MIGRATING] src/lib.rs from 2015 edition to 2018
[FIXED] src/lib.rs (2 fixes)
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

    p.cargo("fix --edition --allow-no-vcs")
        .with_stderr(
            "\
[CHECKING] foo v0.0.1 ([..])
[MIGRATING] src/lib.rs from 2015 edition to 2018
[FIXED] src/lib.rs (1 fix)
[FINISHED] [..]
",
        )
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
[FIXED] src/lib.rs (1 fix)
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

    p.cargo("fix --edition --allow-no-vcs")
        .env("RUSTFLAGS", "-C linker=cc")
        .with_stderr(
            "\
[CHECKING] foo v0.0.1 ([..])
[MIGRATING] src/lib.rs from 2015 edition to 2018
[FIXED] src/lib.rs (1 fix)
[FINISHED] [..]
",
        )
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
[FIXED] src/lib.rs (1 fix)
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
[FIXED] src/lib.rs (2 fixes)
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
[FIXED] src/lib.rs (2 fixes)
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
                #![deny(unused_mut)]

                pub fn foo() -> u32 {
                    let mut x = 3;
                    x
                }

                pub fn bar() {
                    #[allow(unused_mut)]
                    let mut _y = 4;
                }
            ",
        )
        .build();

    p.cargo("fix --allow-no-vcs")
        .env("__CARGO_FIX_YOLO", "1")
        .run();
    assert!(!p.read_file("src/lib.rs").contains("let mut x = 3;"));
    assert!(p.read_file("src/lib.rs").contains("let mut _y = 4;"));
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
        .with_stderr_contains("[FIXED] src/bar.rs (1 fix)")
        .with_stderr_contains("[FIXED] src/lib.rs (1 fix)")
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
        .with_stderr_contains("[FIXED] build.rs (1 fix)")
        // Don't assert number of fixes for this one, as we don't know if we're
        // fixing it once or twice! We run this all concurrently, and if we
        // compile (and fix) in `--test` mode first, we get two fixes. Otherwise
        // we'll fix one non-test thing, and then fix another one later in
        // test mode.
        .with_stderr_contains("[FIXED] src/lib.rs[..]")
        .with_stderr_contains("[FIXED] src/main.rs (1 fix)")
        .with_stderr_contains("[FIXED] examples/foo.rs (1 fix)")
        .with_stderr_contains("[FIXED] tests/a.rs (1 fix)")
        .with_stderr_contains("[FINISHED] [..]")
        .run();
    p.cargo("check").run();
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
    p.cargo("check").run();
    p.cargo("fix --features bar --allow-no-vcs").run();
    p.cargo("check --features bar").run();
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
fn errors_about_untracked_files() {
    let mut git_project = project().at("foo");
    git_project = git_project.file("src/lib.rs", "pub fn foo() {}");
    let p = git_project.build();
    let _ = init(&p.root());

    p.cargo("fix")
        .with_status(101)
        .with_stderr(
            "\
error: the working directory of this package has uncommitted changes, \
and `cargo fix` can potentially perform destructive changes; if you'd \
like to suppress this error pass `--allow-dirty`, `--allow-staged`, or \
commit the changes to these files:

  * Cargo.toml (dirty)
  * src/ (dirty)


",
        )
        .run();
    p.cargo("fix --allow-dirty").run();
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
fn prepare_for_unstable() {
    // During the period where a new edition is coming up, but not yet stable,
    // this test will verify that it cannot be migrated to on stable. If there
    // is no next edition, it does nothing.
    let next = match Edition::LATEST_UNSTABLE {
        Some(next) => next,
        None => {
            eprintln!("Next edition is currently not available, skipping test.");
            return;
        }
    };
    let latest_stable = Edition::LATEST_STABLE;
    let prev = latest_stable.previous().unwrap();
    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "{}"
            "#,
                latest_stable
            ),
        )
        .file("src/lib.rs", "")
        .build();

    // -j1 to make the error more deterministic (otherwise there can be
    // multiple errors since they run in parallel).
    p.cargo("fix --edition --allow-no-vcs -j1")
        .with_stderr(&format_args!("\
[CHECKING] foo [..]
[WARNING] `src/lib.rs` is on the latest edition, but trying to migrate to edition {next}.
Edition {next} is unstable and not allowed in this release, consider trying the nightly release channel.

If you are trying to migrate from the previous edition ({prev}), the
process requires following these steps:

1. Start with `edition = \"{prev}\"` in `Cargo.toml`
2. Run `cargo fix --edition`
3. Modify `Cargo.toml` to set `edition = \"{latest_stable}\"`
4. Run `cargo build` or `cargo test` to verify the fixes worked

More details may be found at
https://doc.rust-lang.org/edition-guide/editions/transitioning-an-existing-project-to-a-new-edition.html

[FINISHED] [..]
", next=next, latest_stable=latest_stable, prev=prev))
        .run();

    if !is_nightly() {
        // The rest of this test is fundamentally always nightly.
        return;
    }

    p.cargo("fix --edition --allow-no-vcs")
        .masquerade_as_nightly_cargo(&["always_nightly"])
        .with_stderr(&format!(
            "\
[CHECKING] foo [..]
[MIGRATING] src/lib.rs from {latest_stable} edition to {next}
[FINISHED] [..]
",
            latest_stable = latest_stable,
            next = next,
        ))
        .run();
}

#[cargo_test]
fn prepare_for_latest_stable() {
    // This is the stable counterpart of prepare_for_unstable.
    let latest_stable = Edition::LATEST_STABLE;
    let previous = latest_stable.previous().unwrap();
    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                [package]
                name = 'foo'
                version = '0.1.0'
                edition = '{}'
            "#,
                previous
            ),
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("fix --edition --allow-no-vcs")
        .with_stderr(&format!(
            "\
[CHECKING] foo [..]
[MIGRATING] src/lib.rs from {} edition to {}
[FINISHED] [..]
",
            previous, latest_stable
        ))
        .run();
}

#[cargo_test(nightly, reason = "fundamentally always nightly")]
fn prepare_for_already_on_latest_unstable() {
    // During the period where a new edition is coming up, but not yet stable,
    // this test will check what happens if you are already on the latest. If
    // there is no next edition, it does nothing.
    let next_edition = match Edition::LATEST_UNSTABLE {
        Some(next) => next,
        None => {
            eprintln!("Next edition is currently not available, skipping test.");
            return;
        }
    };
    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                cargo-features = ["edition{}"]

                [package]
                name = 'foo'
                version = '0.1.0'
                edition = '{}'
            "#,
                next_edition, next_edition
            ),
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("fix --edition --allow-no-vcs")
        .masquerade_as_nightly_cargo(&["always_nightly"])
        .with_stderr_contains("[CHECKING] foo [..]")
        .with_stderr_contains(&format!(
            "\
[WARNING] `src/lib.rs` is already on the latest edition ({next_edition}), unable to migrate further
",
            next_edition = next_edition
        ))
        .run();
}

#[cargo_test]
fn prepare_for_already_on_latest_stable() {
    // Stable counterpart of prepare_for_already_on_latest_unstable.
    if Edition::LATEST_UNSTABLE.is_some() {
        eprintln!("This test cannot run while the latest edition is unstable, skipping.");
        return;
    }
    let latest_stable = Edition::LATEST_STABLE;
    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                [package]
                name = 'foo'
                version = '0.1.0'
                edition = '{}'
            "#,
                latest_stable
            ),
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("fix --edition --allow-no-vcs")
        .with_stderr_contains("[CHECKING] foo [..]")
        .with_stderr_contains(&format!(
            "\
[WARNING] `src/lib.rs` is already on the latest edition ({latest_stable}), unable to migrate further
",
            latest_stable = latest_stable
        ))
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

    p.cargo("fix --allow-no-vcs --edition --lib")
        .with_stderr(
            "\
[CHECKING] foo [..]
[MIGRATING] src/lib.rs from 2015 edition to 2018
[FIXED] src/lib.rs (2 fixes)
[FINISHED] dev [..]
",
        )
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
[FIXED] src/lib.rs (1 fix)
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
        .env("RUSTC_WRAPPER", tools::echo_wrapper())
        .run();
    p.build_dir().rm_rf();
    p.cargo("fix --allow-no-vcs --verbose")
        .env("RUSTC_WORKSPACE_WRAPPER", tools::echo_wrapper())
        .run();
}

#[cargo_test]
fn uses_workspace_wrapper_and_primary_wrapper_override() {
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
        .env("RUSTC_WORKSPACE_WRAPPER", tools::echo_wrapper())
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
[MIGRATING] src/lib.rs from 2015 edition to 2018
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
                    // Ignore calls to things like --print=file-names and compiling build.rs.
                    // Also compatible for rustc invocations with `@path` argfile.
                    let is_lib_rs = env::args_os()
                        .map(PathBuf::from)
                        .flat_map(|p| if let Some(p) = p.to_str().unwrap_or_default().strip_prefix("@") {
                            fs::read_to_string(p).unwrap().lines().map(PathBuf::from).collect()
                        } else {
                            vec![p]
                        })
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
            .file(".gitignore", "foo\ninner\nCargo.lock\ntarget\n")
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

#[cargo_test]
fn edition_v2_resolver_report() {
    // Show a report if the V2 resolver shows differences.
    Package::new("common", "1.0.0")
        .feature("f1", &[])
        .feature("dev-feat", &[])
        .add_dep(Dependency::new("opt_dep", "1.0").optional(true))
        .publish();
    Package::new("opt_dep", "1.0.0").publish();

    Package::new("bar", "1.0.0")
        .add_dep(
            Dependency::new("common", "1.0")
                .target("cfg(whatever)")
                .enable_features(&["f1"]),
        )
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2018"

                [dependencies]
                common = "1.0"
                bar = "1.0"

                [build-dependencies]
                common = { version = "1.0", features = ["opt_dep"] }

                [dev-dependencies]
                common = { version="1.0", features=["dev-feat"] }
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("fix --edition --allow-no-vcs")
        .with_stderr_unordered("\
[UPDATING] [..]
[DOWNLOADING] crates ...
[DOWNLOADED] common v1.0.0 [..]
[DOWNLOADED] bar v1.0.0 [..]
[DOWNLOADED] opt_dep v1.0.0 [..]
note: Switching to Edition 2021 will enable the use of the version 2 feature resolver in Cargo.
This may cause some dependencies to be built with fewer features enabled than previously.
More information about the resolver changes may be found at https://doc.rust-lang.org/nightly/edition-guide/rust-2021/default-cargo-resolver.html
When building the following dependencies, the given features will no longer be used:

  common v1.0.0 removed features: dev-feat, f1, opt_dep
  common v1.0.0 (as host dependency) removed features: dev-feat, f1

The following differences only apply when building with dev-dependencies:

  common v1.0.0 removed features: f1, opt_dep

[CHECKING] opt_dep v1.0.0
[CHECKING] common v1.0.0
[CHECKING] bar v1.0.0
[CHECKING] foo v0.1.0 [..]
[MIGRATING] src/lib.rs from 2018 edition to 2021
[FINISHED] [..]
")
        .run();
}

#[cargo_test]
fn rustfix_handles_multi_spans() {
    // Checks that rustfix handles a single diagnostic with multiple
    // suggestion spans (non_fmt_panic in this case).
    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "0.1.0"))
        .file(
            "src/lib.rs",
            r#"
                pub fn foo() {
                    panic!(format!("hey"));
                }
            "#,
        )
        .build();

    p.cargo("fix --allow-no-vcs").run();
    assert!(p.read_file("src/lib.rs").contains(r#"panic!("hey");"#));
}

#[cargo_test]
fn fix_edition_2021() {
    // Can migrate 2021, even when lints are allowed.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2018"
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
                #![allow(ellipsis_inclusive_range_patterns)]

                pub fn f() -> bool {
                    let x = 123;
                    match x {
                        0...100 => true,
                        _ => false,
                    }
                }
            "#,
        )
        .build();
    p.cargo("fix --edition --allow-no-vcs")
        .with_stderr(
            "\
[CHECKING] foo v0.1.0 [..]
[MIGRATING] src/lib.rs from 2018 edition to 2021
[FIXED] src/lib.rs (1 fix)
[FINISHED] [..]
",
        )
        .run();
    assert!(p.read_file("src/lib.rs").contains(r#"0..=100 => true,"#));
}

#[cargo_test]
fn fix_shared_cross_workspace() {
    // Fixing a file that is shared between multiple packages in the same workspace.
    // Make sure two processes don't try to fix the same file at the same time.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["foo", "bar"]
            "#,
        )
        .file("foo/Cargo.toml", &basic_manifest("foo", "0.1.0"))
        .file("foo/src/lib.rs", "pub mod shared;")
        // This will fix both unused and bare trait.
        .file("foo/src/shared.rs", "pub fn fixme(x: Box<&Fn() -> ()>) {}")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file(
            "bar/src/lib.rs",
            r#"
                #[path="../../foo/src/shared.rs"]
                pub mod shared;
            "#,
        )
        .build();

    // The output here can be either of these two, depending on who runs first:
    //     [FIXED] bar/src/../../foo/src/shared.rs (2 fixes)
    //     [FIXED] foo/src/shared.rs (2 fixes)
    p.cargo("fix --allow-no-vcs")
        .with_stderr_unordered(
            "\
[CHECKING] foo v0.1.0 [..]
[CHECKING] bar v0.1.0 [..]
[FIXED] [..]foo/src/shared.rs (2 fixes)
[FINISHED] [..]
",
        )
        .run();

    assert_match_exact(
        "pub fn fixme(_x: Box<&dyn Fn() -> ()>) {}",
        &p.read_file("foo/src/shared.rs"),
    );
}

#[cargo_test]
fn abnormal_exit() {
    // rustc fails unexpectedly after applying fixes, should show some error information.
    //
    // This works with a proc-macro that runs three times:
    // - First run (collect diagnostics pass): writes a file, exits normally.
    // - Second run (verify diagnostics work): it detects the presence of the
    //   file, removes the file, and aborts the process.
    // - Third run (collecting messages to display): file not found, exits normally.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [dependencies]
                pm = {path="pm"}
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
                pub fn f() {
                    let mut x = 1;
                    pm::crashme!();
                }
            "#,
        )
        .file(
            "pm/Cargo.toml",
            r#"
                [package]
                name = "pm"
                version = "0.1.0"
                edition = "2018"

                [lib]
                proc-macro = true
            "#,
        )
        .file(
            "pm/src/lib.rs",
            r#"
                use proc_macro::TokenStream;
                #[proc_macro]
                pub fn crashme(_input: TokenStream) -> TokenStream {
                    // Use a file to succeed on the first pass, and fail on the second.
                    let p = std::env::var_os("ONCE_PATH").unwrap();
                    let check_path = std::path::Path::new(&p);
                    if check_path.exists() {
                        eprintln!("I'm not a diagnostic.");
                        std::fs::remove_file(check_path).unwrap();
                        std::process::abort();
                    } else {
                        std::fs::write(check_path, "").unwrap();
                        "".parse().unwrap()
                    }
                }
            "#,
        )
        .build();

    p.cargo("fix --lib --allow-no-vcs")
        .env(
            "ONCE_PATH",
            paths::root().join("proc-macro-run-once").to_str().unwrap(),
        )
        .with_stderr_contains(
            "[WARNING] failed to automatically apply fixes suggested by rustc to crate `foo`",
        )
        .with_stderr_contains("I'm not a diagnostic.")
        // "signal: 6, SIGABRT: process abort signal" on some platforms
        .with_stderr_contains("rustc exited abnormally: [..]")
        .with_stderr_contains("Original diagnostics will follow.")
        .run();
}

#[cargo_test]
fn fix_with_run_cargo_in_proc_macros() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2018"

                [lib]
                proc-macro = true
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
                use proc_macro::*;

                #[proc_macro]
                pub fn foo(_input: TokenStream) -> TokenStream {
                    let output = std::process::Command::new(env!("CARGO"))
                        .args(&["metadata", "--format-version=1"])
                        .output()
                        .unwrap();
                    eprintln!("{}", std::str::from_utf8(&output.stderr).unwrap());
                    println!("{}", std::str::from_utf8(&output.stdout).unwrap());
                    "".parse().unwrap()
                }
            "#,
        )
        .file(
            "src/bin/main.rs",
            r#"
                use foo::foo;

                fn main() {
                    foo!("bar")
                }
            "#,
        )
        .build();
    p.cargo("fix --allow-no-vcs")
        .with_stderr_does_not_contain("error: could not find .rs file in rustc args")
        .run();
}

#[cargo_test]
fn non_edition_lint_migration() {
    // Migrating to a new edition where a non-edition lint causes problems.
    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "0.1.0"))
        .file(
            "src/lib.rs",
            r#"
                // This is only used in a test.
                // To be correct, this should be gated on #[cfg(test)], but
                // sometimes people don't do that. If the unused_imports
                // lint removes this, then the unittest will fail to compile.
                use std::str::from_utf8;

                pub mod foo {
                    pub const FOO: &[u8] = &[102, 111, 111];
                }

                #[test]
                fn example() {
                    assert_eq!(
                        from_utf8(::foo::FOO), Ok("foo")
                    );
                }
            "#,
        )
        .build();
    // Check that it complains about an unused import.
    p.cargo("check --lib")
        .with_stderr_contains("[..]unused_imports[..]")
        .with_stderr_contains("[..]std::str::from_utf8[..]")
        .run();
    p.cargo("fix --edition --allow-no-vcs").run();
    let contents = p.read_file("src/lib.rs");
    // Check it does not remove the "unused" import.
    assert!(contents.contains("use std::str::from_utf8;"));
    // Check that it made the edition migration.
    assert!(contents.contains("from_utf8(crate::foo::FOO)"));
}

// For rust-lang/cargo#9857
#[cargo_test]
fn fix_in_dependency() {
    Package::new("bar", "1.0.0")
        .file(
            "src/lib.rs",
            r#"
                #[macro_export]
                macro_rules! m {
                    ($i:tt) => {
                        let $i = 1;
                    };
                }
            "#,
        )
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [dependencies]
                bar = "1.0"
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
                pub fn foo() {
                    bar::m!(abc);
                }
            "#,
        )
        .build();

    p.cargo("fix --allow-no-vcs")
        .with_stderr_does_not_contain("[FIXED] [..]")
        .run();
}
