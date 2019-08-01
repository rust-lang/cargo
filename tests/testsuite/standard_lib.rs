use crate::support::{is_nightly, paths, project, rustc_host, Execs, Project};

fn cargo_build_std(project: &Project, cmd: &str, crates: &str) -> Execs {
    let unstable = if crates.is_empty() {
        "-Zbuild-std".to_string()
    } else {
        format!("-Zbuild-std={}", crates)
    };
    let target = paths::root().join("target");
    let mut execs = project.cargo(cmd);
    if !cmd.contains("--target") {
        execs.arg("--target").arg(rustc_host());
    }
    execs
        .arg(unstable)
        .arg("-Zno-index-update")
        .env_remove("CARGO_HOME")
        .env_remove("HOME")
        .env("CARGO_TARGET_DIR", target.as_os_str())
        .masquerade_as_nightly_cargo();
    execs
}

#[cargo_test]
fn std_lib() {
    if !is_nightly() {
        // -Zbuild-std is nightly
        // -Zno-index-update is nightly
        // We don't want these tests to run on rust-lang/rust.
        return;
    }
    simple_lib_std();
    simple_bin_std();
    lib_nostd();
    bin_nostd();
    check_core();
    cross_custom();
    hashbrown();
    libc();
    test();
    target_proc_macro();
    bench();
    doc();
    check_std();
    doctest();
}

fn simple_lib_std() {
    let p = project().file("src/lib.rs", "").build();
    cargo_build_std(&p, "build -v", "")
        .with_stderr_contains("[RUNNING] `rustc [..]--crate-name std [..]")
        .run();
    // Check freshness.
    p.change_file("src/lib.rs", " ");
    cargo_build_std(&p, "build -v", "std")
        .with_stderr_contains("[FRESH] std[..]")
        .run();
}

fn simple_bin_std() {
    let p = project().file("src/main.rs", "fn main() {}").build();
    cargo_build_std(&p, "run -v", "std").run();
}

fn lib_nostd() {
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
    cargo_build_std(&p, "build -v --lib", "core")
        .with_stderr_does_not_contain("[..]libstd[..]")
        .run();
}

fn bin_nostd() {
    if cfg!(windows) {
        // I think windows requires setting up mainCRTStartup,
        // I'm not in the mood to figure it out.
        return;
    }
    let p = project()
        .file("src/lib.rs", "#![no_std] pub fn foo() {}")
        .file(
            "src/main.rs",
            r#"
            #![no_std]
            #![feature(lang_items, start, core_intrinsics)]

            use core::panic::PanicInfo;

            #[panic_handler]
            fn panic(_info: &PanicInfo) -> ! {
                unsafe { core::intrinsics::abort() }
            }

            #[start]
            fn start(_argc: isize, _argv: *const *const u8) -> isize {
                foo::foo();
                123
            }

            #[lang = "eh_personality"]
            extern fn eh_personality() {}
            "#,
        )
        .file(
            "build.rs",
            r#"
            fn main() {
                let target = std::env::var("TARGET").expect("TARGET was not set");
                if target.contains("apple-darwin") {
                    println!("cargo:rustc-link-lib=System");
                } else if target.contains("linux") {
                    // TODO: why is this needed?
                    println!("cargo:rustc-link-lib=c");
                }
            }
            "#,
        )
        .build();

    cargo_build_std(&p, "run -v", "core")
        .with_status(123)
        .with_stderr_contains("[RUNNING] [..]foo[EXE]`")
        .run();
}

fn check_core() {
    let p = project()
        .file("src/lib.rs", "#![no_std] fn unused_fn() {}")
        .build();

    cargo_build_std(&p, "check -v", "core")
        .with_stderr_contains("[WARNING] [..]unused_fn[..]`")
        .run();
}

fn cross_custom() {
    let p = project()
        .file("src/lib.rs", "#![no_std] pub fn f() {}")
        .file(
            "custom-target.json",
            r#"
            {
                "llvm-target": "x86_64-unknown-none-gnu",
                "data-layout": "e-m:e-i64:64-f80:128-n8:16:32:64-S128",
                "arch": "x86_64",
                "target-endian": "little",
                "target-pointer-width": "64",
                "target-c-int-width": "32",
                "os": "none",
                "linker-flavor": "ld.lld"
            }
            "#,
        )
        .build();

    cargo_build_std(&p, "build --target custom-target.json -v", "core").run();
}

fn hashbrown() {
    let p = project()
        .file(
            "src/lib.rs",
            r#"
            pub fn f() -> hashbrown::HashMap<i32, i32> {
                hashbrown::HashMap::new()
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
            hashbrown = "=0.4.0"
            "#,
        )
        .build();

    cargo_build_std(&p, "build -v", "std").run();
}

fn libc() {
    let p = project()
        .file(
            "src/lib.rs",
            r#"
            pub fn f() -> ! {
                unsafe { libc::exit(123); }
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
            libc = "=0.2.54"
            "#,
        )
        .build();

    cargo_build_std(&p, "build -v", "std").run();
}

fn test() {
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

    cargo_build_std(&p, "test -v", "std")
        .with_stdout_contains("test tests::it_works ... ok")
        .run();
}

fn target_proc_macro() {
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

    cargo_build_std(&p, "build -v", "std,proc_macro").run();
}

fn bench() {
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

    cargo_build_std(&p, "bench -v", "std").run();
}

fn doc() {
    let p = project()
        .file(
            "src/lib.rs",
            r#"
            /// Doc
            pub fn f() -> Result<(), ()> {Ok(())}
            "#,
        )
        .build();

    cargo_build_std(&p, "doc -v", "std").run();
}

fn check_std() {
    let p = project()
        .file("src/lib.rs", "pub fn f() {}")
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

    cargo_build_std(&p, "check -v --all-targets", "std").run();
    cargo_build_std(&p, "check -v --all-targets --profile=test", "std").run();
}

fn doctest() {
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

    cargo_build_std(&p, "test --doc -v", "std").run();
}
