use cargo_test_support::registry::{Dependency, Package};
use cargo_test_support::{is_nightly, paths, project, rustc_host, Execs};

fn setup() -> bool {
    if !is_nightly() {
        // -Zbuild-std is nightly
        // We don't want these tests to run on rust-lang/rust.
        return false;
    }

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
                    core::custom_api();
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
                    core::custom_api();
                    alloc::custom_api();
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
                    std::custom_api();
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
    return true;
}

fn enable_build_std(e: &mut Execs, arg: Option<&str>) {
    // First up, force Cargo to use our "mock sysroot" which mimics what
    // libstd looks like upstream.
    let root = paths::root();
    let root = root
        .parent() // chop off test name
        .unwrap()
        .parent() // chop off `citN`
        .unwrap()
        .parent() // chop off `target`
        .unwrap()
        .join("tests/testsuite/mock-std");
    e.env("__CARGO_TESTS_ONLY_SRC_ROOT", &root);

    // Next, make sure it doesn't have implicit access to the host's sysroot
    e.env("RUSTFLAGS", "--sysroot=/path/to/nowhere");

    // And finally actually enable `build-std` for now
    let arg = match arg {
        Some(s) => format!("-Zbuild-std={}", s),
        None => "-Zbuild-std".to_string(),
    };
    e.arg(arg);
    e.masquerade_as_nightly_cargo();
}

// Helper methods used in the tests below
trait BuildStd: Sized {
    fn build_std(&mut self) -> &mut Self;
    fn build_std_arg(&mut self, arg: &str) -> &mut Self;
    fn target_host(&mut self) -> &mut Self;
}

impl BuildStd for Execs {
    fn build_std(&mut self) -> &mut Self {
        enable_build_std(self, None);
        self
    }

    fn build_std_arg(&mut self, arg: &str) -> &mut Self {
        enable_build_std(self, Some(arg));
        self
    }

    fn target_host(&mut self) -> &mut Self {
        self.arg("--target").arg(rustc_host());
        self
    }
}

#[cargo_test]
fn basic() {
    if !setup() {
        return;
    }

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

    p.cargo("check").build_std().target_host().run();
    p.cargo("build").build_std().target_host().run();
    p.cargo("run").build_std().target_host().run();
    p.cargo("test").build_std().target_host().run();
}

#[cargo_test]
fn simple_lib_std() {
    if !setup() {
        return;
    }
    let p = project().file("src/lib.rs", "").build();
    p.cargo("build -v")
        .build_std()
        .target_host()
        .with_stderr_contains("[RUNNING] `rustc [..]--crate-name std [..]")
        .run();
    // Check freshness.
    p.change_file("src/lib.rs", " ");
    p.cargo("build -v")
        .build_std()
        .target_host()
        .with_stderr_contains("[FRESH] std[..]")
        .run();
}

#[cargo_test]
fn simple_bin_std() {
    if !setup() {
        return;
    }
    let p = project().file("src/main.rs", "fn main() {}").build();
    p.cargo("run -v").build_std().target_host().run();
}

#[cargo_test]
fn lib_nostd() {
    if !setup() {
        return;
    }
    let p = project()
        .file(
            "src/lib.rs",
            r#"
                #![no_std]
                pub fn foo() {
                    assert_eq!(core::u8::MIN, 0);
                }
            "#,
        )
        .build();
    p.cargo("build -v --lib")
        .build_std_arg("core")
        .target_host()
        .with_stderr_does_not_contain("[..]libstd[..]")
        .run();
}

#[cargo_test]
fn check_core() {
    if !setup() {
        return;
    }
    let p = project()
        .file("src/lib.rs", "#![no_std] fn unused_fn() {}")
        .build();

    p.cargo("check -v")
        .build_std_arg("core")
        .target_host()
        .with_stderr_contains("[WARNING] [..]unused_fn[..]`")
        .run();
}

#[cargo_test]
fn depend_same_as_std() {
    if !setup() {
        return;
    }

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

    p.cargo("build -v").build_std().target_host().run();
}

#[cargo_test]
fn test() {
    if !setup() {
        return;
    }
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
        .build_std()
        .target_host()
        .with_stdout_contains("test tests::it_works ... ok")
        .run();
}

#[cargo_test]
fn target_proc_macro() {
    if !setup() {
        return;
    }
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

    p.cargo("build -v").build_std().target_host().run();
}

#[cargo_test]
fn bench() {
    if !setup() {
        return;
    }
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

    p.cargo("bench -v").build_std().target_host().run();
}

#[cargo_test]
fn doc() {
    if !setup() {
        return;
    }
    let p = project()
        .file(
            "src/lib.rs",
            r#"
                /// Doc
                pub fn f() -> Result<(), ()> {Ok(())}
            "#,
        )
        .build();

    p.cargo("doc -v").build_std().target_host().run();
}

#[cargo_test]
fn check_std() {
    if !setup() {
        return;
    }
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
        .build_std()
        .target_host()
        .run();
    p.cargo("check -v --all-targets --profile=test")
        .build_std()
        .target_host()
        .run();
}

#[cargo_test]
fn doctest() {
    if !setup() {
        return;
    }
    let p = project()
        .file(
            "src/lib.rs",
            r#"
                /// Doc
                /// ```
                /// assert_eq!(1, 1);
                /// ```
                pub fn f() {}
            "#,
        )
        .build();

    p.cargo("test --doc -v").build_std().target_host().run();
}
