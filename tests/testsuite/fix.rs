//! Tests for the `cargo fix` command.

use cargo::core::Edition;
use cargo_test_support::git;
use cargo_test_support::paths::CargoPathExt;
use cargo_test_support::registry::{Dependency, Package};
use cargo_test_support::tools;
use cargo_test_support::{basic_manifest, is_nightly, project};

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
        .with_stderr_contains("[ERROR] could not compile `foo` due to previous error")
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
        .with_status(101)
        .with_stderr(&format!("\
[CHECKING] foo [..]
[ERROR] cannot migrate src/lib.rs to edition {next}
Edition {next} is unstable and not allowed in this release, consider trying the nightly release channel.
error: could not compile `foo`
", next=next))
        .run();

    if !is_nightly() {
        // The rest of this test is fundamentally always nightly.
        return;
    }

    p.cargo("fix --edition --allow-no-vcs")
        .masquerade_as_nightly_cargo()
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

#[cargo_test]
fn prepare_for_already_on_latest_unstable() {
    // During the period where a new edition is coming up, but not yet stable,
    // this test will check what happens if you are already on the latest. If
    // there is no next edition, it does nothing.
    if !is_nightly() {
        // This test is fundamentally always nightly.
        return;
    }
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
        .masquerade_as_nightly_cargo()
        .with_stderr_contains(&format!(
            "\
[CHECKING] foo [..]
[WARNING] `src/lib.rs` is already on the latest edition ({next_edition}), unable to migrate further
[FINISHED] [..]
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
        .with_stderr_contains(&format!(
            "\
[CHECKING] foo [..]
[WARNING] `src/lib.rs` is already on the latest edition ({latest_stable}), unable to migrate further
[FINISHED] [..]
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

#[cargo_test]
fn edition_v2_resolver_report() {
    // Show a report if the V2 resolver shows differences.
    if !is_nightly() {
        // 2021 is unstable
        return;
    }
    Package::new("common", "1.0.0")
        .feature("f1", &[])
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
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("fix --edition --allow-no-vcs")
        .masquerade_as_nightly_cargo()
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

  common v1.0.0: f1, opt_dep
  common v1.0.0 (as host dependency): f1

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
#[ignore] // Broken, see https://github.com/rust-lang/rust/pull/86009
fn fix_edition_2021() {
    // Can migrate 2021, even when lints are allowed.
    if !is_nightly() {
        // 2021 is unstable
        return;
    }
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
        .masquerade_as_nightly_cargo()
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
