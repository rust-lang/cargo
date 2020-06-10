use cargo::core::compiler::Lto;
use cargo_test_support::registry::Package;
use cargo_test_support::{project, Project};
use std::process::Output;

#[cargo_test]
fn with_deps() {
    if !cargo_test_support::is_nightly() {
        return;
    }

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
        .with_stderr_contains("[..]`rustc[..]--crate-name bar[..]-Clinker-plugin-lto[..]`")
        .with_stderr_contains("[..]`rustc[..]--crate-name test[..]-C lto[..]`")
        .run();
}

#[cargo_test]
fn shared_deps() {
    if !cargo_test_support::is_nightly() {
        return;
    }

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
    if !cargo_test_support::is_nightly() {
        return;
    }

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
        .with_stderr_contains("[..]`rustc[..]--crate-name bar[..]-Cembed-bitcode=no[..]`")
        .with_stderr_contains("[..]`rustc[..]--crate-name test[..]-C lto[..]`")
        .run();
}

#[cargo_test]
fn complicated() {
    if !cargo_test_support::is_nightly() {
        return;
    }

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
            "#,
        )
        .file("build.rs", "fn main() { dep_build::foo() }")
        .file(
            "src/main.rs",
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
        .with_stderr_contains("[..]`rustc[..]--crate-name dep_normal2 [..]-Clinker-plugin-lto[..]`")
        .with_stderr_contains("[..]`rustc[..]--crate-name dep_normal [..]-Clinker-plugin-lto[..]`")
        // build dependencies and their transitive deps don't need any bitcode,
        // so embedding should be turned off
        .with_stderr_contains("[..]`rustc[..]--crate-name dep_build2 [..]-Cembed-bitcode=no[..]`")
        .with_stderr_contains("[..]`rustc[..]--crate-name dep_build [..]-Cembed-bitcode=no[..]`")
        .with_stderr_contains(
            "[..]`rustc[..]--crate-name build_script_build [..]-Cembed-bitcode=no[..]`",
        )
        // proc macro deps are the same as build deps here
        .with_stderr_contains(
            "[..]`rustc[..]--crate-name dep_proc_macro2 [..]-Cembed-bitcode=no[..]`",
        )
        .with_stderr_contains(
            "[..]`rustc[..]--crate-name dep_proc_macro [..]-Cembed-bitcode=no[..]`",
        )
        .with_stderr_contains("[..]`rustc[..]--crate-name test [..]--crate-type bin[..]-C lto[..]`")
        .with_stderr_contains(
            "[..]`rustc[..]--crate-name test [..]--crate-type cdylib[..]-C lto[..]`",
        )
        .with_stderr_contains("[..]`rustc[..]--crate-name dep_shared [..]`")
        .with_stderr_does_not_contain("[..]--crate-name dep_shared[..]-C lto[..]")
        .with_stderr_does_not_contain("[..]--crate-name dep_shared[..]-Clinker-plugin-lto[..]")
        .with_stderr_does_not_contain("[..]--crate-name dep_shared[..]-Cembed-bitcode[..]")
        .run();
}

#[cargo_test]
fn off_in_manifest_works() {
    if !cargo_test_support::is_nightly() {
        return;
    }

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
[RUNNING] `rustc --crate-name bar [..]--crate-type lib [..]-Cembed-bitcode=no[..]
[COMPILING] test [..]
[RUNNING] `rustc --crate-name test [..]--crate-type lib [..]-Cembed-bitcode=no[..]
[RUNNING] `rustc --crate-name test src/main.rs [..]--crate-type bin [..]-C lto=off[..]
[FINISHED] [..]
",
        )
        .run();
}

#[cargo_test]
fn between_builds() {
    if !cargo_test_support::is_nightly() {
        return;
    }

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
[RUNNING] `rustc [..]--crate-type lib[..]-Clinker-plugin-lto[..]
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
    if !cargo_test_support::is_nightly() {
        return;
    }

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
    if !cargo_test_support::is_nightly() {
        return;
    }

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
    let actual_lto = if let Some(index) = line.find("-C lto=") {
        let s = &line[index..];
        let end = s.find(' ').unwrap();
        let mode = &line[index..index + end];
        if mode == "off" {
            Lto::Off
        } else {
            Lto::Run(Some(mode.into()))
        }
    } else if line.contains("-C lto") {
        Lto::Run(None)
    } else if line.contains("-Clinker-plugin-lto") {
        Lto::OnlyBitcode
    } else if line.contains("-Cembed-bitcode=no") {
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
    if !cargo_test_support::is_nightly() {
        return;
    }
    let p = project_with_dep("'cdylib', 'rlib'");
    let output = p.cargo("build --release -v").exec_with_output().unwrap();
    verify_lto(
        &output,
        "registry",
        "--crate-type lib",
        Lto::ObjectAndBitcode,
    );
    verify_lto(
        &output,
        "registry_shared",
        "--crate-type lib",
        Lto::ObjectAndBitcode,
    );
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
[RUNNING] `rustc --crate-name foo [..]-Cembed-bitcode=no --test[..]
[RUNNING] `rustc --crate-name a [..]-Cembed-bitcode=no --test[..]
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
[FRESH] registry v0.0.1
[FRESH] registry-shared v0.0.1
[COMPILING] bar [..]
[RUNNING] `rustc --crate-name bar [..]-Cembed-bitcode=no --test[..]
[RUNNING] `rustc --crate-name b [..]-Cembed-bitcode=no --test[..]
[FINISHED] [..]
[RUNNING] [..]
[RUNNING] [..]
[DOCTEST] bar
[RUNNING] `rustdoc --crate-type cdylib --crate-type rlib --test [..]
",
        )
        .run();
}

#[cargo_test]
fn dylib() {
    if !cargo_test_support::is_nightly() {
        return;
    }
    let p = project_with_dep("'dylib'");
    let output = p.cargo("build --release -v").exec_with_output().unwrap();
    verify_lto(&output, "registry", "--crate-type lib", Lto::OnlyObject);
    verify_lto(
        &output,
        "registry_shared",
        "--crate-type lib",
        Lto::ObjectAndBitcode,
    );
    verify_lto(&output, "bar", "--crate-type dylib", Lto::OnlyObject);
    verify_lto(&output, "foo", "--crate-type bin", Lto::Run(None));
    p.cargo("test --release -v")
        .with_stderr_unordered(
            "\
[FRESH] registry v0.0.1
[FRESH] registry-shared v0.0.1
[FRESH] bar v0.0.0 [..]
[COMPILING] foo [..]
[RUNNING] `rustc --crate-name foo [..]-Cembed-bitcode=no --test[..]
[RUNNING] `rustc --crate-name a [..]-Cembed-bitcode=no --test[..]
[FINISHED] [..]
[RUNNING] [..]
[RUNNING] [..]
",
        )
        .run();
    p.cargo("build --release -v --manifest-path bar/Cargo.toml")
        .with_stderr_unordered(
            "\
[COMPILING] registry-shared v0.0.1
[FRESH] registry v0.0.1
[RUNNING] `rustc --crate-name registry_shared [..]-Cembed-bitcode=no[..]
[COMPILING] bar [..]
[RUNNING] `rustc --crate-name bar [..]--crate-type dylib [..]-Cembed-bitcode=no[..]
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
[RUNNING] `rustc --crate-name bar [..]-Cembed-bitcode=no --test[..]
[RUNNING] `rustc --crate-name b [..]-Cembed-bitcode=no --test[..]
[FINISHED] [..]
[RUNNING] [..]
[RUNNING] [..]
",
        )
        .run();
}

#[cargo_test]
fn test_profile() {
    if !cargo_test_support::is_nightly() {
        return;
    }
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
        .with_stderr("\
[UPDATING] [..]
[DOWNLOADING] [..]
[DOWNLOADED] [..]
[COMPILING] bar v0.0.1
[RUNNING] `rustc --crate-name bar [..]crate-type lib[..]
[COMPILING] foo [..]
[RUNNING] `rustc --crate-name foo [..]--crate-type lib --emit=dep-info,metadata,link -Cembed-bitcode=no[..]
[RUNNING] `rustc --crate-name foo [..]--emit=dep-info,link -C lto=thin [..]--test[..]
[FINISHED] [..]
[RUNNING] [..]
[DOCTEST] foo
[RUNNING] `rustdoc [..]
")
        .run();
}

#[cargo_test]
fn dev_profile() {
    if !cargo_test_support::is_nightly() {
        return;
    }
    // Mixing dev=LTO with test=not-LTO
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

                [profile.dev]
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
        .with_stderr("\
[UPDATING] [..]
[DOWNLOADING] [..]
[DOWNLOADED] [..]
[COMPILING] bar v0.0.1
[RUNNING] `rustc --crate-name bar [..]crate-type lib[..]
[COMPILING] foo [..]
[RUNNING] `rustc --crate-name foo [..]--crate-type lib --emit=dep-info,metadata,link -Clinker-plugin-lto [..]
[RUNNING] `rustc --crate-name foo [..]--emit=dep-info,link -Cembed-bitcode=no [..]--test[..]
[FINISHED] [..]
[RUNNING] [..]
[DOCTEST] foo
[RUNNING] `rustdoc [..]
")
        .run();
}
