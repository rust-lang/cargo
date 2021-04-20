//! Tests for profile overrides (build-override and per-package overrides).

use cargo_test_support::registry::Package;
use cargo_test_support::{basic_lib_manifest, basic_manifest, project};

#[cargo_test]
fn profile_override_basic() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies]
                bar = {path = "bar"}

                [profile.dev]
                opt-level = 1

                [profile.dev.package.bar]
                opt-level = 3
            "#,
        )
        .file("src/lib.rs", "")
        .file("bar/Cargo.toml", &basic_lib_manifest("bar"))
        .file("bar/src/lib.rs", "")
        .build();

    p.cargo("build -v")
        .with_stderr(
            "[COMPILING] bar [..]
[RUNNING] `rustc --crate-name bar [..] -C opt-level=3 [..]`
[COMPILING] foo [..]
[RUNNING] `rustc --crate-name foo [..] -C opt-level=1 [..]`
[FINISHED] dev [optimized + debuginfo] target(s) in [..]",
        )
        .run();
}

#[cargo_test]
fn profile_override_warnings() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"

                [dependencies]
                bar = {path = "bar"}

                [profile.dev.package.bart]
                opt-level = 3

                [profile.dev.package.no-suggestion]
                opt-level = 3

                [profile.dev.package."bar:1.2.3"]
                opt-level = 3
            "#,
        )
        .file("src/lib.rs", "")
        .file("bar/Cargo.toml", &basic_lib_manifest("bar"))
        .file("bar/src/lib.rs", "")
        .build();

    p.cargo("build")
        .with_stderr_contains(
            "\
[WARNING] profile package spec `bar:1.2.3` in profile `dev` \
    has a version or URL that does not match any of the packages: \
    bar v0.5.0 ([..]/foo/bar)
[WARNING] profile package spec `bart` in profile `dev` did not match any packages

<tab>Did you mean `bar`?
[WARNING] profile package spec `no-suggestion` in profile `dev` did not match any packages
[COMPILING] [..]
",
        )
        .run();
}

#[cargo_test]
fn profile_override_bad_settings() {
    let bad_values = [
        (
            "panic = \"abort\"",
            "`panic` may not be specified in a `package` profile",
        ),
        (
            "lto = true",
            "`lto` may not be specified in a `package` profile",
        ),
        (
            "rpath = true",
            "`rpath` may not be specified in a `package` profile",
        ),
        ("package = {}", "package-specific profiles cannot be nested"),
    ];
    for &(snippet, expected) in bad_values.iter() {
        let p = project()
            .file(
                "Cargo.toml",
                &format!(
                    r#"
                        [package]
                        name = "foo"
                        version = "0.0.1"

                        [dependencies]
                        bar = {{path = "bar"}}

                        [profile.dev.package.bar]
                        {}
                    "#,
                    snippet
                ),
            )
            .file("src/lib.rs", "")
            .file("bar/Cargo.toml", &basic_lib_manifest("bar"))
            .file("bar/src/lib.rs", "")
            .build();

        p.cargo("build")
            .with_status(101)
            .with_stderr_contains(format!("Caused by:\n  {}", expected))
            .run();
    }
}

#[cargo_test]
fn profile_override_hierarchy() {
    // Test that the precedence rules are correct for different types.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["m1", "m2", "m3"]

            [profile.dev]
            codegen-units = 1

            [profile.dev.package.m2]
            codegen-units = 2

            [profile.dev.package."*"]
            codegen-units = 3

            [profile.dev.build-override]
            codegen-units = 4
            "#,
        )
        // m1
        .file(
            "m1/Cargo.toml",
            r#"
            [package]
            name = "m1"
            version = "0.0.1"

            [dependencies]
            m2 = { path = "../m2" }
            dep = { path = "../../dep" }
            "#,
        )
        .file("m1/src/lib.rs", "extern crate m2; extern crate dep;")
        .file("m1/build.rs", "fn main() {}")
        // m2
        .file(
            "m2/Cargo.toml",
            r#"
            [package]
            name = "m2"
            version = "0.0.1"

            [dependencies]
            m3 = { path = "../m3" }

            [build-dependencies]
            m3 = { path = "../m3" }
            dep = { path = "../../dep" }
            "#,
        )
        .file("m2/src/lib.rs", "extern crate m3;")
        .file(
            "m2/build.rs",
            "extern crate m3; extern crate dep; fn main() {}",
        )
        // m3
        .file("m3/Cargo.toml", &basic_lib_manifest("m3"))
        .file("m3/src/lib.rs", "")
        .build();

    // dep (outside of workspace)
    let _dep = project()
        .at("dep")
        .file("Cargo.toml", &basic_lib_manifest("dep"))
        .file("src/lib.rs", "")
        .build();

    // Profiles should be:
    // m3: 4 (as build.rs dependency)
    // m3: 1 (as [profile.dev] as workspace member)
    // dep: 3 (as [profile.dev.package."*"] as non-workspace member)
    // dep: 4 (as [profile.dev.build-override])
    // m1 build.rs: 4 (as [profile.dev.build-override])
    // m2 build.rs: 2 (as [profile.dev.package.m2])
    // m2: 2 (as [profile.dev.package.m2])
    // m1: 1 (as [profile.dev])

    p.cargo("build -v").with_stderr_unordered("\
[COMPILING] m3 [..]
[COMPILING] dep [..]
[RUNNING] `rustc --crate-name m3 m3/src/lib.rs [..] --crate-type lib --emit=[..]link[..]-C codegen-units=4 [..]
[RUNNING] `rustc --crate-name dep [..]dep/src/lib.rs [..] --crate-type lib --emit=[..]link[..]-C codegen-units=3 [..]
[RUNNING] `rustc --crate-name dep [..]dep/src/lib.rs [..] --crate-type lib --emit=[..]link[..]-C codegen-units=4 [..]
[RUNNING] `rustc --crate-name m3 m3/src/lib.rs [..] --crate-type lib --emit=[..]link[..]-C codegen-units=1 [..]
[RUNNING] `rustc --crate-name build_script_build m1/build.rs [..] --crate-type bin --emit=[..]link[..]-C codegen-units=4 [..]
[COMPILING] m2 [..]
[RUNNING] `rustc --crate-name build_script_build m2/build.rs [..] --crate-type bin --emit=[..]link[..]-C codegen-units=2 [..]
[RUNNING] `[..]/m1-[..]/build-script-build`
[RUNNING] `[..]/m2-[..]/build-script-build`
[RUNNING] `rustc --crate-name m2 m2/src/lib.rs [..] --crate-type lib --emit=[..]link[..]-C codegen-units=2 [..]
[COMPILING] m1 [..]
[RUNNING] `rustc --crate-name m1 m1/src/lib.rs [..] --crate-type lib --emit=[..]link[..]-C codegen-units=1 [..]
[FINISHED] dev [unoptimized + debuginfo] [..]
",
        )
        .run();
}

#[cargo_test]
fn profile_override_spec_multiple() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"

            [dependencies]
            bar = { path = "bar" }

            [profile.dev.package.bar]
            opt-level = 3

            [profile.dev.package."bar:0.5.0"]
            opt-level = 3
            "#,
        )
        .file("src/lib.rs", "")
        .file("bar/Cargo.toml", &basic_lib_manifest("bar"))
        .file("bar/src/lib.rs", "")
        .build();

    p.cargo("build -v")
        .with_status(101)
        .with_stderr_contains(
            "\
[ERROR] multiple package overrides in profile `dev` match package `bar v0.5.0 ([..])`
found package specs: bar, bar:0.5.0",
        )
        .run();
}

#[cargo_test]
fn profile_override_spec() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["m1", "m2"]

            [profile.dev.package."dep:1.0.0"]
            codegen-units = 1

            [profile.dev.package."dep:2.0.0"]
            codegen-units = 2
            "#,
        )
        // m1
        .file(
            "m1/Cargo.toml",
            r#"
            [package]
            name = "m1"
            version = "0.0.1"

            [dependencies]
            dep = { path = "../../dep1" }
            "#,
        )
        .file("m1/src/lib.rs", "extern crate dep;")
        // m2
        .file(
            "m2/Cargo.toml",
            r#"
            [package]
            name = "m2"
            version = "0.0.1"

            [dependencies]
            dep = {path = "../../dep2" }
            "#,
        )
        .file("m2/src/lib.rs", "extern crate dep;")
        .build();

    project()
        .at("dep1")
        .file("Cargo.toml", &basic_manifest("dep", "1.0.0"))
        .file("src/lib.rs", "")
        .build();

    project()
        .at("dep2")
        .file("Cargo.toml", &basic_manifest("dep", "2.0.0"))
        .file("src/lib.rs", "")
        .build();

    p.cargo("build -v")
        .with_stderr_contains("[RUNNING] `rustc [..]dep1/src/lib.rs [..] -C codegen-units=1 [..]")
        .with_stderr_contains("[RUNNING] `rustc [..]dep2/src/lib.rs [..] -C codegen-units=2 [..]")
        .run();
}

#[cargo_test]
fn override_proc_macro() {
    Package::new("shared", "1.0.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            edition = "2018"

            [dependencies]
            shared = "1.0"
            pm = {path = "pm"}

            [profile.dev.build-override]
            codegen-units = 4
            "#,
        )
        .file("src/lib.rs", r#"pm::eat!{}"#)
        .file(
            "pm/Cargo.toml",
            r#"
            [package]
            name = "pm"
            version = "0.1.0"

            [lib]
            proc-macro = true

            [dependencies]
            shared = "1.0"
            "#,
        )
        .file(
            "pm/src/lib.rs",
            r#"
            extern crate proc_macro;
            use proc_macro::TokenStream;

            #[proc_macro]
            pub fn eat(_item: TokenStream) -> TokenStream {
                "".parse().unwrap()
            }
            "#,
        )
        .build();

    p.cargo("build -v")
        // Shared built for the proc-macro.
        .with_stderr_contains("[RUNNING] `rustc [..]--crate-name shared [..]-C codegen-units=4[..]")
        // Shared built for the library.
        .with_stderr_line_without(
            &["[RUNNING] `rustc --crate-name shared"],
            &["-C codegen-units"],
        )
        .with_stderr_contains("[RUNNING] `rustc [..]--crate-name pm [..]-C codegen-units=4[..]")
        .with_stderr_line_without(
            &["[RUNNING] `rustc [..]--crate-name foo"],
            &["-C codegen-units"],
        )
        .run();
}

#[cargo_test]
fn no_warning_ws() {
    // https://github.com/rust-lang/cargo/issues/7378, avoid warnings in a workspace.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["a", "b"]

            [profile.dev.package.a]
            codegen-units = 3
            "#,
        )
        .file("a/Cargo.toml", &basic_manifest("a", "0.1.0"))
        .file("a/src/lib.rs", "")
        .file("b/Cargo.toml", &basic_manifest("b", "0.1.0"))
        .file("b/src/lib.rs", "")
        .build();

    p.cargo("build -p b")
        .with_stderr(
            "\
[COMPILING] b [..]
[FINISHED] [..]
",
        )
        .run();
}

#[cargo_test]
fn build_override_shared() {
    // A dependency with a build script that is shared with a build
    // dependency, using different profile settings. That is:
    //
    // foo DEBUG=2
    // ├── common DEBUG=2
    // │   └── common Run build.rs DEBUG=2
    // │       └── common build.rs DEBUG=0 (build_override)
    // └── foo Run build.rs DEBUG=2
    //     └── foo build.rs DEBUG=0 (build_override)
    //         └── common DEBUG=0 (build_override)
    //             └── common Run build.rs DEBUG=0 (build_override)
    //                 └── common build.rs DEBUG=0 (build_override)
    //
    // The key part here is that `common` RunCustomBuild is run twice, once
    // with DEBUG=2 (as a dependency of foo) and once with DEBUG=0 (as a
    // build-dependency of foo's build script).
    Package::new("common", "1.0.0")
        .file(
            "build.rs",
            r#"
            fn main() {
                if std::env::var("DEBUG").unwrap() != "false" {
                    println!("cargo:rustc-cfg=foo_debug");
                } else {
                    println!("cargo:rustc-cfg=foo_release");
                }
            }
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
            pub fn foo() -> u32 {
                if cfg!(foo_debug) {
                    assert!(cfg!(debug_assertions));
                    1
                } else if cfg!(foo_release) {
                    assert!(!cfg!(debug_assertions));
                    2
                } else {
                    panic!("not set");
                }
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
            edition = "2018"

            [build-dependencies]
            common = "1.0"

            [dependencies]
            common = "1.0"

            [profile.dev.build-override]
            debug = 0
            debug-assertions = false
            "#,
        )
        .file(
            "build.rs",
            r#"
            fn main() {
                assert_eq!(common::foo(), 2);
            }
            "#,
        )
        .file(
            "src/main.rs",
            r#"
            fn main() {
                assert_eq!(common::foo(), 1);
            }
            "#,
        )
        .build();

    p.cargo("run").run();
}
