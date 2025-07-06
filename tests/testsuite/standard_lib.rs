//! Tests for building the standard library (-Zbuild-std).
//!
//! These tests all use a "mock" standard library so that we don't have to
//! rebuild the real one. There is a separate integration test `build-std`
//! which builds the real thing, but that should be avoided if possible.

use std::path::{Path, PathBuf};

use crate::prelude::*;
use cargo_test_support::ProjectBuilder;
use cargo_test_support::cross_compile;
use cargo_test_support::registry::{Dependency, Package};
use cargo_test_support::{Execs, paths, project, rustc_host, str};

struct Setup {
    rustc_wrapper: PathBuf,
    real_sysroot: String,
}

fn setup() -> Setup {
    // Our mock sysroot requires a few packages from crates.io, so make sure
    // they're "published" to crates.io. Also edit their code a bit to make sure
    // that they have access to our custom crates with custom apis.
    Package::new("registry-dep-using-core", "1.0.0")
        .file(
            "src/lib.rs",
            "
                #![no_std]

                #[cfg(feature = \"mockbuild\")]
                pub fn custom_api() {
                }

                #[cfg(not(feature = \"mockbuild\"))]
                pub fn non_sysroot_api() {
                    core::custom_api();
                }
            ",
        )
        .add_dep(Dependency::new("rustc-std-workspace-core", "*").optional(true))
        .feature("mockbuild", &["rustc-std-workspace-core"])
        .publish();
    Package::new("registry-dep-using-alloc", "1.0.0")
        .file(
            "src/lib.rs",
            "
                #![no_std]

                extern crate alloc;

                #[cfg(feature = \"mockbuild\")]
                pub fn custom_api() {
                }

                #[cfg(not(feature = \"mockbuild\"))]
                pub fn non_sysroot_api() {
                    core::custom_api();
                    alloc::custom_api();
                }
            ",
        )
        .add_dep(Dependency::new("rustc-std-workspace-core", "*").optional(true))
        .add_dep(Dependency::new("rustc-std-workspace-alloc", "*").optional(true))
        .feature(
            "mockbuild",
            &["rustc-std-workspace-core", "rustc-std-workspace-alloc"],
        )
        .publish();
    Package::new("registry-dep-using-std", "1.0.0")
        .file(
            "src/lib.rs",
            "
                #[cfg(feature = \"mockbuild\")]
                pub fn custom_api() {
                }

                #[cfg(not(feature = \"mockbuild\"))]
                pub fn non_sysroot_api() {
                    std::custom_api();
                }
            ",
        )
        .add_dep(Dependency::new("rustc-std-workspace-std", "*").optional(true))
        .feature("mockbuild", &["rustc-std-workspace-std"])
        .publish();

    let p = ProjectBuilder::new(paths::root().join("rustc-wrapper"))
        .file(
            "src/main.rs",
            &r#"
                use std::process::Command;
                use std::env;
                fn main() {
                    let mut args = env::args().skip(1).collect::<Vec<_>>();

                    let is_sysroot_crate = env::var_os("RUSTC_BOOTSTRAP").is_some();
                    if is_sysroot_crate {
                        args.push("--sysroot".to_string());
                        args.push(env::var("REAL_SYSROOT").unwrap());
                    } else if let Some(pos) = args.iter().position(|arg| arg == "--target") {
                        // build-std target unit

                        // Set --sysroot only when the target is host
                        if args.iter().nth(pos + 1) == Some(&"__HOST_TARGET__".to_string()) {
                            // This `--sysroot` is here to disable the sysroot lookup,
                            // to ensure nothing is required.
                            // See https://github.com/rust-lang/wg-cargo-std-aware/issues/31
                            // for more information on this.
                            args.push("--sysroot".to_string());
                            args.push("/path/to/nowhere".to_string());
                        }
                    } else {
                        // host unit, do not use sysroot
                    }

                    let ret = Command::new(&args[0]).args(&args[1..]).status().unwrap();
                    std::process::exit(ret.code().unwrap_or(1));
                }
            "#
            .replace("__HOST_TARGET__", rustc_host()),
        )
        .build();
    p.cargo("build").run();

    Setup {
        rustc_wrapper: p.bin("foo"),
        real_sysroot: paths::sysroot(),
    }
}

fn enable_build_std(e: &mut Execs, setup: &Setup) {
    // First up, force Cargo to use our "mock sysroot" which mimics what
    // libstd looks like upstream.
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/testsuite/mock-std/library");
    e.env("__CARGO_TESTS_ONLY_SRC_ROOT", &root);

    e.masquerade_as_nightly_cargo(&["build-std"]);

    // We do various shenanigans to ensure our "mock sysroot" actually links
    // with the real sysroot, so we don't have to actually recompile std for
    // each test. Perform all that logic here, namely:
    //
    // * RUSTC_WRAPPER - uses our shim executable built above to control rustc
    // * REAL_SYSROOT - used by the shim executable to swap out to the real
    //   sysroot temporarily for some compilations
    // * RUST{,DOC}FLAGS - an extra `-L` argument to ensure we can always load
    //   crates from the sysroot, but only indirectly through other crates.
    e.env("RUSTC_WRAPPER", &setup.rustc_wrapper);
    e.env("REAL_SYSROOT", &setup.real_sysroot);
    let libdir = format!("/lib/rustlib/{}/lib", rustc_host());
    e.env(
        "RUSTFLAGS",
        format!("-Ldependency={}{}", setup.real_sysroot, libdir),
    );
    e.env(
        "RUSTDOCFLAGS",
        format!("-Ldependency={}{}", setup.real_sysroot, libdir),
    );
}

// Helper methods used in the tests below
trait BuildStd: Sized {
    fn build_std(&mut self, setup: &Setup) -> &mut Self;
    fn build_std_arg(&mut self, setup: &Setup, arg: &str) -> &mut Self;
    fn target_host(&mut self) -> &mut Self;
}

impl BuildStd for Execs {
    fn build_std(&mut self, setup: &Setup) -> &mut Self {
        enable_build_std(self, setup);
        self.arg("-Zbuild-std");
        self
    }

    fn build_std_arg(&mut self, setup: &Setup, arg: &str) -> &mut Self {
        enable_build_std(self, setup);
        self.arg(format!("-Zbuild-std={}", arg));
        self
    }

    fn target_host(&mut self) -> &mut Self {
        self.arg("--target").arg(rustc_host());
        self
    }
}

#[cargo_test(build_std_mock)]
fn basic() {
    let setup = setup();

    let p = project()
        .file(
            "src/main.rs",
            "
                fn main() {
                    std::custom_api();
                    foo::f();
                }

                #[test]
                fn smoke_bin_unit() {
                    std::custom_api();
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
                    core::custom_api();
                    std::custom_api();
                    alloc::custom_api();
                    proc_macro::custom_api();
                }

                #[test]
                fn smoke_lib_unit() {
                    std::custom_api();
                    f();
                }
            ",
        )
        .file(
            "tests/smoke.rs",
            "
                #[test]
                fn smoke_integration() {
                    std::custom_api();
                    foo::f();
                }
            ",
        )
        .build();

    p.cargo("check -v").build_std(&setup).target_host().run();
    p.cargo("build").build_std(&setup).target_host().run();
    p.cargo("run").build_std(&setup).target_host().run();
    p.cargo("test").build_std(&setup).target_host().run();
}

#[cargo_test(build_std_mock)]
fn shared_std_dependency_rebuild() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let setup = setup();
    let p = project()
        .file(
            "Cargo.toml",
            format!(
                "
                [package]
                name = \"foo\"
                version = \"0.1.0\"
                edition = \"2021\"

                [build-dependencies]
                dep_test = {{ path = \"{}/tests/testsuite/mock-std/dep_test\" }}
            ",
                manifest_dir.replace('\\', "/")
            )
            .as_str(),
        )
        .file(
            "src/main.rs",
            r#"
            fn main() {
                println!("Hello, World!");
            }
            "#,
        )
        .file(
            "build.rs",
            r#"
            fn main() {
                println!("cargo::rerun-if-changed=build.rs");
            }
            "#,
        )
        .build();

    p.cargo("build -v")
        .build_std(&setup)
        .target_host()
        .with_stderr_data(str![[r#"
...
[RUNNING] `[..] rustc --crate-name dep_test [..]`
...
[RUNNING] `[..] rustc --crate-name dep_test [..]`
...
"#]])
        .run();

    p.cargo("build -v")
        .build_std(&setup)
        .with_stderr_does_not_contain(str![[r#"
    ...
    [RUNNING] `[..] rustc --crate-name dep_test [..]`
    ...
    [RUNNING] `[..] rustc --crate-name dep_test [..]`
    ...
    "#]])
        .run();
}

#[cargo_test(build_std_mock)]
fn simple_lib_std() {
    let setup = setup();

    let p = project().file("src/lib.rs", "").build();
    p.cargo("build -v")
        .build_std(&setup)
        .target_host()
        .with_stderr_data(str![[r#"
...
[RUNNING] `[..] rustc --crate-name std [..]`
...
"#]])
        .run();
    // Check freshness.
    p.change_file("src/lib.rs", " ");
    p.cargo("build -v")
        .build_std(&setup)
        .target_host()
        .with_stderr_data(str![[r#"
...
[FRESH] std v0.1.0 ([..]/tests/testsuite/mock-std/library/std)
...
"#]])
        .run();
}

#[cargo_test(build_std_mock)]
fn simple_bin_std() {
    let setup = setup();

    let p = project().file("src/main.rs", "fn main() {}").build();
    p.cargo("run -v").build_std(&setup).target_host().run();
}

#[cargo_test(build_std_mock)]
fn lib_nostd() {
    let setup = setup();

    let p = project()
        .file(
            "src/lib.rs",
            r#"
                #![no_std]
                pub fn foo() {
                    assert_eq!(u8::MIN, 0);
                }
            "#,
        )
        .build();
    p.cargo("build -v --lib")
        .build_std_arg(&setup, "core")
        .target_host()
        .with_stderr_does_not_contain("[..]libstd[..]")
        .run();
}

#[cargo_test(build_std_mock)]
fn check_core() {
    let setup = setup();

    let p = project()
        .file("src/lib.rs", "#![no_std] fn unused_fn() {}")
        .build();

    p.cargo("check -v")
        .build_std_arg(&setup, "core")
        .target_host()
        .with_stderr_data(str![[r#"
...
[WARNING] function `unused_fn` is never used
...
"#]])
        .run();
}

#[cargo_test(build_std_mock)]
fn build_std_with_no_arg_for_core_only_target() {
    let target = "aarch64-unknown-none";
    if !cross_compile::requires_target_installed(target) {
        return;
    }

    let setup = setup();

    let p = project()
        .file(
            "src/lib.rs",
            r#"
                #![no_std]
                pub fn foo() {
                    assert_eq!(u8::MIN, 0);
                }
            "#,
        )
        .build();

    p.cargo("build -v")
        .arg("--target")
        .arg(target)
        .build_std(&setup)
        .with_stderr_data(
            str![[r#"
[UPDATING] `dummy-registry` index
[DOWNLOADING] crates ...
[DOWNLOADED] registry-dep-using-std v1.0.0 (registry `dummy-registry`)
[DOWNLOADED] registry-dep-using-core v1.0.0 (registry `dummy-registry`)
[DOWNLOADED] registry-dep-using-alloc v1.0.0 (registry `dummy-registry`)
[COMPILING] compiler_builtins v0.1.0 ([..]/library/compiler_builtins)
[COMPILING] core v0.1.0 ([..]/library/core)
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `[..] rustc --crate-name compiler_builtins [..]--target aarch64-unknown-none[..]`
[RUNNING] `[..] rustc --crate-name core [..]--target aarch64-unknown-none[..]`
[RUNNING] `[..] rustc --crate-name foo [..]--target aarch64-unknown-none[..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]
            .unordered(),
        )
        .run();

    p.cargo("clean").run();

    // Also work for a mix of std and core-only targets,
    // though not sure how common it is...
    //
    // Note that we don't  download std dependencies for the second call
    // because `-Zbuild-std` downloads them all also when building for core only.
    p.cargo("build -v")
        .arg("--target")
        .arg(target)
        .target_host()
        .build_std(&setup)
        .with_stderr_data(
            str![[r#"
[UPDATING] `dummy-registry` index
[COMPILING] core v0.1.0 ([..]/library/core)
[COMPILING] dep_test v0.1.0 ([..]/dep_test)
[COMPILING] compiler_builtins v0.1.0 ([..]/library/compiler_builtins)
[COMPILING] proc_macro v0.1.0 ([..]/library/proc_macro)
[COMPILING] panic_unwind v0.1.0 ([..]/library/panic_unwind)
[COMPILING] rustc-std-workspace-core v1.9.0 ([..]/library/rustc-std-workspace-core)
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[COMPILING] registry-dep-using-core v1.0.0
[COMPILING] alloc v0.1.0 ([..]/library/alloc)
[COMPILING] rustc-std-workspace-alloc v1.9.0 ([..]/library/rustc-std-workspace-alloc)
[COMPILING] registry-dep-using-alloc v1.0.0
[COMPILING] std v0.1.0 ([..]/library/std)
[RUNNING] `[..]rustc --crate-name compiler_builtins [..]--target aarch64-unknown-none[..]`
[RUNNING] `[..]rustc --crate-name core [..]--target aarch64-unknown-none[..]`
[RUNNING] `[..]rustc --crate-name foo [..]--target aarch64-unknown-none[..]`
[RUNNING] `[..]rustc --crate-name core [..]--target [HOST_TARGET][..]`
[RUNNING] `[..]rustc --crate-name dep_test [..]--target [HOST_TARGET][..]`
[RUNNING] `[..]rustc --crate-name proc_macro [..]--target [HOST_TARGET][..]`
[RUNNING] `[..]rustc --crate-name panic_unwind [..]--target [HOST_TARGET][..]`
[RUNNING] `[..]rustc --crate-name compiler_builtins [..]--target [HOST_TARGET][..]`
[RUNNING] `[..]rustc --crate-name rustc_std_workspace_core [..]--target [HOST_TARGET][..]`
[RUNNING] `[..]rustc --crate-name registry_dep_using_core [..]--target [HOST_TARGET][..]`
[RUNNING] `[..]rustc --crate-name alloc [..]--target [HOST_TARGET][..]`
[RUNNING] `[..]rustc --crate-name rustc_std_workspace_alloc [..]--target [HOST_TARGET][..]`
[RUNNING] `[..]rustc --crate-name registry_dep_using_alloc [..]--target [HOST_TARGET][..]`
[RUNNING] `[..]rustc --crate-name std [..]--target [HOST_TARGET][..]`
[RUNNING] `[..]rustc --crate-name foo [..]--target [HOST_TARGET][..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test(build_std_mock)]
fn depend_same_as_std() {
    let setup = setup();

    let p = project()
        .file(
            "src/lib.rs",
            r#"
                pub fn f() {
                    registry_dep_using_core::non_sysroot_api();
                    registry_dep_using_alloc::non_sysroot_api();
                    registry_dep_using_std::non_sysroot_api();
                }
            "#,
        )
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2018"

                [dependencies]
                registry-dep-using-core = "1.0"
                registry-dep-using-alloc = "1.0"
                registry-dep-using-std = "1.0"
            "#,
        )
        .build();

    p.cargo("build -v").build_std(&setup).target_host().run();
}

#[cargo_test(build_std_mock)]
fn test() {
    let setup = setup();

    let p = project()
        .file(
            "src/lib.rs",
            r#"
                #[cfg(test)]
                mod tests {
                    #[test]
                    fn it_works() {
                        assert_eq!(2 + 2, 4);
                    }
                }
            "#,
        )
        .build();

    p.cargo("test -v")
        .build_std(&setup)
        .target_host()
        .with_stdout_data(str![[r#"

running 1 test
test tests::it_works ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in [ELAPSED]s
...
"#]])
        .run();
}

#[cargo_test(build_std_mock)]
fn target_proc_macro() {
    let setup = setup();

    let p = project()
        .file(
            "src/lib.rs",
            r#"
                extern crate proc_macro;
                pub fn f() {
                    let _ts = proc_macro::TokenStream::new();
                }
            "#,
        )
        .build();

    p.cargo("build -v").build_std(&setup).target_host().run();
}

#[cargo_test(build_std_mock)]
fn bench() {
    let setup = setup();

    let p = project()
        .file(
            "src/lib.rs",
            r#"
                #![feature(test)]
                extern crate test;

                #[bench]
                fn b1(b: &mut test::Bencher) {
                    b.iter(|| ())
                }
            "#,
        )
        .build();

    p.cargo("bench -v").build_std(&setup).target_host().run();
}

#[cargo_test(build_std_mock)]
fn doc() {
    let setup = setup();

    let p = project()
        .file(
            "src/lib.rs",
            r#"
                /// Doc
                pub fn f() -> Result<(), ()> {Ok(())}
            "#,
        )
        .build();

    p.cargo("doc -v").build_std(&setup).target_host().run();
}

#[cargo_test(build_std_mock)]
fn check_std() {
    let setup = setup();

    let p = project()
        .file(
            "src/lib.rs",
            "
                extern crate core;
                extern crate alloc;
                extern crate proc_macro;
                pub fn f() {}
            ",
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "tests/t1.rs",
            r#"
                #[test]
                fn t1() {
                    assert_eq!(1, 2);
                }
            "#,
        )
        .build();

    p.cargo("check -v --all-targets")
        .build_std(&setup)
        .target_host()
        .run();
    p.cargo("check -v --all-targets --profile=test")
        .build_std(&setup)
        .target_host()
        .run();
}

#[cargo_test(build_std_mock)]
fn doctest() {
    let setup = setup();

    let p = project()
        .file(
            "src/lib.rs",
            r#"
                /// Doc
                /// ```
                /// std::custom_api();
                /// ```
                pub fn f() {}
            "#,
        )
        .build();

    p.cargo("test --doc -v")
        .build_std(&setup)
        .with_stdout_data(str![[r#"

running 1 test
test src/lib.rs - f (line 3) ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in [ELAPSED]s


"#]])
        .target_host()
        .run();
}

#[cargo_test(build_std_mock)]
fn no_implicit_alloc() {
    // Demonstrate that alloc is not implicitly in scope.
    let setup = setup();

    let p = project()
        .file(
            "src/lib.rs",
            r#"
                pub fn f() {
                    let _: Vec<i32> = alloc::vec::Vec::new();
                }
            "#,
        )
        .build();

    p.cargo("build -v")
        .build_std(&setup)
        .target_host()
        .with_stderr_data(str![[r#"
...
error[E0433]: failed to resolve[..]`alloc`
...
"#]])
        .with_status(101)
        .run();
}

#[cargo_test(build_std_mock)]
fn macro_expanded_shadow() {
    // This tests a bug caused by the previous use of `--extern` to directly
    // load sysroot crates. This necessitated the switch to `--sysroot` to
    // retain existing behavior. See
    // https://github.com/rust-lang/wg-cargo-std-aware/issues/40 for more
    // detail.
    let setup = setup();

    let p = project()
        .file(
            "src/lib.rs",
            r#"
                macro_rules! a {
                    () => (extern crate std as alloc;)
                }
                a!();
            "#,
        )
        .build();

    p.cargo("build -v").build_std(&setup).target_host().run();
}

#[cargo_test(build_std_mock)]
fn ignores_incremental() {
    // Incremental is not really needed for std, make sure it is disabled.
    // Incremental also tends to have bugs that affect std libraries more than
    // any other crate.
    let setup = setup();

    let p = project().file("src/lib.rs", "").build();
    p.cargo("build")
        .env("CARGO_INCREMENTAL", "1")
        .build_std(&setup)
        .target_host()
        .run();
    let incremental: Vec<_> = p
        .glob(format!("target/{}/debug/incremental/*", rustc_host()))
        .map(|e| e.unwrap())
        .collect();
    assert_eq!(incremental.len(), 1);
    assert!(
        incremental[0]
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("foo-")
    );
}

#[cargo_test(build_std_mock)]
fn cargo_config_injects_compiler_builtins() {
    let setup = setup();

    let p = project()
        .file(
            "src/lib.rs",
            r#"
                #![no_std]
                pub fn foo() {
                    assert_eq!(u8::MIN, 0);
                }
            "#,
        )
        .file(
            ".cargo/config.toml",
            r#"
                [unstable]
                build-std = ['core']
            "#,
        )
        .build();
    let mut build = p.cargo("build -v --lib");
    enable_build_std(&mut build, &setup);
    build
        .target_host()
        .with_stderr_does_not_contain("[..]libstd[..]")
        .run();
}

#[cargo_test(build_std_mock)]
fn different_features() {
    let setup = setup();

    let p = project()
        .file(
            "src/lib.rs",
            "
                pub fn foo() {
                    std::conditional_function();
                }
            ",
        )
        .build();
    p.cargo("build")
        .build_std(&setup)
        .arg("-Zbuild-std-features=feature1")
        .target_host()
        .run();
}

#[cargo_test(build_std_mock)]
fn no_roots() {
    // Checks for a bug where it would panic if there are no roots.
    let setup = setup();

    let p = project().file("tests/t1.rs", "").build();
    p.cargo("build")
        .build_std(&setup)
        .target_host()
        .with_stderr_data(str![[r#"
...
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test(build_std_mock)]
fn proc_macro_only() {
    // Checks for a bug where it would panic if building a proc-macro only
    let setup = setup();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "pm"
                version = "0.1.0"

                [lib]
                proc-macro = true
            "#,
        )
        .file("src/lib.rs", "")
        .build();
    p.cargo("build")
        .build_std(&setup)
        .target_host()
        .with_stderr_data(str![[r#"
...
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test(build_std_mock)]
fn fetch() {
    let setup = setup();

    let p = project().file("src/main.rs", "fn main() {}").build();
    p.cargo("fetch")
        .build_std(&setup)
        .target_host()
        .with_stderr_contains("[DOWNLOADED] [..]")
        .run();
    p.cargo("build")
        .build_std(&setup)
        .target_host()
        .with_stderr_does_not_contain("[DOWNLOADED] [..]")
        .run();
}
