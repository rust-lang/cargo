//! A test suite for `-Zbuild-std` which is much more expensive than the
//! standard test suite.
//!
//! This test suite attempts to perform a full integration test where we
//! actually compile the standard library from source (like the real one) and
//! the various tests associated with that.
//!
//! YOU SHOULD IDEALLY NOT WRITE TESTS HERE.
//!
//! If possible, use `tests/testsuite/standard_lib.rs` instead. That uses a
//! 'mock' sysroot which is much faster to compile. The tests here are
//! extremely intensive and are only intended to run on CI and are theoretically
//! not catching any regressions that `tests/testsuite/standard_lib.rs` isn't
//! already catching.
//!
//! All tests here should use `#[cargo_test(build_std_real)]` to indicate that
//! boilerplate should be generated to require the nightly toolchain and the
//! `CARGO_RUN_BUILD_STD_TESTS` env var to be set to actually run these tests.
//! Otherwise the tests are skipped.

#![allow(clippy::disallowed_methods)]

use cargo_test_support::Execs;
use cargo_test_support::basic_manifest;
use cargo_test_support::paths;
use cargo_test_support::project;
use cargo_test_support::rustc_host;
use cargo_test_support::str;
use cargo_test_support::target_spec_json;
use cargo_test_support::{Project, prelude::*};
use std::env;
use std::path::{Path, PathBuf};

fn enable_build_std(e: &mut Execs, arg: Option<&str>, isolated: bool) {
    if !isolated {
        e.env_remove("CARGO_HOME");
        e.env_remove("HOME");
    }

    // And finally actually enable `build-std` for now
    let arg = match arg {
        Some(s) => format!("-Zbuild-std={}", s),
        None => "-Zbuild-std".to_string(),
    };
    e.arg(arg).arg("-Zpublic-dependency");
    e.masquerade_as_nightly_cargo(&["build-std"]);
}

// Helper methods used in the tests below
trait BuildStd: Sized {
    /// Set `-Zbuild-std` args and will download dependencies of the standard
    /// library in users's `CARGO_HOME` (`~/.cargo/`) instead of isolated
    /// environment `cargo-test-support` usually provides.
    ///
    /// The environment is not isolated is to avoid excessive network requests
    /// and downloads. A side effect is `[BLOCKING]` will show up in stderr,
    /// as a sign of package cache lock contention when running other build-std
    /// tests concurrently.
    fn build_std(&mut self) -> &mut Self;

    /// Like [`BuildStd::build_std`] and is able to specify what crates to build.
    fn build_std_arg(&mut self, arg: &str) -> &mut Self;

    /// Like [`BuildStd::build_std`] but use an isolated `CARGO_HOME` environment
    /// to avoid package cache lock contention.
    ///
    /// Don't use this unless you really need to assert the full stderr
    /// and avoid any `[BLOCKING]` message.
    fn build_std_isolated(&mut self) -> &mut Self;
    fn target_host(&mut self) -> &mut Self;
}

impl BuildStd for Execs {
    fn build_std(&mut self) -> &mut Self {
        enable_build_std(self, None, false);
        self
    }

    fn build_std_arg(&mut self, arg: &str) -> &mut Self {
        enable_build_std(self, Some(arg), false);
        self
    }

    fn build_std_isolated(&mut self) -> &mut Self {
        enable_build_std(self, None, true);
        self
    }

    fn target_host(&mut self) -> &mut Self {
        self.arg("--target").arg(rustc_host());
        self
    }
}

#[cargo_test(build_std_real)]
fn basic() {
    let p = project()
        .file(
            "src/main.rs",
            "
                fn main() {
                    foo::f();
                }

                #[test]
                fn smoke_bin_unit() {
                    foo::f();
                }
            ",
        )
        .file(
            "src/lib.rs",
            "
                extern crate alloc;
                extern crate proc_macro;

                /// ```
                /// foo::f();
                /// ```
                pub fn f() {
                }

                #[test]
                fn smoke_lib_unit() {
                    f();
                }
            ",
        )
        .file(
            "tests/smoke.rs",
            "
                #[test]
                fn smoke_integration() {
                    foo::f();
                }
            ",
        )
        .build();

    // HACK: use an isolated the isolated CARGO_HOME environment (`build_std_isolated`)
    // to avoid `[BLOCKING]` messages (from lock contention with other tests)
    // from getting in this test's asserts
    p.cargo("check").build_std_isolated().target_host().run();
    p.cargo("build")
        .build_std_isolated()
        .target_host()
        // Importantly, this should not say [UPDATING]
        // There have been multiple bugs where every build triggers and update.
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    p.cargo("run")
        .build_std_isolated()
        .target_host()
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `target/[HOST_TARGET]/debug/foo`

"#]])
        .run();
    p.cargo("test")
        .build_std_isolated()
        .target_host()
        .with_stderr_data(str![[r#"
[COMPILING] [..]
...
[COMPILING] test v0.0.0 ([..])
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] unittests src/lib.rs (target/[HOST_TARGET]/debug/deps/foo-[HASH])
[RUNNING] unittests src/main.rs (target/[HOST_TARGET]/debug/deps/foo-[HASH])
[RUNNING] tests/smoke.rs (target/[HOST_TARGET]/debug/deps/smoke-[HASH])
[DOCTEST] foo

"#]])
        .run();

    // Check for hack that removes dylibs.
    let deps_dir = Path::new("target")
        .join(rustc_host())
        .join("debug")
        .join("deps");
    assert!(p.glob(deps_dir.join("*.rlib")).count() > 0);
    assert_eq!(p.glob(deps_dir.join("*.dylib")).count(), 0);
}

#[cargo_test(build_std_real)]
fn host_proc_macro() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2021"

                [dependencies]
                macro_test = { path = "macro_test" }
            "#,
        )
        .file(
            "src/main.rs",
            r#"
            extern crate macro_test;
            use macro_test::make_answer;

            make_answer!();

            fn main() {
                println!("Hello, World: {}", answer());
            }
            "#,
        )
        .file(
            "macro_test/Cargo.toml",
            r#"
            [package]
            name = "macro_test"
            version = "0.1.0"
            edition = "2021"

            [lib]
            proc-macro = true
            "#,
        )
        .file(
            "macro_test/src/lib.rs",
            r#"
            extern crate proc_macro;
            use proc_macro::TokenStream;

            #[proc_macro]
            pub fn make_answer(_item: TokenStream) -> TokenStream {
                "fn answer() -> u32 { 42 }".parse().unwrap()
            }
            "#,
        )
        .build();

    p.cargo("build")
        .build_std_arg("std")
        .build_std_arg("proc_macro")
        .run();
}

#[cargo_test(build_std_real)]
fn cross_custom() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2018"

                [target.custom-target.dependencies]
                dep = { path = "dep" }
            "#,
        )
        .file(
            "src/lib.rs",
            "#![no_std] pub fn f() -> u32 { dep::answer() }",
        )
        .file("dep/Cargo.toml", &basic_manifest("dep", "0.1.0"))
        .file("dep/src/lib.rs", "#![no_std] pub fn answer() -> u32 { 42 }")
        .file("custom-target.json", target_spec_json())
        .build();

    p.cargo("build --target custom-target.json -v")
        .build_std_arg("core")
        .run();
}

#[cargo_test(build_std_real)]
fn custom_test_framework() {
    let p = project()
        .file(
            "src/lib.rs",
            r#"
            #![no_std]
            #![cfg_attr(test, no_main)]
            #![feature(custom_test_frameworks)]
            #![test_runner(crate::test_runner)]

            pub fn test_runner(_tests: &[&dyn Fn()]) {}

            #[panic_handler]
            fn panic(_info: &core::panic::PanicInfo) -> ! {
                loop {}
            }
            "#,
        )
        .file("target.json", target_spec_json())
        .build();

    // This is a bit of a hack to use the rust-lld that ships with most toolchains.
    let sysroot = paths::sysroot();
    let sysroot = Path::new(&sysroot);
    let sysroot_bin = sysroot
        .join("lib")
        .join("rustlib")
        .join(rustc_host())
        .join("bin");
    let path = env::var_os("PATH").unwrap_or_default();
    let mut paths = env::split_paths(&path).collect::<Vec<_>>();
    paths.insert(0, sysroot_bin);
    let new_path = env::join_paths(paths).unwrap();

    p.cargo("test --target target.json --no-run -v")
        .env("PATH", new_path)
        .build_std_arg("core")
        .run();
}

// Fixing rust-lang/rust#117839.
// on macOS it never gets remapped.
// Might be a separate issue, so only run on Linux.
#[cargo_test(build_std_real)]
#[cfg(target_os = "linux")]
fn remap_path_scope() {
    let p = project()
        .file(
            "src/main.rs",
            "
                fn main() {
                    panic!(\"remap to /rustc/<hash>\");
                }
            ",
        )
        .file(
            ".cargo/config.toml",
            "
                [profile.release]
                debug = \"line-tables-only\"
            ",
        )
        .build();

    p.cargo("run --release -Ztrim-paths")
        .masquerade_as_nightly_cargo(&["-Ztrim-paths"])
        .env("RUST_BACKTRACE", "1")
        .build_std()
        .target_host()
        .with_status(101)
        .with_stderr_data(
            str![[r#"
[FINISHED] `release` profile [optimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `target/[HOST_TARGET]/release/foo`
...
[..]thread [..] panicked at [..]src/main.rs:3:[..]:
[..]remap to /rustc/<hash>[..]
[..]at /rustc/[..]/library/std/src/[..]
[..]at ./src/main.rs:3:[..]
[..]at /rustc/[..]/library/core/src/[..]
...
"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test(build_std_real)]
fn test_proc_macro() {
    // See rust-lang/cargo#14735
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                edition = "2021"

                [lib]
                proc-macro = true
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("test --lib")
        .env_remove(cargo_util::paths::dylib_path_envvar())
        .build_std()
        .with_stderr_data(str![[r#"
...
[COMPILING] foo v0.0.0 ([ROOT]/foo)
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] unittests src/lib.rs (target/debug/deps/foo-[HASH])

"#]])
        .run();
}

#[cargo_test(build_std_real)]
fn default_features_still_included_with_extra_build_std_features() {
    // This is a regression test to ensure when adding extra `build-std-features`,
    // the default feature set is still respected and included.
    // See rust-lang/cargo#14935
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                edition = "2021"
            "#,
        )
        .file("src/lib.rs", "#![no_std]")
        .build();

    p.cargo("check")
        .build_std_arg("std,panic_abort")
        .env("RUSTFLAGS", "-C panic=abort")
        .arg("-Zbuild-std-features=optimize_for_size")
        .run();
}

pub trait CargoProjectExt {
    /// Creates a `ProcessBuilder` to run cargo.
    ///
    /// Arguments can be separated by spaces.
    ///
    /// For `cargo run`, see [`Project::rename_run`].
    ///
    /// # Example:
    ///
    /// ```no_run
    /// # let p = cargo_test_support::project().build();
    /// p.cargo("build --bin foo").run();
    /// ```
    fn cargo(&self, cmd: &str) -> Execs;
}

impl CargoProjectExt for Project {
    fn cargo(&self, cmd: &str) -> Execs {
        let cargo = cargo_exe();
        let mut execs = self.process(&cargo);
        execs.env("CARGO", cargo);
        execs.arg_line(cmd);
        execs
    }
}

/// Path to the cargo binary
pub fn cargo_exe() -> PathBuf {
    snapbox::cmd::cargo_bin!("cargo").to_path_buf()
}
