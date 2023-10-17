use cargo::core::compiler::Lto;
use cargo_test_support::registry::Package;
use cargo_test_support::{basic_manifest, project, Project};
use std::process::Output;

#[cargo_test]
fn with_deps() {
    Package::new("bar", "0.0.1").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "test"
                version = "0.0.0"

                [dependencies]
                bar = "*"

                [profile.release]
                lto = true
            "#,
        )
        .file("src/main.rs", "extern crate bar; fn main() {}")
        .build();
    p.cargo("build -v --release")
        .with_stderr_contains("[..]`rustc[..]--crate-name bar[..]-C linker-plugin-lto[..]`")
        .with_stderr_contains("[..]`rustc[..]--crate-name test[..]-C lto[..]`")
        .run();
}

#[cargo_test]
fn shared_deps() {
    Package::new("bar", "0.0.1").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "test"
                version = "0.0.0"

                [dependencies]
                bar = "*"

                [build-dependencies]
                bar = "*"

                [profile.release]
                lto = true
            "#,
        )
        .file("build.rs", "extern crate bar; fn main() {}")
        .file("src/main.rs", "extern crate bar; fn main() {}")
        .build();
    p.cargo("build -v --release")
        .with_stderr_contains("[..]`rustc[..]--crate-name test[..]-C lto[..]`")
        .run();
}

#[cargo_test]
fn build_dep_not_ltod() {
    Package::new("bar", "0.0.1").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "test"
                version = "0.0.0"

                [build-dependencies]
                bar = "*"

                [profile.release]
                lto = true
            "#,
        )
        .file("build.rs", "extern crate bar; fn main() {}")
        .file("src/main.rs", "fn main() {}")
        .build();
    p.cargo("build -v --release")
        .with_stderr_contains("[..]`rustc[..]--crate-name bar[..]-C embed-bitcode=no[..]`")
        .with_stderr_contains("[..]`rustc[..]--crate-name test[..]-C lto[..]`")
        .run();
}

#[cargo_test]
fn complicated() {
    Package::new("dep-shared", "0.0.1")
        .file("src/lib.rs", "pub fn foo() {}")
        .publish();
    Package::new("dep-normal2", "0.0.1")
        .file("src/lib.rs", "pub fn foo() {}")
        .publish();
    Package::new("dep-normal", "0.0.1")
        .dep("dep-shared", "*")
        .dep("dep-normal2", "*")
        .file(
            "src/lib.rs",
            "
                pub fn foo() {
                    dep_shared::foo();
                    dep_normal2::foo();
                }
            ",
        )
        .publish();
    Package::new("dep-build2", "0.0.1")
        .file("src/lib.rs", "pub fn foo() {}")
        .publish();
    Package::new("dep-build", "0.0.1")
        .dep("dep-shared", "*")
        .dep("dep-build2", "*")
        .file(
            "src/lib.rs",
            "
                pub fn foo() {
                    dep_shared::foo();
                    dep_build2::foo();
                }
            ",
        )
        .publish();
    Package::new("dep-proc-macro2", "0.0.1")
        .file("src/lib.rs", "pub fn foo() {}")
        .publish();
    Package::new("dep-proc-macro", "0.0.1")
        .proc_macro(true)
        .dep("dep-shared", "*")
        .dep("dep-proc-macro2", "*")
        .file(
            "src/lib.rs",
            "
                extern crate proc_macro;
                use proc_macro::TokenStream;

                #[proc_macro_attribute]
                pub fn foo(_: TokenStream, a: TokenStream) -> TokenStream {
                    dep_shared::foo();
                    dep_proc_macro2::foo();
                    a
                }
            ",
        )
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "test"
                version = "0.0.0"

                [lib]
                crate-type = ['cdylib', 'staticlib']

                [dependencies]
                dep-normal = "*"
                dep-proc-macro = "*"

                [build-dependencies]
                dep-build = "*"

                [profile.release]
                lto = true

                # force build deps to share an opt-level with the rest of the
                # graph so they only get built once.
                [profile.release.build-override]
                opt-level = 3
            "#,
        )
        .file("build.rs", "fn main() { dep_build::foo() }")
        .file(
            "src/bin/foo-bin.rs",
            "#[dep_proc_macro::foo] fn main() { dep_normal::foo() }",
        )
        .file(
            "src/lib.rs",
            "#[dep_proc_macro::foo] pub fn foo() { dep_normal::foo() }",
        )
        .build();
    p.cargo("build -v --release")
        // normal deps and their transitive dependencies do not need object
        // code, so they should have linker-plugin-lto specified
        .with_stderr_contains(
            "[..]`rustc[..]--crate-name dep_normal2 [..]-C linker-plugin-lto[..]`",
        )
        .with_stderr_contains("[..]`rustc[..]--crate-name dep_normal [..]-C linker-plugin-lto[..]`")
        // build dependencies and their transitive deps don't need any bitcode,
        // so embedding should be turned off
        .with_stderr_contains("[..]`rustc[..]--crate-name dep_build2 [..]-C embed-bitcode=no[..]`")
        .with_stderr_contains("[..]`rustc[..]--crate-name dep_build [..]-C embed-bitcode=no[..]`")
        .with_stderr_contains(
            "[..]`rustc[..]--crate-name build_script_build [..]-C embed-bitcode=no[..]`",
        )
        // proc macro deps are the same as build deps here
        .with_stderr_contains(
            "[..]`rustc[..]--crate-name dep_proc_macro2 [..]-C embed-bitcode=no[..]`",
        )
        .with_stderr_contains(
            "[..]`rustc[..]--crate-name dep_proc_macro [..]-C embed-bitcode=no[..]`",
        )
        .with_stderr_contains(
            "[..]`rustc[..]--crate-name foo_bin [..]--crate-type bin[..]-C lto[..]`",
        )
        .with_stderr_contains(
            "[..]`rustc[..]--crate-name test [..]--crate-type cdylib[..]-C lto[..]`",
        )
        .with_stderr_contains("[..]`rustc[..]--crate-name dep_shared [..]`")
        .with_stderr_does_not_contain("[..]--crate-name dep_shared[..]-C lto[..]")
        .with_stderr_does_not_contain("[..]--crate-name dep_shared[..]-C linker-plugin-lto[..]")
        .with_stderr_does_not_contain("[..]--crate-name dep_shared[..]-C embed-bitcode[..]")
        .run();
}

#[cargo_test]
fn off_in_manifest_works() {
    Package::new("bar", "0.0.1")
        .file("src/lib.rs", "pub fn foo() {}")
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "test"
                version = "0.0.0"

                [dependencies]
                bar = "*"

                [profile.release]
                lto = "off"
            "#,
        )
        .file("src/lib.rs", "pub fn foo() {}")
        .file(
            "src/main.rs",
            "fn main() {
            test::foo();
            bar::foo();
        }",
        )
        .build();
    p.cargo("build -v --release")
        .with_stderr(
            "\
[UPDATING] [..]
[DOWNLOADING] [..]
[DOWNLOADED] [..]
[COMPILING] bar v0.0.1
[RUNNING] `rustc --crate-name bar [..]--crate-type lib [..]-C lto=off -C embed-bitcode=no[..]
[COMPILING] test [..]
[RUNNING] `rustc --crate-name test [..]--crate-type lib [..]-C lto=off -C embed-bitcode=no[..]
[RUNNING] `rustc --crate-name test src/main.rs [..]--crate-type bin [..]-C lto=off[..]
[FINISHED] [..]
",
        )
        .run();
}

#[cargo_test]
fn between_builds() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "test"
                version = "0.0.0"

                [profile.release]
                lto = true
            "#,
        )
        .file("src/lib.rs", "pub fn foo() {}")
        .file("src/main.rs", "fn main() { test::foo() }")
        .build();
    p.cargo("build -v --release --lib")
        .with_stderr(
            "\
[COMPILING] test [..]
[RUNNING] `rustc [..]--crate-type lib[..]-C linker-plugin-lto[..]
[FINISHED] [..]
",
        )
        .run();
    p.cargo("build -v --release")
        .with_stderr_contains(
            "\
[COMPILING] test [..]
[RUNNING] `rustc [..]--crate-type bin[..]-C lto[..]
[FINISHED] [..]
",
        )
        .run();
}

#[cargo_test]
fn test_all() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.0"

                [profile.release]
                lto = true
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("tests/a.rs", "")
        .file("tests/b.rs", "")
        .build();
    p.cargo("test --release -v")
        .with_stderr_contains("[RUNNING] `rustc[..]--crate-name foo[..]-C lto[..]")
        .run();
}

#[cargo_test]
fn test_all_and_bench() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.0"

                [profile.release]
                lto = true
                [profile.bench]
                lto = true
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("tests/a.rs", "")
        .file("tests/b.rs", "")
        .build();
    p.cargo("test --release -v")
        .with_stderr_contains("[RUNNING] `rustc[..]--crate-name a[..]-C lto[..]")
        .with_stderr_contains("[RUNNING] `rustc[..]--crate-name b[..]-C lto[..]")
        .with_stderr_contains("[RUNNING] `rustc[..]--crate-name foo[..]-C lto[..]")
        .run();
}

/// Basic setup:
///
/// foo v0.0.0
/// ├── bar v0.0.0
/// │   ├── registry v0.0.1
/// │   └── registry-shared v0.0.1
/// └── registry-shared v0.0.1
///
/// Where `bar` will have the given crate types.
fn project_with_dep(crate_types: &str) -> Project {
    Package::new("registry", "0.0.1")
        .file("src/lib.rs", r#"pub fn foo() { println!("registry"); }"#)
        .publish();
    Package::new("registry-shared", "0.0.1")
        .file("src/lib.rs", r#"pub fn foo() { println!("shared"); }"#)
        .publish();

    project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.0"

                [workspace]

                [dependencies]
                bar = { path = 'bar' }
                registry-shared = "*"

                [profile.release]
                lto = true
            "#,
        )
        .file(
            "src/main.rs",
            "
                fn main() {
                    bar::foo();
                    registry_shared::foo();
                }
            ",
        )
        .file(
            "bar/Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "bar"
                    version = "0.0.0"

                    [dependencies]
                    registry = "*"
                    registry-shared = "*"

                    [lib]
                    crate-type = [{}]
                "#,
                crate_types
            ),
        )
        .file(
            "bar/src/lib.rs",
            r#"
                pub fn foo() {
                    println!("bar");
                    registry::foo();
                    registry_shared::foo();
                }
            "#,
        )
        .file("tests/a.rs", "")
        .file("bar/tests/b.rs", "")
        .build()
}

/// Helper for checking which LTO behavior is used for a specific crate.
///
/// `krate_info` is extra compiler flags used to distinguish this if the same
/// crate name is being built multiple times.
fn verify_lto(output: &Output, krate: &str, krate_info: &str, expected_lto: Lto) {
    let stderr = std::str::from_utf8(&output.stderr).unwrap();
    let mut matches = stderr.lines().filter(|line| {
        line.contains("Running")
            && line.contains(&format!("--crate-name {} ", krate))
            && line.contains(krate_info)
    });
    let line = matches.next().unwrap_or_else(|| {
        panic!(
            "expected to find crate `{}` info: `{}`, not found in output:\n{}",
            krate, krate_info, stderr
        );
    });
    if let Some(line2) = matches.next() {
        panic!(
            "found multiple lines matching crate `{}` info: `{}`:\nline1:{}\nline2:{}\noutput:\n{}",
            krate, krate_info, line, line2, stderr
        );
    }
    let actual_lto = if let Some((_, line)) = line.split_once("-C lto=") {
        let mode = line.splitn(2, ' ').next().unwrap();
        if mode == "off" {
            Lto::Off
        } else {
            Lto::Run(Some(mode.into()))
        }
    } else if line.contains("-C lto") {
        Lto::Run(None)
    } else if line.contains("-C linker-plugin-lto") {
        Lto::OnlyBitcode
    } else if line.contains("-C embed-bitcode=no") {
        Lto::OnlyObject
    } else {
        Lto::ObjectAndBitcode
    };
    assert_eq!(
        actual_lto, expected_lto,
        "did not find expected LTO in line: {}",
        line
    );
}

#[cargo_test]
fn cdylib_and_rlib() {
    let p = project_with_dep("'cdylib', 'rlib'");
    let output = p.cargo("build --release -v").exec_with_output().unwrap();
    // `registry` is ObjectAndBitcode because it needs Object for the
    // rlib, and Bitcode for the cdylib (which doesn't support LTO).
    verify_lto(
        &output,
        "registry",
        "--crate-type lib",
        Lto::ObjectAndBitcode,
    );
    // Same as `registry`
    verify_lto(
        &output,
        "registry_shared",
        "--crate-type lib",
        Lto::ObjectAndBitcode,
    );
    // Same as `registry`
    verify_lto(
        &output,
        "bar",
        "--crate-type cdylib --crate-type rlib",
        Lto::ObjectAndBitcode,
    );
    verify_lto(&output, "foo", "--crate-type bin", Lto::Run(None));
    p.cargo("test --release -v")
        .with_stderr_unordered(
            "\
[FRESH] registry v0.0.1
[FRESH] registry-shared v0.0.1
[FRESH] bar v0.0.0 [..]
[COMPILING] foo [..]
[RUNNING] `rustc --crate-name foo [..]-C lto [..]--test[..]
[RUNNING] `rustc --crate-name a [..]-C lto [..]--test[..]
[FINISHED] [..]
[RUNNING] [..]
[RUNNING] [..]
",
        )
        .run();
    p.cargo("build --release -v --manifest-path bar/Cargo.toml")
        .with_stderr_unordered(
            "\
[FRESH] registry-shared v0.0.1
[FRESH] registry v0.0.1
[FRESH] bar v0.0.0 [..]
[FINISHED] [..]
",
        )
        .run();
    p.cargo("test --release -v --manifest-path bar/Cargo.toml")
        .with_stderr_unordered(
            "\
[FRESH] registry-shared v0.0.1
[FRESH] registry v0.0.1
[COMPILING] bar [..]
[RUNNING] `rustc --crate-name bar [..]-C lto[..]--test[..]
[RUNNING] `rustc --crate-name b [..]-C lto[..]--test[..]
[FINISHED] [..]
[RUNNING] [..]target/release/deps/bar-[..]
[RUNNING] [..]target/release/deps/b-[..]
[DOCTEST] bar
[RUNNING] `rustdoc --crate-type cdylib --crate-type rlib --crate-name bar --test [..]-C lto[..]
",
        )
        .run();
}

#[cargo_test]
fn dylib() {
    let p = project_with_dep("'dylib'");
    let output = p.cargo("build --release -v").exec_with_output().unwrap();
    // `registry` is OnlyObject because rustc doesn't support LTO with dylibs.
    verify_lto(&output, "registry", "--crate-type lib", Lto::OnlyObject);
    // `registry_shared` is both because it is needed by both bar (Object) and
    // foo (Bitcode for LTO).
    verify_lto(
        &output,
        "registry_shared",
        "--crate-type lib",
        Lto::ObjectAndBitcode,
    );
    // `bar` is OnlyObject because rustc doesn't support LTO with dylibs.
    verify_lto(&output, "bar", "--crate-type dylib", Lto::OnlyObject);
    // `foo` is LTO because it is a binary, and the profile specifies `lto=true`.
    verify_lto(&output, "foo", "--crate-type bin", Lto::Run(None));
    // `cargo test` should not rebuild dependencies. It builds the test
    // executables with `lto=true` because the tests are built with the
    // `--release` flag.
    p.cargo("test --release -v")
        .with_stderr_unordered(
            "\
[FRESH] registry v0.0.1
[FRESH] registry-shared v0.0.1
[FRESH] bar v0.0.0 [..]
[COMPILING] foo [..]
[RUNNING] `rustc --crate-name foo [..]-C lto [..]--test[..]
[RUNNING] `rustc --crate-name a [..]-C lto [..]--test[..]
[FINISHED] [..]
[RUNNING] [..]
[RUNNING] [..]
",
        )
        .run();
    // Building just `bar` causes `registry-shared` to get rebuilt because it
    // switches to OnlyObject because it is now only being used with a dylib
    // which does not support LTO.
    //
    // `bar` gets rebuilt because `registry_shared` got rebuilt.
    p.cargo("build --release -v --manifest-path bar/Cargo.toml")
        .with_stderr_unordered(
            "\
[COMPILING] registry-shared v0.0.1
[FRESH] registry v0.0.1
[RUNNING] `rustc --crate-name registry_shared [..]-C embed-bitcode=no[..]
[DIRTY] bar v0.0.0 ([..]): dependency info changed
[COMPILING] bar [..]
[RUNNING] `rustc --crate-name bar [..]--crate-type dylib [..]-C embed-bitcode=no[..]
[FINISHED] [..]
",
        )
        .run();
    // Testing just `bar` causes `registry` to get rebuilt because it switches
    // to needing both Object (for the `bar` dylib) and Bitcode (for the test
    // built with LTO).
    //
    // `bar` the dylib gets rebuilt because `registry` got rebuilt.
    p.cargo("test --release -v --manifest-path bar/Cargo.toml")
        .with_stderr_unordered(
            "\
[FRESH] registry-shared v0.0.1
[COMPILING] registry v0.0.1
[RUNNING] `rustc --crate-name registry [..]
[DIRTY] bar v0.0.0 ([..]): dependency info changed
[COMPILING] bar [..]
[RUNNING] `rustc --crate-name bar [..]--crate-type dylib [..]-C embed-bitcode=no[..]
[RUNNING] `rustc --crate-name bar [..]-C lto [..]--test[..]
[RUNNING] `rustc --crate-name b [..]-C lto [..]--test[..]
[FINISHED] [..]
[RUNNING] [..]
[RUNNING] [..]
",
        )
        .run();
}

#[cargo_test]
// This is currently broken on windows-gnu, see https://github.com/rust-lang/rust/issues/109797
#[cfg_attr(
    all(target_os = "windows", target_env = "gnu"),
    ignore = "windows-gnu not working"
)]
fn test_profile() {
    Package::new("bar", "0.0.1")
        .file("src/lib.rs", "pub fn foo() -> i32 { 123 } ")
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2018"

                [profile.test]
                lto = 'thin'

                [dependencies]
                bar = "*"
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
                #[test]
                fn t1() {
                    assert_eq!(123, bar::foo());
                }
            "#,
        )
        .build();

    p.cargo("test -v")
        // unordered because the two `foo` builds start in parallel
        .with_stderr_unordered("\
[UPDATING] [..]
[DOWNLOADING] [..]
[DOWNLOADED] [..]
[COMPILING] bar v0.0.1
[RUNNING] `rustc --crate-name bar [..]crate-type lib[..]
[COMPILING] foo [..]
[RUNNING] `rustc --crate-name foo [..]--crate-type lib --emit=dep-info,metadata,link -C linker-plugin-lto[..]
[RUNNING] `rustc --crate-name foo [..]--emit=dep-info,link -C lto=thin [..]--test[..]
[FINISHED] [..]
[RUNNING] [..]
[DOCTEST] foo
[RUNNING] `rustdoc [..]
")
        .run();
}

#[cargo_test]
fn doctest() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2018"

                [profile.release]
                lto = true

                [dependencies]
                bar = { path = "bar" }
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
                /// Foo!
                ///
                /// ```
                /// foo::foo();
                /// ```
                pub fn foo() { bar::bar(); }
            "#,
        )
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file(
            "bar/src/lib.rs",
            r#"
                pub fn bar() { println!("hi!"); }
            "#,
        )
        .build();

    p.cargo("test --doc --release -v")
        .with_stderr_contains("[..]`rustc --crate-name bar[..]-C linker-plugin-lto[..]")
        .with_stderr_contains("[..]`rustc --crate-name foo[..]-C linker-plugin-lto[..]")
        // embed-bitcode should be harmless here
        .with_stderr_contains("[..]`rustdoc [..]-C lto[..]")
        .run();

    // Try with bench profile.
    p.cargo("test --doc --release -v")
        .env("CARGO_PROFILE_BENCH_LTO", "true")
        .with_stderr_unordered(
            "\
[FRESH] bar v0.1.0 [..]
[FRESH] foo v0.1.0 [..]
[FINISHED] release [..]
[DOCTEST] foo
[RUNNING] `rustdoc [..]-C lto[..]
",
        )
        .run();
}

#[cargo_test]
fn dylib_rlib_bin() {
    // dylib+rlib linked with a binary
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [lib]
                crate-type = ["dylib", "rlib"]

                [profile.release]
                lto = true
            "#,
        )
        .file("src/lib.rs", "pub fn foo() { println!(\"hi!\"); }")
        .file("src/bin/ferret.rs", "fn main() { foo::foo(); }")
        .build();

    let output = p.cargo("build --release -v").exec_with_output().unwrap();
    verify_lto(
        &output,
        "foo",
        "--crate-type dylib --crate-type rlib",
        Lto::ObjectAndBitcode,
    );
    verify_lto(&output, "ferret", "--crate-type bin", Lto::Run(None));
}

#[cargo_test]
fn fresh_swapping_commands() {
    // In some rare cases, different commands end up building dependencies
    // with different LTO settings. This checks that it doesn't cause the
    // cache to thrash in that scenario.
    Package::new("bar", "1.0.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [dependencies]
                bar = "1.0"

                [profile.release]
                lto = true
            "#,
        )
        .file("src/lib.rs", "pub fn foo() { println!(\"hi!\"); }")
        .build();

    p.cargo("build --release -v")
        .with_stderr(
            "\
[UPDATING] [..]
[DOWNLOADING] crates ...
[DOWNLOADED] bar v1.0.0 [..]
[COMPILING] bar v1.0.0
[RUNNING] `rustc --crate-name bar [..]-C linker-plugin-lto[..]
[COMPILING] foo v0.1.0 [..]
[RUNNING] `rustc --crate-name foo src/lib.rs [..]-C linker-plugin-lto[..]
[FINISHED] [..]
",
        )
        .run();
    p.cargo("test --release -v")
        .with_stderr_unordered(
            "\
[FRESH] bar v1.0.0
[COMPILING] foo v0.1.0 [..]
[RUNNING] `rustc --crate-name foo src/lib.rs [..]-C lto[..]--test[..]
[FINISHED] [..]
[RUNNING] `[..]/foo[..]`
[DOCTEST] foo
[RUNNING] `rustdoc [..]-C lto[..]
",
        )
        .run();

    p.cargo("build --release -v")
        .with_stderr(
            "\
[FRESH] bar v1.0.0
[FRESH] foo [..]
[FINISHED] [..]
",
        )
        .run();
    p.cargo("test --release -v --no-run -v")
        .with_stderr(
            "\
[FRESH] bar v1.0.0
[FRESH] foo [..]
[FINISHED] [..]
[EXECUTABLE] `[..]/target/release/deps/foo-[..][EXE]`
",
        )
        .run();
}
