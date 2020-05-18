use cargo_test_support::project;
use cargo_test_support::registry::Package;

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
        .file("src/main.rs", "fn main() { bar::foo() }")
        .build();
    p.cargo("build -v --release").run();
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
    p.cargo("build -v --release --lib").run();
    p.cargo("build -v --release").run();
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

#[cargo_test]
fn cdylib_and_rlib() {
    if !cargo_test_support::is_nightly() {
        return;
    }

    Package::new("registry", "0.0.1")
        .file("src/lib.rs", "pub fn foo() {}")
        .publish();
    Package::new("registry-shared", "0.0.1")
        .file("src/lib.rs", "pub fn foo() {}")
        .publish();

    let p = project()
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
            r#"
                [package]
                name = "bar"
                version = "0.0.0"

                [dependencies]
                registry = "*"
                registry-shared = "*"

                [lib]
                crate-type = ['cdylib', 'rlib']
            "#,
        )
        .file(
            "bar/src/lib.rs",
            "
                pub fn foo() {
                    registry::foo();
                    registry_shared::foo();
                }
            ",
        )
        .file("tests/a.rs", "")
        .file("bar/tests/b.rs", "")
        .build();
    p.cargo("build --release -v").run();
    p.cargo("test --release -v").run();
    p.cargo("build --release -v --manifest-path bar/Cargo.toml")
        .run();
    p.cargo("test --release -v --manifest-path bar/Cargo.toml")
        .run();
}
