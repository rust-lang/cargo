//! Tests for the `cargo fix` command.

use crate::prelude::*;
use crate::utils::tools;
use cargo::core::Edition;
use cargo_test_support::compare::assert_e2e;
use cargo_test_support::git::{self, init};
use cargo_test_support::paths;
use cargo_test_support::registry::{Dependency, Package};
use cargo_test_support::str;
use cargo_test_support::{basic_manifest, is_nightly, project};

#[cargo_test]
fn do_not_fix_broken_builds() {
    let p = project()
        .file(
            "src/lib.rs",
            r#"
                pub fn foo() {
                    let mut x = 3;
                    let _ = x;
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
        .with_stderr_data(str![[r#"
...
[ERROR] could not compile `foo` (lib) due to 1 previous error; 1 warning emitted
...
"#]])
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
fn fix_path_deps() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

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
        .with_stdout_data("")
        .with_stderr_data(
            str![[r#"
[CHECKING] bar v0.1.0 ([ROOT]/foo/bar)
[FIXED] bar/src/lib.rs (1 fix)
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[FIXED] src/lib.rs (1 fix)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]
            .unordered(),
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
                edition = "2015"

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

    p.cargo("fix --edition --allow-no-vcs")
        .with_stderr_data(str![[r#"
[MIGRATING] Cargo.toml from 2015 edition to 2018
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[MIGRATING] src/lib.rs from 2015 edition to 2018
[FIXED] src/lib.rs (2 fixes)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .with_stdout_data("")
        .run();

    println!("{}", p.read_file("src/lib.rs"));
    assert!(p.read_file("src/lib.rs").contains("use crate::foo::FOO;"));
    assert!(
        p.read_file("src/lib.rs")
            .contains("let x = crate::foo::FOO;")
    );
}

#[cargo_test]
fn fix_tests_with_edition() {
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
                pub fn foo() {}

                #[cfg(test)]
                mod tests {
                    #[test]
                    fn it_works() {
                        f();
                    }
                    fn f() -> bool {
                        let x = 123;
                        match x {
                            0...100 => true,
                            _ => false,
                        }
                    }
                }
            "#,
        )
        .build();

    p.cargo("fix --edition --allow-no-vcs")
        .with_stderr_data(str![[r#"
[MIGRATING] Cargo.toml from 2018 edition to 2021
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[MIGRATING] src/lib.rs from 2018 edition to 2021
[FIXED] src/lib.rs (1 fix)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .with_stdout_data("")
        .run();
    // Check that the test is fixed.
    assert!(p.read_file("src/lib.rs").contains(r#"0..=100 => true,"#));
}

#[cargo_test]
fn fix_tests_with_edition_idioms() {
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
                pub fn foo() {}

                #[cfg(test)]
                mod tests {
                    #[test]
                    fn it_works() {
                        f();
                    }

                    use std::any::Any;
                    pub fn f() {
                        let _x: Box<Any> = Box::new(3);
                    }
                }
            "#,
        )
        .build();

    p.cargo("fix --edition-idioms --allow-no-vcs")
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[FIXED] src/lib.rs (1 fix)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .with_stdout_data("")
        .run();
    // Check that the test is fixed.
    assert!(p.read_file("src/lib.rs").contains("Box<dyn Any>"));
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
        .with_stderr_data(str![[r#"
[MIGRATING] Cargo.toml from 2015 edition to 2018
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[MIGRATING] src/lib.rs from 2015 edition to 2018
[FIXED] src/lib.rs (1 fix)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .with_stdout_data("")
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

    p.cargo("fix --allow-no-vcs")
        .env("__CARGO_FIX_YOLO", "1")
        .with_stderr_data(str![[r#"
[CHECKING] bar v0.1.0 ([ROOT]/foo/bar)
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[FIXED] src/lib.rs (1 fix)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .with_stdout_data("")
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
        .with_stderr_data(str![[r#"
[MIGRATING] Cargo.toml from 2015 edition to 2018
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[MIGRATING] src/lib.rs from 2015 edition to 2018
[FIXED] src/lib.rs (1 fix)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .with_stdout_data("")
        .run();
}

#[cargo_test]
fn no_changes_necessary() {
    let p = project().file("src/lib.rs", "").build();

    p.cargo("fix --allow-no-vcs")
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .with_stdout_data("")
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

    p.cargo("fix --allow-no-vcs")
        .env("__CARGO_FIX_YOLO", "1")
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FIXED] src/lib.rs (1 fix)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .with_stdout_data("")
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

    p.cargo("fix --allow-no-vcs")
        .env("__CARGO_FIX_YOLO", "1")
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FIXED] src/lib.rs (2 fixes)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .with_stdout_data("")
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

    p.cargo("fix --allow-no-vcs")
        .env("__CARGO_FIX_YOLO", "1")
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FIXED] src/lib.rs (2 fixes)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .with_stdout_data("")
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
             pub fn foo() { let mut x = 3; let _ = x; }
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
        .with_stderr_data(
            str![[r#"
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FIXED] src/bar.rs (1 fix)
[FIXED] src/lib.rs (1 fix)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]
            .unordered(),
        )
        .run();
    assert!(!p.read_file("src/lib.rs").contains("let mut x = 3;"));
    assert!(!p.read_file("src/bar.rs").contains("let mut x = 3;"));
}

#[cargo_test]
fn fixes_missing_ampersand() {
    let p = project()
        .file("src/main.rs", "fn main() { let mut x = 3; let _ = x; }")
        .file(
            "src/lib.rs",
            r#"
                pub fn foo() { let mut x = 3; let _ = x; }

                #[test]
                pub fn foo2() { let mut x = 3; let _ = x; }
            "#,
        )
        .file(
            "tests/a.rs",
            r#"
                #[test]
                pub fn foo() { let mut x = 3; let _ = x; }
            "#,
        )
        .file("examples/foo.rs", "fn main() { let mut x = 3; let _ = x; }")
        .file("build.rs", "fn main() { let mut x = 3; let _ = x; }")
        .build();

    p.cargo("fix --all-targets --allow-no-vcs")
        .env("__CARGO_FIX_YOLO", "1")
        .with_stdout_data("")
        // Don't assert number of fixes for `src/lib.rs`, as we don't know if we're
        // fixing it once or twice! We run this all concurrently, and if we
        // compile (and fix) in `--test` mode first, we get two fixes. Otherwise
        // we'll fix one non-test thing, and then fix another one later in
        // test mode.
        .with_stderr_data(
            str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FIXED] build.rs (1 fix)
[FIXED] src/lib.rs ([..]fix[..])
[FIXED] src/main.rs (1 fix)
[FIXED] examples/foo.rs (1 fix)
[FIXED] tests/a.rs (1 fix)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
...
"#]]
            .unordered(),
        )
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
                edition = "2015"

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
        .with_stderr_data(str![[r#"
...
[WARNING] use of deprecated function `bar`
...
"#]])
        .run();
}

#[cargo_test]
fn warns_if_no_vcs_detected() {
    let p = project().file("src/lib.rs", "pub fn foo() {}").build();

    p.cargo("fix")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] no VCS found for this package and `cargo fix` can potentially perform destructive changes; if you'd like to suppress this error pass `--allow-no-vcs`

"#]])
        .run();
    p.cargo("fix --allow-no-vcs").run();
}

#[cargo_test]
fn warns_about_dirty_working_directory() {
    let p = git::new("foo", |p| p.file("src/lib.rs", "pub fn foo() {}"));

    p.change_file("src/lib.rs", "");

    p.cargo("fix")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] the working directory of this package has uncommitted changes, and `cargo fix` can potentially perform destructive changes; if you'd like to suppress this error pass `--allow-dirty`, or commit the changes to these files:

  * src/lib.rs (dirty)



"#]])
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
        .with_stderr_data(str![[r#"
[ERROR] the working directory of this package has uncommitted changes, and `cargo fix` can potentially perform destructive changes; if you'd like to suppress this error pass `--allow-dirty`, or commit the changes to these files:

  * src/lib.rs (staged)



"#]])
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
        .with_stderr_data(str![[r#"
[ERROR] the working directory of this package has uncommitted changes, and `cargo fix` can potentially perform destructive changes; if you'd like to suppress this error pass `--allow-dirty`, or commit the changes to these files:

  * Cargo.toml (dirty)
  * src/ (dirty)



"#]])
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
fn do_not_fix_tests_by_default() {
    let p = project()
        .file("src/lib.rs", "pub fn foo() { let mut x = 3; let _ = x; }")
        .file("tests/foo.rs", "pub fn foo() { let mut x = 3; let _ = x; }")
        .build();
    p.cargo("fix --allow-no-vcs")
        .env("__CARGO_FIX_YOLO", "1")
        .run();
    assert!(!p.read_file("src/lib.rs").contains("let mut x"));
    assert!(p.read_file("tests/foo.rs").contains("let mut x"));
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
        .with_stderr_data(&format!("\
[CHECKING] foo v0.1.0 ([ROOT]/foo)
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

[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
", next=next, latest_stable=latest_stable, prev=prev))
        .run();

    if !is_nightly() {
        // The rest of this test is fundamentally always nightly.
        return;
    }

    p.cargo("fix --edition --allow-no-vcs")
        .masquerade_as_nightly_cargo(&["always_nightly"])
        .with_stderr_data(&format!(
            "\
[MIGRATING] Cargo.toml from {latest_stable} edition to {next}
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[MIGRATING] src/lib.rs from {latest_stable} edition to {next}
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
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
        .with_stderr_data(&format!(
            "\
[MIGRATING] Cargo.toml from {previous} edition to {latest_stable}
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[MIGRATING] src/lib.rs from {previous} edition to {latest_stable}
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
",
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
        .with_stderr_data(&format!(
            "\
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[WARNING] `src/lib.rs` is already on the latest edition ({next_edition}), unable to migrate further
...
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
        .with_stderr_data(str![[r#"
[MIGRATING] Cargo.toml from 2015 edition to 2018
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[MIGRATING] src/lib.rs from 2015 edition to 2018
[FIXED] src/lib.rs (2 fixes)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
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

    p.cargo("fix --edition-idioms --allow-no-vcs")
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[FIXED] src/lib.rs (1 fix)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
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
        .with_stderr_data(str![[r#"
...
[WARNING] use of deprecated function `bar`
...
"#]])
        .run();

    p.cargo("fix --allow-no-vcs")
        .with_stderr_data(str![[r#"
...
[WARNING] use of deprecated function `bar`
...
"#]])
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
        .with_stderr_data(
            str![[r#"
...
 --> src/lib.rs:6:29
...
 --> src/main.rs:6:29
...
 --> examples/fooxample.rs:6:29
...
 --> tests/foo.rs:7:29
...
 --> tests/bar.rs:7:29
...

"#]]
            .unordered(),
        )
        .run();

    p.cargo("fix --allow-no-vcs --all-targets")
        .with_stderr_data(
            str![[r#"
...
 --> src/lib.rs:6:29
...
 --> src/main.rs:6:29
...
 --> examples/fooxample.rs:6:29
...
 --> tests/bar.rs:7:29
...
 --> tests/foo.rs:7:29
...
"#]]
            .unordered(),
        )
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
                edition = "2015"

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
        .with_stdout_data("")
        .with_stderr_data(str![[r#"
[CHECKING] bar v0.1.0 ([ROOT]/foo/bar)
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("fix --allow-no-vcs -p foo")
        .env("__CARGO_FIX_YOLO", "1")
        .with_stdout_data("")
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
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
                edition = "2015"
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
                edition = "2015"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("fix --allow-no-vcs --verbose")
        .env("RUSTC_WORKSPACE_WRAPPER", tools::echo_wrapper())
        .with_stderr_data(str![[r#"
...
WRAPPER CALLED: rustc src/lib.rs --crate-name foo [..]
...
"#]])
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
                edition = "2015"

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
                edition = "2015"
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
        .with_stderr_data(str![[r#"
[MIGRATING] Cargo.toml from 2015 edition to 2018
[LOCKING] 1 package to latest compatible version
[CHECKING] a v0.1.0 ([ROOT]/foo/a)
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[MIGRATING] src/lib.rs from 2015 edition to 2018
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
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
                edition = "2015"
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
                edition = "2015"
                [workspace]
            "#,
        )
        .file("bar/build.rs", "fn main() {}")
        .file("bar/src/lib.rs", "pub fn foo() { let mut x = 3; let _ = x; }")
        .build();

    // Build our rustc shim
    p.cargo("build").cwd("foo").run();

    // Attempt to fix code, but our shim will always fail the second compile
    p.cargo("fix --allow-no-vcs --broken-code")
        .cwd("bar")
        .env("RUSTC", p.root().join("foo/target/debug/foo"))
        .with_stderr_data(str![[r#"
...
[WARNING] failed to automatically apply fixes suggested by rustc to crate `bar`
...
"#]])
        .run();

    assert_e2e().eq(
        p.read_file("bar/src/lib.rs"),
        str!["pub fn foo() { let x = 3; let _ = x; }"],
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

    assert_e2e().eq(
        p.read_file("tests/common/mod.rs"),
        str!["pub fn r#try() {}"],
    );
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
        .with_stderr_data(str![[r#"
[ERROR] no VCS found for this package and `cargo fix` can potentially perform destructive changes; if you'd like to suppress this error pass `--allow-no-vcs`

"#]])
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
        .with_stderr_data(
            "\
...
[..]\x1b[[..]
...
",
        )
        .with_status(101)
        .run();

    p.cargo("fix --allow-no-vcs --color=never")
        .with_stderr_data(str![[r#"
...
[ERROR] color test
...
"#]])
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
        .with_stderr_data(str![[r#"
[MIGRATING] Cargo.toml from 2018 edition to 2021
[UPDATING] `dummy-registry` index
[LOCKING] 3 packages to latest compatible versions
[DOWNLOADING] crates ...
[DOWNLOADED] common v1.0.0 (registry `dummy-registry`)
[DOWNLOADED] bar v1.0.0 (registry `dummy-registry`)
[DOWNLOADED] opt_dep v1.0.0 (registry `dummy-registry`)
[NOTE] Switching to Edition 2021 will enable the use of the version 2 feature resolver in Cargo.
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
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[MIGRATING] src/lib.rs from 2018 edition to 2021
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]].unordered())
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
        .with_stderr_data(str![[r#"
[MIGRATING] Cargo.toml from 2018 edition to 2021
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[MIGRATING] src/lib.rs from 2018 edition to 2021
[FIXED] src/lib.rs (1 fix)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
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
        .env("__CARGO_FIX_YOLO", "1")
        .with_stderr_data(
            str![[r#"
[CHECKING] foo v0.1.0 ([ROOT]/foo/foo)
[CHECKING] bar v0.1.0 ([ROOT]/foo/bar)
[FIXED] [..]foo/src/shared.rs (2 fixes)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]
            .unordered(),
        )
        .run();

    assert_e2e().eq(
        &p.read_file("foo/src/shared.rs"),
        str!["pub fn fixme(_x: Box<&dyn Fn() -> ()>) {}"],
    );
}

#[cargo_test]
fn abnormal_exit() {
    // rustc fails unexpectedly after applying fixes, should show some error information.
    //
    // This works with a proc-macro that runs twice:
    // - First run (collect diagnostics pass): writes a file, exits normally.
    // - Second run (verify diagnostics work): it detects the presence of the
    //   file, removes the file, and aborts the process.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

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
        // "signal: 6, SIGABRT: process abort signal" on some platforms
        .with_stderr_data(str![[r#"
...
[WARNING] failed to automatically apply fixes suggested by rustc to crate `foo`
...
I'm not a diagnostic.
rustc exited abnormally: [..]
Original diagnostics will follow.
...
"#]])
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
        .with_stderr_data(str![[r#"
...
[..]use std::str::from_utf8;
...
  = [NOTE] `#[warn(unused_imports)]` [..]on by default
...
"#]])
        .run();
    p.cargo("fix --edition --allow-no-vcs").run();
    let contents = p.read_file("src/lib.rs");
    // Check it does not remove the "unused" import.
    assert!(contents.contains("use std::str::from_utf8;"));
    // Check that it made the edition migration.
    assert!(contents.contains("from_utf8(crate::foo::FOO)"));
}

#[cargo_test]
fn fix_in_dependency() {
    // Tests what happens if rustc emits a suggestion to modify a file from a
    // dependency in cargo's home directory. This should never happen, and
    // indicates a bug in rustc. However, there are several known bugs in
    // rustc where it does this (often involving macros), so `cargo fix` has a
    // guard that says if the suggestion points to some location in CARGO_HOME
    // to not apply it.
    //
    // See https://github.com/rust-lang/cargo/issues/9857 for some other
    // examples.
    //
    // This test uses a simulated rustc which replays a suggestion via a JSON
    // message that points into CARGO_HOME. This does not use the real rustc
    // because as the bugs are fixed in the real rustc, that would cause this
    // test to stop working.
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
                edition = "2015"

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
    p.cargo("fetch").run();

    // The path in CARGO_HOME.
    let bar_path = std::fs::read_dir(paths::home().join(".cargo/registry/src"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    // Since this is a substitution into a Rust string (representing a JSON
    // string), deal with backslashes like on Windows.
    let bar_path_str = bar_path.to_str().unwrap().replace("\\", "/");

    // This is a fake rustc that will emit a JSON message when the `foo` crate
    // builds that tells cargo to modify a file it shouldn't.
    let rustc = project()
        .at("rustc-replay")
        .file("Cargo.toml", &basic_manifest("rustc-replay", "1.0.0"))
        .file("src/main.rs",
            &r##"
                fn main() {
                    let pkg_name = match std::env::var("CARGO_PKG_NAME") {
                        Ok(pkg_name) => pkg_name,
                        Err(_) => {
                            let r = std::process::Command::new("rustc")
                                .args(std::env::args_os().skip(1))
                                .status();
                            std::process::exit(r.unwrap().code().unwrap_or(2));
                        }
                    };
                    if pkg_name == "foo" {
                        eprintln!("{}", r#"{
                          "$message_type": "diagnostic",
                          "message": "unused variable: `abc`",
                          "code":
                          {
                            "code": "unused_variables",
                            "explanation": null
                          },
                          "level": "warning",
                          "spans":
                          [
                            {
                              "file_name": "__BAR_PATH__/bar-1.0.0/src/lib.rs",
                              "byte_start": 127,
                              "byte_end": 129,
                              "line_start": 5,
                              "line_end": 5,
                              "column_start": 29,
                              "column_end": 31,
                              "is_primary": true,
                              "text":
                              [
                                {
                                  "text": "                        let $i = 1;",
                                  "highlight_start": 29,
                                  "highlight_end": 31
                                }
                              ],
                              "label": null,
                              "suggested_replacement": null,
                              "suggestion_applicability": null,
                              "expansion": null
                            }
                          ],
                          "children":
                          [
                            {
                              "message": "`#[warn(unused_variables)]` on by default",
                              "code": null,
                              "level": "note",
                              "spans":
                              [],
                              "children":
                              [],
                              "rendered": null
                            },
                            {
                              "message": "if this is intentional, prefix it with an underscore",
                              "code": null,
                              "level": "help",
                              "spans":
                              [
                                {
                                  "file_name": "__BAR_PATH__/bar-1.0.0/src/lib.rs",
                                  "byte_start": 127,
                                  "byte_end": 129,
                                  "line_start": 5,
                                  "line_end": 5,
                                  "column_start": 29,
                                  "column_end": 31,
                                  "is_primary": true,
                                  "text":
                                  [
                                    {
                                      "text": "                        let $i = 1;",
                                      "highlight_start": 29,
                                      "highlight_end": 31
                                    }
                                  ],
                                  "label": null,
                                  "suggested_replacement": "_abc",
                                  "suggestion_applicability": "MachineApplicable",
                                  "expansion": null
                                }
                              ],
                              "children":
                              [],
                              "rendered": null
                            }
                          ],
                          "rendered": "warning: unused variable: `abc`\n --> __BAR_PATH__/bar-1.0.0/src/lib.rs:5:29\n  |\n5 |                         let $i = 1;\n  |                             ^^ help: if this is intentional, prefix it with an underscore: `_abc`\n  |\n  = note: `#[warn(unused_variables)]` on by default\n\n"
                        }"#.replace("\n", ""));
                    }
                }
            "##.replace("__BAR_PATH__", &bar_path_str))
        .build();
    rustc.cargo("build").run();
    let rustc_bin = rustc.bin("rustc-replay");

    // The output here should not say `Fixed`.
    //
    // It is OK to compare the full diagnostic output here because the text is
    // hard-coded in rustc-replay. Normally tests should not be checking the
    // compiler output.
    p.cargo("fix --lib --allow-no-vcs")
        .env("RUSTC", &rustc_bin)
        .with_stderr_data(str![[r#"
[CHECKING] bar v1.0.0
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[WARNING] unused variable: `abc`
 --> [ROOT]/home/.cargo/registry/src/-[HASH]/bar-1.0.0/src/lib.rs:5:29
  |
5 |                         let $i = 1;
  |                             ^^ [HELP] if this is intentional, prefix it with an underscore: `_abc`
  |
  = [NOTE] `#[warn(unused_variables)]` on by default

[WARNING] `foo` (lib) generated 1 warning (run `cargo fix --lib -p foo` to apply 1 suggestion)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn fix_in_rust_src() {
    // Tests what happens if rustc emits a suggestion to modify the standard
    // library in rust source. This should never happen, and indicates a bug in
    // rustc. However, there are several known bugs in rustc where it does this
    // (often involving macros), so `cargo fix` has a guard that says if the
    // suggestion points to rust source under sysroot to not apply it.
    //
    // See https://github.com/rust-lang/cargo/issues/9857 for some other
    // examples.
    //
    // This test uses a simulated rustc which replays a suggestion via a JSON
    // message that points into rust-src. This does not use the real rustc
    // because as the bugs are fixed in the real rustc, that would cause this
    // test to stop working.

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                edition = "2021"
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
                pub fn bug_report<W: std::fmt::Write>(w: &mut W) -> std::fmt::Result {
                    if true {
                        writeln!(w, "`;?` here ->")?;
                    } else {
                        writeln!(w, "but not here")
                    }
                    Ok(())
                }
            "#,
        )
        .build();
    p.cargo("fetch").run();

    // Since this is a substitution into a Rust string (representing a JSON
    // string), deal with backslashes like on Windows.
    let sysroot = paths::sysroot().replace("\\", "/");

    // This is a fake rustc that will emit a JSON message when the `foo` crate
    // builds that tells cargo to modify a file it shouldn't.
    let rustc = project()
        .at("rustc-replay")
        .file("Cargo.toml", &basic_manifest("rustc-replay", "1.0.0"))
        .file("src/main.rs",
            &r##"
                fn main() {
                    let pkg_name = match std::env::var("CARGO_PKG_NAME") {
                        Ok(pkg_name) => pkg_name,
                        Err(_) => {
                            let r = std::process::Command::new("rustc")
                                .args(std::env::args_os().skip(1))
                                .status();
                            std::process::exit(r.unwrap().code().unwrap_or(2));
                        }
                    };
                    if pkg_name == "foo" {
                        eprintln!("{}", r#"{
    "$message_type": "diagnostic",
    "message": "mismatched types",
    "code":
    {
        "code": "E0308",
        "explanation": "Expected type did not match the received type.\n\nErroneous code examples:\n\n```compile_fail,E0308\nfn plus_one(x: i32) -> i32 {\n    x + 1\n}\n\nplus_one(\"Not a number\");\n//       ^^^^^^^^^^^^^^ expected `i32`, found `&str`\n\nif \"Not a bool\" {\n// ^^^^^^^^^^^^ expected `bool`, found `&str`\n}\n\nlet x: f32 = \"Not a float\";\n//     ---   ^^^^^^^^^^^^^ expected `f32`, found `&str`\n//     |\n//     expected due to this\n```\n\nThis error occurs when an expression was used in a place where the compiler\nexpected an expression of a different type. It can occur in several cases, the\nmost common being when calling a function and passing an argument which has a\ndifferent type than the matching type in the function declaration.\n"
    },
    "level": "error",
    "spans":
    [
        {
            "file_name": "__SYSROOT__/lib/rustlib/src/rust/library/core/src/macros/mod.rs",
            "byte_start": 23568,
            "byte_end": 23617,
            "line_start": 670,
            "line_end": 670,
            "column_start": 9,
            "column_end": 58,
            "is_primary": true,
            "text":
            [
                {
                    "text": "        $dst.write_fmt($crate::format_args_nl!($($arg)*))",
                    "highlight_start": 9,
                    "highlight_end": 58
                }
            ],
            "label": "expected `()`, found `Result<(), Error>`",
            "suggested_replacement": null,
            "suggestion_applicability": null,
            "expansion":
            {
                "span":
                {
                    "file_name": "lib.rs",
                    "byte_start": 144,
                    "byte_end": 171,
                    "line_start": 5,
                    "line_end": 5,
                    "column_start": 9,
                    "column_end": 36,
                    "is_primary": false,
                    "text":
                    [
                        {
                            "text": "        writeln!(w, \"but not here\")",
                            "highlight_start": 9,
                            "highlight_end": 36
                        }
                    ],
                    "label": null,
                    "suggested_replacement": null,
                    "suggestion_applicability": null,
                    "expansion": null
                },
                "macro_decl_name": "writeln!",
                "def_site_span":
                {
                    "file_name": "__SYSROOT__/lib/rustlib/src/rust/library/core/src/macros/mod.rs",
                    "byte_start": 23434,
                    "byte_end": 23454,
                    "line_start": 665,
                    "line_end": 665,
                    "column_start": 1,
                    "column_end": 21,
                    "is_primary": false,
                    "text":
                    [
                        {
                            "text": "macro_rules! writeln {",
                            "highlight_start": 1,
                            "highlight_end": 21
                        }
                    ],
                    "label": null,
                    "suggested_replacement": null,
                    "suggestion_applicability": null,
                    "expansion": null
                }
            }
        },
        {
            "file_name": "lib.rs",
            "byte_start": 75,
            "byte_end": 177,
            "line_start": 2,
            "line_end": 6,
            "column_start": 5,
            "column_end": 6,
            "is_primary": false,
            "text":
            [
                {
                    "text": "    if true {",
                    "highlight_start": 5,
                    "highlight_end": 14
                },
                {
                    "text": "        writeln!(w, \"`;?` here ->\")?;",
                    "highlight_start": 1,
                    "highlight_end": 38
                },
                {
                    "text": "    } else {",
                    "highlight_start": 1,
                    "highlight_end": 13
                },
                {
                    "text": "        writeln!(w, \"but not here\")",
                    "highlight_start": 1,
                    "highlight_end": 36
                },
                {
                    "text": "    }",
                    "highlight_start": 1,
                    "highlight_end": 6
                }
            ],
            "label": "expected this to be `()`",
            "suggested_replacement": null,
            "suggestion_applicability": null,
            "expansion": null
        }
    ],
    "children":
    [
        {
            "message": "use the `?` operator to extract the `Result<(), std::fmt::Error>` value, propagating a `Result::Err` value to the caller",
            "code": null,
            "level": "help",
            "spans":
            [
                {
                    "file_name": "__SYSROOT__/lib/rustlib/src/rust/library/core/src/macros/mod.rs",
                    "byte_start": 23617,
                    "byte_end": 23617,
                    "line_start": 670,
                    "line_end": 670,
                    "column_start": 58,
                    "column_end": 58,
                    "is_primary": true,
                    "text":
                    [
                        {
                            "text": "        $dst.write_fmt($crate::format_args_nl!($($arg)*))",
                            "highlight_start": 58,
                            "highlight_end": 58
                        }
                    ],
                    "label": null,
                    "suggested_replacement": "?",
                    "suggestion_applicability": "HasPlaceholders",
                    "expansion":
                    {
                        "span":
                        {
                            "file_name": "lib.rs",
                            "byte_start": 144,
                            "byte_end": 171,
                            "line_start": 5,
                            "line_end": 5,
                            "column_start": 9,
                            "column_end": 36,
                            "is_primary": false,
                            "text":
                            [
                                {
                                    "text": "        writeln!(w, \"but not here\")",
                                    "highlight_start": 9,
                                    "highlight_end": 36
                                }
                            ],
                            "label": null,
                            "suggested_replacement": null,
                            "suggestion_applicability": null,
                            "expansion": null
                        },
                        "macro_decl_name": "writeln!",
                        "def_site_span":
                        {
                            "file_name": "__SYSROOT__/lib/rustlib/src/rust/library/core/src/macros/mod.rs",
                            "byte_start": 23434,
                            "byte_end": 23454,
                            "line_start": 665,
                            "line_end": 665,
                            "column_start": 1,
                            "column_end": 21,
                            "is_primary": false,
                            "text":
                            [
                                {
                                    "text": "macro_rules! writeln {",
                                    "highlight_start": 1,
                                    "highlight_end": 21
                                }
                            ],
                            "label": null,
                            "suggested_replacement": null,
                            "suggestion_applicability": null,
                            "expansion": null
                        }
                    }
                }
            ],
            "children":
            [],
            "rendered": null
        }
    ],
    "rendered": "error[E0308]: mismatched types\n --> lib.rs:5:9\n  |\n2 | /     if true {\n3 | |         writeln!(w, \"`;?` here ->\")?;\n4 | |     } else {\n5 | |         writeln!(w, \"but not here\")\n  | |         ^^^^^^^^^^^^^^^^^^^^^^^^^^^ expected `()`, found `Result<(), Error>`\n6 | |     }\n  | |_____- expected this to be `()`\n  |\n  = note: expected unit type `()`\n                  found enum `Result<(), std::fmt::Error>`\n  = note: this error originates in the macro `writeln` (in Nightly builds, run with -Z macro-backtrace for more info)\nhelp: consider using a semicolon here\n  |\n6 |     };\n  |      +\nhelp: you might have meant to return this value\n  |\n5 |         return writeln!(w, \"but not here\");\n  |         ++++++                            +\nhelp: use the `?` operator to extract the `Result<(), std::fmt::Error>` value, propagating a `Result::Err` value to the caller\n --> __SYSROOT__/lib/rustlib/src/rust/library/core/src/macros/mod.rs:670:58\n  |\n67|         $dst.write_fmt($crate::format_args_nl!($($arg)*))?\n  |                                                          +\n\n"
}"#.replace("\n", ""));

                        std::process::exit(2);
                    }
                }
            "##.replace("__SYSROOT__", &sysroot))
        .build();
    rustc.cargo("build").run();
    let rustc_bin = rustc.bin("rustc-replay");

    // The output here should not say `Fixed`.
    //
    // It is OK to compare the full diagnostic output here because the text is
    // hard-coded in rustc-replay. Normally tests should not be checking the
    // compiler output.
    p.cargo("fix --lib --allow-no-vcs --broken-code")
        .env("__CARGO_FIX_YOLO", "1")
        .env("RUSTC", &rustc_bin)
        .with_status(101)
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.0.0 ([ROOT]/foo)
error[E0308]: mismatched types
 --> lib.rs:5:9
  |
2 | /     if true {
3 | |         writeln!(w, "`;?` here ->")?;
4 | |     } else {
5 | |         writeln!(w, "but not here")
  | |         ^^^^^^^^^^^^^^^^^^^^^^^^^^^ expected `()`, found `Result<(), Error>`
6 | |     }
  | |_____- expected this to be `()`
  |
  = [NOTE] expected unit type `()`
                  found enum `Result<(), std::fmt::Error>`
  = [NOTE] this error originates in the macro `writeln` (in Nightly builds, run with -Z macro-backtrace for more info)
[HELP] consider using a semicolon here
  |
6 |     };
  |      +
[HELP] you might have meant to return this value
  |
5 |         return writeln!(w, "but not here");
  |         ++++++                            +
[HELP] use the `?` operator to extract the `Result<(), std::fmt::Error>` value, propagating a `Result::Err` value to the caller
 --> [..]/lib/rustlib/src/rust/library/core/src/macros/mod.rs:670:58
  |
67|         $dst.write_fmt($crate::format_args_nl!($($arg)*))?
  |                                                          +

[ERROR] could not compile `foo` (lib) due to 1 previous error

"#]])
        .run();
}

// See <https://github.com/rust-lang/cargo/issues/13027>
#[cargo_test]
fn fix_only_once_for_duplicates() {
    let p = project()
        .file(
            "src/main.rs",
            r#"
macro_rules! foo {
    () => {
        let x = Box::new(1);
        std::mem::forget(&x);
    };
}

fn main() {
    foo!();
    foo!();
}
"#,
        )
        .build();

    p.cargo("fix --allow-no-vcs")
        .env("__CARGO_FIX_YOLO", "1")
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FIXED] src/main.rs (1 fix)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    assert_e2e().eq(
        p.read_file("src/main.rs"),
        str![[r#"

macro_rules! foo {
    () => {
        let x = Box::new(1);
        let _ = &x;
    };
}

fn main() {
    foo!();
    foo!();
}

"#]],
    );
}

#[cargo_test]
fn migrate_project_to_package() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
# Before project
[ project ] # After project header
# After project header line
name = "foo"
edition = "2021"
# After project table
"#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("fix --edition --allow-no-vcs")
        .with_stderr_data(str![[r#"
[MIGRATING] Cargo.toml from 2021 edition to 2024
[FIXED] Cargo.toml (1 fix)
[CHECKING] foo v0.0.0 ([ROOT]/foo)
[MIGRATING] src/lib.rs from 2021 edition to 2024
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    assert_e2e().eq(
        p.read_file("Cargo.toml"),
        str![[r#"

# Before project
[ package ] # After project header
# After project header line
name = "foo"
edition = "2021"
# After project table

"#]],
    );
}

#[cargo_test]
fn migrate_removes_project() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
# Before package
[ package ] # After package header
# After package header line
name = "foo"
edition = "2021"
# After package table

# Before project
[ project ] # After project header
# After project header line
name = "foo"
edition = "2021"
# After project table
"#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("fix --edition --allow-no-vcs")
        .with_stderr_data(str![[r#"
[MIGRATING] Cargo.toml from 2021 edition to 2024
[FIXED] Cargo.toml (1 fix)
[CHECKING] foo v0.0.0 ([ROOT]/foo)
[MIGRATING] src/lib.rs from 2021 edition to 2024
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    assert_e2e().eq(
        p.read_file("Cargo.toml"),
        str![[r#"

# Before package
[ package ] # After package header
# After package header line
name = "foo"
edition = "2021"
# After project table

"#]],
    );
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
fn migrate_removes_project_for_script() {
    let p = project()
        .file(
            "foo.rs",
            r#"
---
# Before package
[ package ] # After package header
# After package header line
name = "foo"
edition = "2021"
# After package table

# Before project
[ project ] # After project header
# After project header line
name = "foo"
edition = "2021"
# After project table
---

fn main() {
}
"#,
        )
        .build();

    p.cargo("-Zscript fix --edition --allow-no-vcs --manifest-path foo.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stderr_data(str![[r#"
[MIGRATING] foo.rs from 2021 edition to 2024
[FIXED] foo.rs (1 fix)
[CHECKING] foo v0.0.0 ([ROOT]/foo/foo.rs)
[MIGRATING] foo.rs from 2021 edition to 2024
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    assert_e2e().eq(
        p.read_file("foo.rs"),
        str![[r#"

---
# Before package
[ package ] # After package header
# After package header line
name = "foo"
edition = "2021"
# After project table
---

fn main() {
}

"#]],
    );
}

#[cargo_test]
fn migrate_rename_underscore_fields() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
[workspace.dependencies]
# Before default_features
a = {path = "a", default_features = false}  # After default_features value
# After default_features line

[package]
name = "foo"
edition = "2021"

[lib]
name = "foo"
# Before crate_type
crate_type = ["staticlib", "dylib"]  # After crate_type value
# After crate_type line

[[example]]
name = "ex"
path = "examples/ex.rs"
# Before crate_type
crate_type = ["proc-macro"]  # After crate_type value
# After crate_type line

# Before dev_dependencies
[ dev_dependencies ] # After dev_dependencies header
# After dev_dependencies line
a = {path = "a", default_features = false}
# After dev_dependencies table

# Before build_dependencies
[ build_dependencies ] # After build_dependencies header
# After build_dependencies line
a = {path = "a", default_features = false}
# After build_dependencies table

# Before dev_dependencies
[ target.'cfg(any())'.dev_dependencies ] # After dev_dependencies header
# After dev_dependencies line
a = {path = "a", default_features = false}
# After dev_dependencies table

# Before build_dependencies
[ target.'cfg(any())'.build_dependencies ] # After build_dependencies header
# After build_dependencies line
a = {path = "a", default_features = false}
# After build_dependencies table
"#,
        )
        .file("src/lib.rs", "")
        .file(
            "examples/ex.rs",
            r#"
                fn main() { println!("ex"); }
            "#,
        )
        .file(
            "a/Cargo.toml",
            r#"
                [package]
                name = "a"
                version = "0.0.1"
                edition = "2015"
            "#,
        )
        .file("a/src/lib.rs", "")
        .build();

    p.cargo("fix --edition --allow-no-vcs")
        .with_stderr_data(str![[r#"
[MIGRATING] Cargo.toml from 2021 edition to 2024
[FIXED] Cargo.toml (11 fixes)
[CHECKING] a v0.0.1 ([ROOT]/foo/a)
[CHECKING] foo v0.0.0 ([ROOT]/foo)
[MIGRATING] src/lib.rs from 2021 edition to 2024
[MIGRATING] examples/ex.rs from 2021 edition to 2024
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    assert_e2e().eq(
        p.read_file("Cargo.toml"),
        str![[r#"

[workspace.dependencies]
# Before default_features
a = {path = "a", default-features = false}  # After default_features value
# After default_features line

[package]
name = "foo"
edition = "2021"

[lib]
name = "foo"
# Before crate_type
crate-type = ["staticlib", "dylib"]  # After crate_type value
# After crate_type line

[[example]]
name = "ex"
path = "examples/ex.rs"
# Before crate_type
crate-type = ["proc-macro"]  # After crate_type value
# After crate_type line

# Before dev_dependencies
[ dev-dependencies ] # After dev_dependencies header
# After dev_dependencies line
a = {path = "a", default-features = false}
# After dev_dependencies table

# Before build_dependencies
[ build-dependencies ] # After build_dependencies header
# After build_dependencies line
a = {path = "a", default-features = false}
# After build_dependencies table

# Before dev_dependencies
[ target.'cfg(any())'.dev-dependencies ] # After dev_dependencies header
# After dev_dependencies line
a = {path = "a", default-features = false}
# After dev_dependencies table

# Before build_dependencies
[ target.'cfg(any())'.build-dependencies ] # After build_dependencies header
# After build_dependencies line
a = {path = "a", default-features = false}
# After build_dependencies table

"#]],
    );
}

#[cargo_test]
fn migrate_rename_underscore_fields_in_virtual_manifest() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
[workspace]
members = ["foo"]
resolver = "2"

[workspace.dependencies]
# Before default_features
a = {path = "a", default_features = false}  # After default_features value
# After default_features line
"#,
        )
        .file(
            "foo/Cargo.toml",
            r#"
[package]
name = "foo"
edition = "2021"
"#,
        )
        .file("foo/src/lib.rs", "")
        .file(
            "a/Cargo.toml",
            r#"
                [package]
                name = "a"
                version = "0.0.1"
                edition = "2015"
            "#,
        )
        .file("a/src/lib.rs", "")
        .build();

    p.cargo("fix --edition --allow-no-vcs")
        .with_stderr_data(str![[r#"
[MIGRATING] Cargo.toml from 2021 edition to 2024
[FIXED] Cargo.toml (1 fix)
[MIGRATING] foo/Cargo.toml from 2021 edition to 2024
[CHECKING] foo v0.0.0 ([ROOT]/foo/foo)
[MIGRATING] foo/src/lib.rs from 2021 edition to 2024
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    assert_e2e().eq(
        p.read_file("Cargo.toml"),
        str![[r#"

[workspace]
members = ["foo"]
resolver = "2"

[workspace.dependencies]
# Before default_features
a = {path = "a", default-features = false}  # After default_features value
# After default_features line

"#]],
    );
    assert_e2e().eq(
        p.read_file("foo/Cargo.toml"),
        str![[r#"

[package]
name = "foo"
edition = "2021"

"#]],
    );
}

#[cargo_test]
fn remove_ignored_default_features() {
    Package::new("dep_simple", "0.1.0").publish();
    Package::new("dep_df_true", "0.1.0").publish();
    Package::new("dep_df_false", "0.1.0").publish();

    let pkg_default = r#"
[package]
name = "pkg_default"
version = "0.1.0"
edition = "2021"

[dependencies]
dep_simple = { workspace = true }
dep_df_true = { workspace = true }
dep_df_false = { workspace = true }

[build-dependencies]
dep_simple = { workspace = true }
dep_df_true = { workspace = true }
dep_df_false = { workspace = true }

[target.'cfg(target_os = "linux")'.dependencies]
dep_simple = { workspace = true }
dep_df_true = { workspace = true }
dep_df_false = { workspace = true }
"#;
    let pkg_df_true = r#"
[package]
name = "pkg_df_true"
version = "0.1.0"
edition = "2021"

[dependencies]
dep_simple = { workspace = true, default-features = true }
dep_df_true = { workspace = true, default-features = true }
dep_df_false = { workspace = true, default-features = true }

[build-dependencies]
dep_simple = { workspace = true, default-features = true }
dep_df_true = { workspace = true, default-features = true }
dep_df_false = { workspace = true, default-features = true }

[target.'cfg(target_os = "linux")'.dependencies]
dep_simple = { workspace = true, default-features = true }
dep_df_true = { workspace = true, default-features = true }
dep_df_false = { workspace = true, default-features = true }
"#;
    let pkg_df_false = r#"
[package]
name = "pkg_df_false"
version = "0.1.0"
edition = "2021"

[dependencies]
dep_simple = { workspace = true, default-features = false }
dep_df_true = { workspace = true, default-features = false }
dep_df_false = { workspace = true, default-features = false }

[build-dependencies]
dep_simple = { workspace = true, default-features = false }
dep_df_true = { workspace = true, default-features = false }
dep_df_false = { workspace = true, default-features = false }

[target.'cfg(target_os = "linux")'.dependencies]
dep_simple = { workspace = true, default-features = false }
dep_df_true = { workspace = true, default-features = false }
dep_df_false = { workspace = true, default-features = false }
"#;
    let p = project()
        .file(
            "Cargo.toml",
            r#"
[workspace]
members = ["pkg_default", "pkg_df_true", "pkg_df_false"]
resolver = "2"

[workspace.dependencies]
dep_simple = "0.1.0"
dep_df_true = { version = "0.1.0", default-features = true }
dep_df_false = { version = "0.1.0", default-features = false }
"#,
        )
        .file("pkg_default/Cargo.toml", pkg_default)
        .file("pkg_default/src/lib.rs", "")
        .file("pkg_df_true/Cargo.toml", pkg_df_true)
        .file("pkg_df_true/src/lib.rs", "")
        .file("pkg_df_false/Cargo.toml", pkg_df_false)
        .file("pkg_df_false/src/lib.rs", "")
        .build();

    p.cargo("fix --all --edition --allow-no-vcs")
        .with_stderr_data(
            str![[r#"
[MIGRATING] pkg_default/Cargo.toml from 2021 edition to 2024
[MIGRATING] pkg_df_true/Cargo.toml from 2021 edition to 2024
[MIGRATING] pkg_df_false/Cargo.toml from 2021 edition to 2024
[FIXED] pkg_df_false/Cargo.toml (6 fixes)
[UPDATING] `dummy-registry` index
[LOCKING] 3 packages to latest compatible versions
[DOWNLOADING] crates ...
[DOWNLOADED] dep_simple v0.1.0 (registry `dummy-registry`)
[DOWNLOADED] dep_df_true v0.1.0 (registry `dummy-registry`)
[DOWNLOADED] dep_df_false v0.1.0 (registry `dummy-registry`)
[CHECKING] dep_df_true v0.1.0
[CHECKING] dep_df_false v0.1.0
[CHECKING] dep_simple v0.1.0
[CHECKING] pkg_df_true v0.1.0 ([ROOT]/foo/pkg_df_true)
[CHECKING] pkg_df_false v0.1.0 ([ROOT]/foo/pkg_df_false)
[CHECKING] pkg_default v0.1.0 ([ROOT]/foo/pkg_default)
[MIGRATING] pkg_df_false/src/lib.rs from 2021 edition to 2024
[MIGRATING] pkg_df_true/src/lib.rs from 2021 edition to 2024
[MIGRATING] pkg_default/src/lib.rs from 2021 edition to 2024
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]
            .unordered(),
        )
        .run();

    assert_e2e().eq(p.read_file("pkg_default/Cargo.toml"), pkg_default);
    assert_e2e().eq(p.read_file("pkg_df_true/Cargo.toml"), pkg_df_true);
    assert_e2e().eq(
        p.read_file("pkg_df_false/Cargo.toml"),
        str![[r#"

[package]
name = "pkg_df_false"
version = "0.1.0"
edition = "2021"

[dependencies]
dep_simple = { workspace = true}
dep_df_true = { workspace = true}
dep_df_false = { workspace = true, default-features = false }

[build-dependencies]
dep_simple = { workspace = true}
dep_df_true = { workspace = true}
dep_df_false = { workspace = true, default-features = false }

[target.'cfg(target_os = "linux")'.dependencies]
dep_simple = { workspace = true}
dep_df_true = { workspace = true}
dep_df_false = { workspace = true, default-features = false }

"#]],
    );
}

#[cargo_test]
fn fix_edition_skips_old_editions() {
    // Checks that -Zfix-edition will skip things that are not 2024.
    let p = project()
        .file(
            "Cargo.toml",
            r#"[workspace]
            members = ["e2021", "e2024"]
            resolver = "3"
            "#,
        )
        .file(
            "e2021/Cargo.toml",
            r#"
            [package]
            name = "e2021"
            edition = "2021"
            "#,
        )
        .file("e2021/src/lib.rs", "")
        .file(
            "e2024/Cargo.toml",
            r#"
                [package]
                name = "e2024"
                edition = "2024"
            "#,
        )
        .file("e2024/src/lib.rs", "")
        .build();

    // Doing the whole workspace should skip since there is a 2021 in the mix.
    p.cargo("fix -Zfix-edition=start=2024 -v")
        .masquerade_as_nightly_cargo(&["fix-edition"])
        .with_stderr_data(str![[r#"
[SKIPPING] not all packages are at edition 2024

"#]])
        .run();

    // Same with `end`.
    p.cargo("fix -Zfix-edition=end=2024,future -v")
        .masquerade_as_nightly_cargo(&["fix-edition"])
        .with_stderr_data(str![[r#"
[SKIPPING] not all packages are at edition 2024

"#]])
        .run();

    // Doing an individual package at the correct edition should check it.
    p.cargo("fix -Zfix-edition=start=2024 -p e2024")
        .masquerade_as_nightly_cargo(&["fix-edition"])
        .with_stderr_data(str![[r#"
[CHECKING] e2024 v0.0.0 ([ROOT]/foo/e2024)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test(nightly, reason = "future edition is always unstable")]
fn fix_edition_future() {
    // Checks that the -Zfix-edition can work for the future.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            edition = "2024""#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("fix -Zfix-edition=end=2024,future")
        .masquerade_as_nightly_cargo(&["fix-edition"])
        .with_stderr_data(str![[r#"
[MIGRATING] Cargo.toml from 2024 edition to future
[CHECKING] foo v0.0.0 ([ROOT]/foo)
[MIGRATING] src/lib.rs from 2024 edition to future
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
     Updated edition to future
[CHECKING] foo v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    assert_e2e().eq(
        p.read_file("Cargo.toml"),
        str![[r#"
cargo-features = ["unstable-editions"]

            [package]
            name = "foo"
edition = "future"

"#]],
    );
}
