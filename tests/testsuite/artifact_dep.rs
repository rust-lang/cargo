//! Tests specific to artifact dependencies, designated using
//! the new `dep = { artifact = "bin", … }` syntax in manifests.

use cargo_test_support::compare::match_exact;
use cargo_test_support::registry::Package;
use cargo_test_support::{
    basic_bin_manifest, basic_manifest, cross_compile, project, publish, registry, rustc_host,
    Project,
};

#[cargo_test]
fn check_with_invalid_artifact_dependency() {
    // invalid name
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.0"
                authors = []
                resolver = "2"
                
                [dependencies]
                bar = { path = "bar/", artifact = "unknown" }
            "#,
        )
        .file("src/lib.rs", "extern crate bar;") // this would fail but we don't get there, artifacts are no libs
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file("bar/src/lib.rs", "")
        .build();
    p.cargo("check -Z bindeps")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[ERROR] failed to parse manifest at `[..]/Cargo.toml`

Caused by:
  'unknown' is not a valid artifact specifier
",
        )
        .with_status(101)
        .run();

    fn run_cargo_with_and_without_bindeps_feature(
        p: &Project,
        cmd: &str,
        assert: &dyn Fn(&mut cargo_test_support::Execs),
    ) {
        assert(
            p.cargo(&format!("{} -Z bindeps", cmd))
                .masquerade_as_nightly_cargo(),
        );
        assert(&mut p.cargo(cmd));
    }

    // lib specified without artifact
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.0"
                authors = []
                
                [dependencies]
                bar = { path = "bar/", lib = true }
            "#,
        )
        .file("src/lib.rs", "")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file("bar/src/lib.rs", "")
        .build();
    run_cargo_with_and_without_bindeps_feature(&p, "check", &|cargo| {
        cargo
            .with_stderr(
                "\
[ERROR] failed to parse manifest at `[..]/Cargo.toml`

Caused by:
  'lib' specifier cannot be used without an 'artifact = …' value (bar)
",
            )
            .with_status(101)
            .run();
    });

    // target specified without artifact
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.0"
                authors = []
                
                [dependencies]
                bar = { path = "bar/", target = "target" }
            "#,
        )
        .file("src/lib.rs", "")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file("bar/src/lib.rs", "")
        .build();
    run_cargo_with_and_without_bindeps_feature(&p, "check", &|cargo| {
        cargo
            .with_stderr(
                "\
[ERROR] failed to parse manifest at `[..]/Cargo.toml`

Caused by:
  'target' specifier cannot be used without an 'artifact = …' value (bar)
",
            )
            .with_status(101)
            .run();
    })
}

#[cargo_test]
fn check_with_invalid_target_triple() {
    // invalid name
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.0"
                authors = []
                resolver = "2"
                
                [dependencies]
                bar = { path = "bar/", artifact = "bin", target = "unknown-target-triple" }
            "#,
        )
        .file("src/lib.rs", "")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file("bar/src/main.rs", "fn main() {}")
        .build();
    p.cargo("check -Z bindeps")
        .masquerade_as_nightly_cargo()
        .with_stderr_contains(
            r#"[..]Could not find specification for target "unknown-target-triple"[..]"#,
        )
        .with_status(101)
        .run();
}

#[cargo_test]
fn build_without_nightly_aborts_with_error() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.0"
                authors = []
                resolver = "2"
                
                [dependencies]
                bar = { path = "bar/", artifact = "bin" }
            "#,
        )
        .file("src/lib.rs", "extern crate bar;")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file("bar/src/lib.rs", "")
        .build();
    p.cargo("check")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] failed to parse manifest at [..]

Caused by:
  `artifact = …` requires `-Z bindeps` (bar)
",
        )
        .run();
}

#[cargo_test]
fn disallow_artifact_and_no_artifact_dep_to_same_package_within_the_same_dep_category() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.0"
                authors = []
                resolver = "2"
                
                [dependencies]
                bar = { path = "bar/", artifact = "bin" }
                bar_stable = { path = "bar/", package = "bar" }
            "#,
        )
        .file("src/lib.rs", "")
        .file("bar/Cargo.toml", &basic_bin_manifest("bar"))
        .file("bar/src/main.rs", "fn main() {}")
        .build();
    p.cargo("check -Z bindeps")
        .masquerade_as_nightly_cargo()
        .with_status(101)
        .with_stderr("\
[WARNING] foo v0.0.0 ([CWD]) ignoring invalid dependency `bar_stable` which is missing a lib target
[ERROR] the crate `foo v0.0.0 ([CWD])` depends on crate `bar v0.5.0 ([CWD]/bar)` multiple times with different names",
        )
        .run();
}

#[cargo_test]
fn features_are_unified_among_lib_and_bin_dep_of_same_target() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
                authors = []
                resolver = "2"

                [dependencies.d1]
                path = "d1"
                features = ["d1f1"]
                artifact = "bin"
                lib = true
                
                [dependencies.d2]
                path = "d2"
                features = ["d2f2"]
            "#,
        )
        .file(
            "src/main.rs",
            r#"
                fn main() {
                    d1::f1();
                    d1::f2();
                    d2::f1();
                    d2::f2();
                }
            "#,
        )
        .file(
            "d1/Cargo.toml",
            r#"
                [package]
                name = "d1"
                version = "0.0.1"
                authors = []

                [features]
                d1f1 = ["d2"]

                [dependencies.d2]
                path = "../d2"
                features = ["d2f1"]
                optional = true
            "#,
        )
        .file(
            "d1/src/main.rs",
            r#"fn main() {
                #[cfg(feature = "d1f1")]
                d2::f1();
                
                // Using f2 is only possible as features are unififed across the same target.
                // Our own manifest would only enable f1, and f2 comes in because a parent crate
                // enables the feature in its manifest.
                #[cfg(feature = "d1f1")]
                d2::f2();
            }"#,
        )
        .file(
            "d1/src/lib.rs",
            r#"
            #[cfg(feature = "d2")]
            extern crate d2;
            /// Importing f2 here shouldn't be possible as unless features are unified.
            #[cfg(feature = "d1f1")]
            pub use d2::{f1, f2};
        "#,
        )
        .file(
            "d2/Cargo.toml",
            r#"
                [package]
                name = "d2"
                version = "0.0.1"
                authors = []

                [features]
                d2f1 = []
                d2f2 = []
            "#,
        )
        .file(
            "d2/src/lib.rs",
            r#"
                #[cfg(feature = "d2f1")] pub fn f1() {}
                #[cfg(feature = "d2f2")] pub fn f2() {}
            "#,
        )
        .build();

    p.cargo("build -Z bindeps")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[COMPILING] d2 v0.0.1 ([CWD]/d2)
[COMPILING] d1 v0.0.1 ([CWD]/d1)
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cargo_test]
fn features_are_not_unified_among_lib_and_bin_dep_of_different_target() {
    if cross_compile::disabled() {
        return;
    }
    let target = cross_compile::alternate();
    let p = project()
        .file(
            "Cargo.toml",
            &r#"
                [project]
                name = "foo"
                version = "0.0.1"
                authors = []
                resolver = "2"

                [dependencies.d1]
                path = "d1"
                features = ["d1f1"]
                artifact = "bin"
                lib = true
                target = "$TARGET"
                
                [dependencies.d2]
                path = "d2"
                features = ["d2f2"]
            "#
            .replace("$TARGET", target),
        )
        .file(
            "src/main.rs",
            r#"
                fn main() {
                    // the lib = true part always builds for our current target, unifying dependencies
                    d1::d2::f1();
                    d1::d2::f2();
                    d2::f1();
                    d2::f2();
                }
            "#,
        )
        .file(
            "d1/Cargo.toml",
            r#"
                [package]
                name = "d1"
                version = "0.0.1"
                authors = []

                [features]
                d1f1 = ["d2"]

                [dependencies.d2]
                path = "../d2"
                features = ["d2f1"]
                optional = true
            "#,
        )
        .file("d1/src/main.rs", r#"fn main() {
            // f1 we set ourselves
            d2::f1();
            // As 'main' is only compiled as part of the artifact dependency and since that is not unified
            // if the target differs, trying to access f2 is a compile time error as the feature isn't enabled in our dependency tree.
            d2::f2();
        }"#)
        .file(
            "d1/src/lib.rs",
            r#"
            #[cfg(feature = "d2")]
            pub extern crate d2;
        "#,
        )
        .file(
            "d2/Cargo.toml",
            r#"
                [package]
                name = "d2"
                version = "0.0.1"
                authors = []

                [features]
                d2f1 = []
                d2f2 = []
            "#,
        )
        .file(
            "d2/src/lib.rs",
            r#"
                #[cfg(feature = "d2f1")] pub fn f1() {}
                #[cfg(feature = "d2f2")] pub fn f2() {}
            "#,
        )
        .build();

    p.cargo("build -Z bindeps")
        .masquerade_as_nightly_cargo()
        .with_status(101)
        .with_stderr_contains(
            "error[E0425]: cannot find function `f2` in crate `d2`\n --> d1/src/main.rs:6:17",
        )
        .run();
}

#[cargo_test]
fn build_script_with_bin_artifacts() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.0"
                authors = []
                resolver = "2"
                
                [build-dependencies]
                bar = { path = "bar/", artifact = ["bin", "staticlib", "cdylib"] }
            "#,
        )
        .file("src/lib.rs", "")
        .file("build.rs", r#"
            fn main() {
                let baz: std::path::PathBuf = std::env::var("CARGO_BIN_FILE_BAR_baz").expect("CARGO_BIN_FILE_BAR_baz").into();
                println!("{}", baz.display());
                assert!(&baz.is_file()); 
                
                let lib: std::path::PathBuf = std::env::var("CARGO_STATICLIB_FILE_BAR_bar").expect("CARGO_STATICLIB_FILE_BAR_bar").into();
                println!("{}", lib.display());
                assert!(&lib.is_file()); 
                
                let lib: std::path::PathBuf = std::env::var("CARGO_CDYLIB_FILE_BAR_bar").expect("CARGO_CDYLIB_FILE_BAR_bar").into();
                println!("{}", lib.display());
                assert!(&lib.is_file()); 
                
                let dir: std::path::PathBuf = std::env::var("CARGO_BIN_DIR_BAR").expect("CARGO_BIN_DIR_BAR").into();
                println!("{}", dir.display());
                assert!(dir.is_dir());
                
                let bar: std::path::PathBuf = std::env::var("CARGO_BIN_FILE_BAR").expect("CARGO_BIN_FILE_BAR").into();
                println!("{}", bar.display());
                assert!(&bar.is_file()); 
                
                let bar2: std::path::PathBuf = std::env::var("CARGO_BIN_FILE_BAR_bar").expect("CARGO_BIN_FILE_BAR_bar").into();
                println!("{}", bar2.display());
                assert_eq!(bar, bar2);
            }
        "#)
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.5.0"
                authors = []
                
                [lib]
                crate-type = ["staticlib", "cdylib"]
            "#,
        )
        // compilation target is native for build scripts unless overridden
        .file("bar/src/bin/bar.rs", &format!(r#"fn main() {{ assert_eq!(std::env::var("TARGET").unwrap(), "{}"); }}"#, cross_compile::native()))
        .file("bar/src/bin/baz.rs", "fn main() {}")
        .file("bar/src/lib.rs", "")
        .build();
    p.cargo("build -Z bindeps")
        .masquerade_as_nightly_cargo()
        .with_stderr_contains("[COMPILING] foo [..]")
        .with_stderr_contains("[COMPILING] bar v0.5.0 ([CWD]/bar)")
        .with_stderr_contains("[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]")
        .run();

    let build_script_output = build_script_output_string(&p, "foo");
    let msg = "we need the binary directory for this artifact along with all binary paths";
    if cfg!(target_env = "msvc") {
        match_exact(
            "[..]/artifact/bar-[..]/bin/baz.exe\n\
             [..]/artifact/bar-[..]/staticlib/bar-[..].lib\n\
             [..]/artifact/bar-[..]/cdylib/bar.dll\n\
             [..]/artifact/bar-[..]/bin\n\
             [..]/artifact/bar-[..]/bin/bar.exe\n\
             [..]/artifact/bar-[..]/bin/bar.exe",
            &build_script_output,
            msg,
            "",
            None,
        )
        .unwrap();
    } else {
        match_exact(
            "[..]/artifact/bar-[..]/bin/baz-[..]\n\
             [..]/artifact/bar-[..]/staticlib/libbar-[..].a\n\
             [..]/artifact/bar-[..]/cdylib/[..]bar.[..]\n\
             [..]/artifact/bar-[..]/bin\n\
             [..]/artifact/bar-[..]/bin/bar-[..]\n\
             [..]/artifact/bar-[..]/bin/bar-[..]",
            &build_script_output,
            msg,
            "",
            None,
        )
        .unwrap();
    }

    assert!(
        !p.bin("bar").is_file(),
        "artifacts are located in their own directory, exclusively, and won't be lifted up"
    );
    assert!(!p.bin("baz").is_file(),);
    assert_artifact_executable_output(&p, "debug", "bar", "bar");
}

#[cargo_test]
fn build_script_with_bin_artifact_and_lib_false() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.0"
                authors = []
                resolver = "2"
                
                [build-dependencies]
                bar = { path = "bar/", artifact = "bin" }
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "build.rs",
            r#"
            fn main() {
               bar::doit()
            }
        "#,
        )
        .file("bar/Cargo.toml", &basic_bin_manifest("bar"))
        .file("bar/src/main.rs", "fn main() { bar::doit(); }")
        .file(
            "bar/src/lib.rs",
            r#"
            pub fn doit() {
               panic!("sentinel");
            }
        "#,
        )
        .build();
    p.cargo("build -Z bindeps")
        .masquerade_as_nightly_cargo()
        .with_status(101)
        .with_stderr_does_not_contain("[..]sentinel[..]")
        .run();
}

#[cargo_test]
fn lib_with_bin_artifact_and_lib_false() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.0"
                authors = []
                resolver = "2"
                
                [dependencies]
                bar = { path = "bar/", artifact = "bin" }
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
            pub fn foo() {
               bar::doit()
            }"#,
        )
        .file("bar/Cargo.toml", &basic_bin_manifest("bar"))
        .file("bar/src/main.rs", "fn main() { bar::doit(); }")
        .file(
            "bar/src/lib.rs",
            r#"
            pub fn doit() {
               panic!("sentinel");
            }
        "#,
        )
        .build();
    p.cargo("build -Z bindeps")
        .masquerade_as_nightly_cargo()
        .with_status(101)
        .with_stderr_does_not_contain("[..]sentinel[..]")
        .run();
}

#[cargo_test]
fn build_script_with_selected_dashed_bin_artifact_and_lib_true() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.0"
                authors = []
                resolver = "2"
                
                [build-dependencies]
                bar-baz = { path = "bar/", artifact = "bin:baz-suffix", lib = true }
            "#,
        )
        .file("src/lib.rs", "")
        .file("build.rs", r#"
            fn main() {
               bar_baz::print_env()
            }
        "#)
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar-baz"
                version = "0.5.0"
                authors = []
                
                [[bin]]
                name = "bar"
                
                [[bin]]
                name = "baz-suffix"
            "#,
        )
        .file("bar/src/main.rs", "fn main() {}")
        .file("bar/src/lib.rs", r#"
            pub fn print_env() {
                let dir: std::path::PathBuf = std::env::var("CARGO_BIN_DIR_BAR_BAZ").expect("CARGO_BIN_DIR_BAR_BAZ").into();
                let bin: std::path::PathBuf = std::env::var("CARGO_BIN_FILE_BAR_BAZ_baz-suffix").expect("CARGO_BIN_FILE_BAR_BAZ_baz-suffix").into();
                println!("{}", dir.display());
                println!("{}", bin.display());
                assert!(dir.is_dir());
                assert!(&bin.is_file());
                assert!(std::env::var("CARGO_BIN_FILE_BAR_BAZ").is_err(), "CARGO_BIN_FILE_BAR_BAZ isn't set due to name mismatch");
                assert!(std::env::var("CARGO_BIN_FILE_BAR_BAZ_bar").is_err(), "CARGO_BIN_FILE_BAR_BAZ_bar isn't set as binary isn't selected");
            }
        "#)
        .build();
    p.cargo("build -Z bindeps")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[COMPILING] bar-baz v0.5.0 ([CWD]/bar)
[COMPILING] foo [..]
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]",
        )
        .run();

    let build_script_output = build_script_output_string(&p, "foo");
    let msg = "we need the binary directory for this artifact and the binary itself";

    if cfg!(target_env = "msvc") {
        cargo_test_support::compare::match_exact(
            &format!(
                "[..]/artifact/bar-baz-[..]/bin\n\
                 [..]/artifact/bar-baz-[..]/bin/baz_suffix{}",
                std::env::consts::EXE_SUFFIX,
            ),
            &build_script_output,
            msg,
            "",
            None,
        )
        .unwrap();
    } else {
        cargo_test_support::compare::match_exact(
            "[..]/artifact/bar-baz-[..]/bin\n\
        [..]/artifact/bar-baz-[..]/bin/baz_suffix-[..]",
            &build_script_output,
            msg,
            "",
            None,
        )
        .unwrap();
    }

    assert!(
        !p.bin("bar").is_file(),
        "artifacts are located in their own directory, exclusively, and won't be lifted up"
    );
    assert_artifact_executable_output(&p, "debug", "bar", "baz_suffix");
}

#[cargo_test]
fn lib_with_selected_dashed_bin_artifact_and_lib_true() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.0"
                authors = []
                resolver = "2"
                
                [dependencies]
                bar-baz = { path = "bar/", artifact = ["bin:baz-suffix", "staticlib", "cdylib"], lib = true }
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
            pub fn foo() {
                bar_baz::exists();
                
                env!("CARGO_BIN_DIR_BAR_BAZ");
                let _b = include_bytes!(env!("CARGO_BIN_FILE_BAR_BAZ_baz-suffix"));
                let _b = include_bytes!(env!("CARGO_STATICLIB_FILE_BAR_BAZ"));
                let _b = include_bytes!(env!("CARGO_STATICLIB_FILE_BAR_BAZ_bar-baz"));
                let _b = include_bytes!(env!("CARGO_CDYLIB_FILE_BAR_BAZ"));
                let _b = include_bytes!(env!("CARGO_CDYLIB_FILE_BAR_BAZ_bar-baz"));
            }
        "#,
        )
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar-baz"
                version = "0.5.0"
                authors = []
                
                [lib]
                crate-type = ["rlib", "staticlib", "cdylib"]
                
                [[bin]]
                name = "bar"
                
                [[bin]]
                name = "baz-suffix"
            "#,
        )
        .file("bar/src/main.rs", "fn main() {}")
        .file("bar/src/lib.rs", "pub fn exists() {}")
        .build();
    p.cargo("build -Z bindeps")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[COMPILING] bar-baz v0.5.0 ([CWD]/bar)
[COMPILING] foo [..]
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]",
        )
        .run();

    assert!(
        !p.bin("bar").is_file(),
        "artifacts are located in their own directory, exclusively, and won't be lifted up"
    );
    assert_artifact_executable_output(&p, "debug", "bar", "baz_suffix");
}

#[cargo_test]
fn allow_artifact_and_no_artifact_dep_to_same_package_within_different_dep_categories() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.0"
                authors = []
                resolver = "2"
                
                [dependencies]
                bar = { path = "bar/", artifact = "bin" }
                
                [dev-dependencies]
                bar = { path = "bar/", package = "bar" }
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
            #[cfg(test)] extern crate bar;
            pub fn foo() {
                env!("CARGO_BIN_DIR_BAR");
                let _b = include_bytes!(env!("CARGO_BIN_FILE_BAR"));
            }"#,
        )
        .file("bar/Cargo.toml", &basic_bin_manifest("bar"))
        .file("bar/src/main.rs", "fn main() {}")
        .file("bar/src/lib.rs", "")
        .build();
    p.cargo("test -Z bindeps")
        .masquerade_as_nightly_cargo()
        .with_stderr_contains("[COMPILING] bar v0.5.0 ([CWD]/bar)")
        .with_stderr_contains("[FINISHED] test [unoptimized + debuginfo] target(s) in [..]")
        .run();
}

#[cargo_test]
fn normal_build_deps_are_picked_up_in_presence_of_an_artifact_build_dep_to_the_same_package() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.0"
                authors = []
                resolver = "2"

                [dependencies]
                bar = { path = "bar", artifact = "bin:bar" }

                [build-dependencies]
                bar = { path = "bar" }
            "#,
        )
        .file("build.rs", "fn main() { bar::f(); }")
        .file(
            "src/lib.rs",
            r#"
            pub fn foo() {
                env!("CARGO_BIN_DIR_BAR");
                let _b = include_bytes!(env!("CARGO_BIN_FILE_BAR"));
            }"#,
        )
        .file("bar/Cargo.toml", &basic_bin_manifest("bar"))
        .file("bar/src/main.rs", "fn main() {}")
        .file("bar/src/lib.rs", "pub fn f() {}")
        .build();
    p.cargo("check -Z bindeps")
        .masquerade_as_nightly_cargo()
        .run();
}

#[cargo_test]
fn disallow_using_example_binaries_as_artifacts() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.0"
                authors = []
                resolver = "2"
                
                [dependencies]
                bar = { path = "bar/", artifact = "bin:one-example" }
            "#,
        )
        .file("src/lib.rs", "")
        .file("bar/Cargo.toml", &basic_bin_manifest("bar"))
        .file("bar/src/main.rs", "fn main() {}")
        .file("bar/examples/one-example.rs", "fn main() {}")
        .build();
    p.cargo("build -Z bindeps")
        .masquerade_as_nightly_cargo()
        .with_status(101)
        .with_stderr(r#"[ERROR] dependency `bar` in package `foo` requires a `bin:one-example` artifact to be present."#)
        .run();
}

/// From RFC 3028
///
/// > You may also specify separate dependencies with different artifact values, as well as
/// dependencies on the same crate without artifact specified; for instance, you may have a
/// build dependency on the binary of a crate and a normal dependency on the Rust library of the same crate.
#[cargo_test]
fn allow_artifact_and_non_artifact_dependency_to_same_crate() {
    let p = project()
            .file(
                "Cargo.toml",
                r#"
                [package]
                name = "foo"
                version = "0.0.0"
                authors = []
                resolver = "2"
                
                [build-dependencies]
                bar = { path = "bar/", artifact = "bin" }
                
                [dependencies]
                bar = { path = "bar/" }
            "#,
            )
            .file("src/lib.rs", r#"
                    pub fn foo() {
                         bar::doit();
                         assert!(option_env!("CARGO_BIN_FILE_BAR").is_none());
                    }"#)
            .file(
                "build.rs",
                r#"
                fn main() {
                     assert!(option_env!("CARGO_BIN_FILE_BAR").is_none(), "no environment variables at build time");
                     std::process::Command::new(std::env::var("CARGO_BIN_FILE_BAR").expect("BAR present")).status().unwrap();
                }"#,
            )
            .file("bar/Cargo.toml", &basic_bin_manifest("bar"))
            .file("bar/src/main.rs", "fn main() {}")
            .file("bar/src/lib.rs", "pub fn doit() {}")
        .build();

    p.cargo("check -Z bindeps")
        .masquerade_as_nightly_cargo()
        .with_stderr_contains("[COMPILING] bar [..]")
        .with_stderr_contains("[COMPILING] foo [..]")
        .run();
}

#[cargo_test]
fn build_script_deps_adopt_specified_target_unconditionally() {
    if cross_compile::disabled() {
        return;
    }

    let target = cross_compile::alternate();
    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                [package]
                name = "foo"
                version = "0.0.0"
                authors = []
                resolver = "2"
                
                [build-dependencies.bar]
                path = "bar/"
                artifact = "bin"
                target = "{}"
            "#,
                target
            ),
        )
        .file("src/lib.rs", "")
        .file("build.rs", r#"
                fn main() {
                    let bar: std::path::PathBuf = std::env::var("CARGO_BIN_FILE_BAR").expect("CARGO_BIN_FILE_BAR").into();
                    assert!(&bar.is_file()); 
                }"#)
        .file("bar/Cargo.toml", &basic_bin_manifest("bar"))
        .file("bar/src/main.rs", "fn main() {}")
        .file("bar/src/lib.rs", "pub fn doit() {}")
        .build();

    p.cargo("check -v -Z bindeps")
        .masquerade_as_nightly_cargo()
        .with_stderr_does_not_contain(format!(
            "[RUNNING] `rustc --crate-name build_script_build build.rs [..]--target {} [..]",
            target
        ))
        .with_stderr_contains("[RUNNING] `rustc --crate-name build_script_build build.rs [..]")
        .with_stderr_contains(format!(
            "[RUNNING] `rustc --crate-name bar bar/src/lib.rs [..]--target {} [..]",
            target
        ))
        .with_stderr_contains(format!(
            "[RUNNING] `rustc --crate-name bar bar/src/main.rs [..]--target {} [..]",
            target
        ))
        .with_stderr_does_not_contain(format!(
            "[RUNNING] `rustc --crate-name foo [..]--target {} [..]",
            target
        ))
        .with_stderr_contains("[RUNNING] `rustc --crate-name foo [..]")
        .run();
}

/// inverse RFC-3176
#[cargo_test]
fn build_script_deps_adopt_do_not_allow_multiple_targets_under_different_name_and_same_version() {
    if cross_compile::disabled() {
        return;
    }

    let alternate = cross_compile::alternate();
    let native = cross_compile::native();
    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                [package]
                name = "foo"
                version = "0.0.0"
                authors = []
                resolver = "2"
                
                [build-dependencies.bar]
                path = "bar/"
                artifact = "bin"
                target = "{}"
                
                [build-dependencies.bar-native]
                package = "bar"
                path = "bar/"
                artifact = "bin"
                target = "{}"
            "#,
                alternate,
                native
            ),
        )
        .file("src/lib.rs", "")
        .file("build.rs", r#"
                fn main() {
                    let bar: std::path::PathBuf = std::env::var("CARGO_BIN_FILE_BAR").expect("CARGO_BIN_FILE_BAR").into();
                    assert!(&bar.is_file()); 
                    let bar_native: std::path::PathBuf = std::env::var("CARGO_BIN_FILE_BAR_NATIVE_bar").expect("CARGO_BIN_FILE_BAR_NATIVE_bar").into();
                    assert!(&bar_native.is_file()); 
                    assert_ne!(bar_native, bar, "should build different binaries due to different targets"); 
                }"#)
        .file("bar/Cargo.toml", &basic_bin_manifest("bar"))
        .file("bar/src/main.rs", "fn main() {}")
        .build();

    p.cargo("check -v -Z bindeps")
        .masquerade_as_nightly_cargo()
        .with_status(101)
        .with_stderr(format!(
            "error: the crate `foo v0.0.0 ([CWD])` depends on crate `bar v0.5.0 ([CWD]/bar)` multiple times with different names",
        ))
        .run();
}

#[cargo_test]
fn non_build_script_deps_adopt_specified_target_unconditionally() {
    if cross_compile::disabled() {
        return;
    }

    let target = cross_compile::alternate();
    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                [package]
                name = "foo"
                version = "0.0.0"
                authors = []
                resolver = "2"
                
                [dependencies.bar]
                path = "bar/"
                artifact = "bin"
                target = "{}"
            "#,
                target
            ),
        )
        .file(
            "src/lib.rs",
            r#"pub fn foo() { let _b = include_bytes!(env!("CARGO_BIN_FILE_BAR")); }"#,
        )
        .file("bar/Cargo.toml", &basic_bin_manifest("bar"))
        .file("bar/src/main.rs", "fn main() {}")
        .file("bar/src/lib.rs", "pub fn doit() {}")
        .build();

    p.cargo("check -v -Z bindeps")
        .masquerade_as_nightly_cargo()
        .with_stderr_contains(format!(
            "[RUNNING] `rustc --crate-name bar bar/src/lib.rs [..]--target {} [..]",
            target
        ))
        .with_stderr_contains(format!(
            "[RUNNING] `rustc --crate-name bar bar/src/main.rs [..]--target {} [..]",
            target
        ))
        .with_stderr_does_not_contain(format!(
            "[RUNNING] `rustc --crate-name foo [..]--target {} [..]",
            target
        ))
        .with_stderr_contains("[RUNNING] `rustc --crate-name foo [..]")
        .run();
}

#[cargo_test]
fn no_cross_doctests_works_with_artifacts() {
    if cross_compile::disabled() {
        return;
    }

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []
                resolver = "2"
                
                [dependencies]
                bar = { path = "bar/", artifact = "bin", lib = true }
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
                //! ```
                //! env!("CARGO_BIN_DIR_BAR");
                //! let _b = include_bytes!(env!("CARGO_BIN_FILE_BAR"));
                //! ```
                pub fn foo() {
                    env!("CARGO_BIN_DIR_BAR");
                    let _b = include_bytes!(env!("CARGO_BIN_FILE_BAR"));
                }
            "#,
        )
        .file("bar/Cargo.toml", &basic_bin_manifest("bar"))
        .file("bar/src/lib.rs", r#"pub extern "C" fn c() {}"#)
        .file("bar/src/main.rs", "fn main() {}")
        .build();

    let target = rustc_host();
    p.cargo("test -Z bindeps --target")
        .arg(&target)
        .masquerade_as_nightly_cargo()
        .with_stderr(&format!(
            "\
[COMPILING] bar v0.5.0 ([CWD]/bar)
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]
[RUNNING] [..] (target/{triple}/debug/deps/foo-[..][EXE])
[DOCTEST] foo
",
            triple = target
        ))
        .run();

    println!("c");
    let target = cross_compile::alternate();

    // This will build the library, but does not build or run doc tests.
    // This should probably be a warning or error.
    p.cargo("test -Z bindeps -v --doc --target")
        .arg(&target)
        .masquerade_as_nightly_cargo()
        .with_stderr_contains(format!(
            "[COMPILING] bar v0.5.0 ([CWD]/bar)
[RUNNING] `rustc --crate-name bar bar/src/lib.rs [..]--target {triple} [..]
[RUNNING] `rustc --crate-name bar bar/src/main.rs [..]--target {triple} [..]
[COMPILING] foo v0.0.1 ([CWD])
[RUNNING] `rustc --crate-name foo [..]
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]",
            triple = target
        ))
        .run();

    if !cross_compile::can_run_on_host() {
        return;
    }

    // This tests the library, but does not run the doc tests.
    p.cargo("test -Z bindeps -v --target")
        .arg(&target)
        .masquerade_as_nightly_cargo()
        .with_stderr_contains(&format!(
            "[FRESH] bar v0.5.0 ([CWD]/bar)
[COMPILING] foo v0.0.1 ([CWD])
[RUNNING] `rustc --crate-name foo [..]--test[..]
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]
[RUNNING] `[CWD]/target/{triple}/debug/deps/foo-[..][EXE]`",
            triple = target
        ))
        .run();
}

#[cargo_test]
fn build_script_deps_adopts_target_platform_if_target_equals_target() {
    if cross_compile::disabled() {
        return;
    }

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.0"
                authors = []
                resolver = "2"
                
                [build-dependencies]
                bar = { path = "bar/", artifact = "bin", target = "target" }
            "#,
        )
        .file("src/lib.rs", "")
        .file("build.rs", r#"
                fn main() {
                    let bar: std::path::PathBuf = std::env::var("CARGO_BIN_FILE_BAR").expect("CARGO_BIN_FILE_BAR").into();
                    assert!(&bar.is_file()); 
                }"#)
        .file("bar/Cargo.toml", &basic_bin_manifest("bar"))
        .file("bar/src/main.rs", "fn main() {}")
        .file("bar/src/lib.rs", "pub fn doit() {}")
        .build();

    let alternate_target = cross_compile::alternate();
    p.cargo("check -v -Z bindeps --target")
        .arg(alternate_target)
        .masquerade_as_nightly_cargo()
        .with_stderr_does_not_contain(format!(
            "[RUNNING] `rustc --crate-name build_script_build build.rs [..]--target {} [..]",
            alternate_target
        ))
        .with_stderr_contains("[RUNNING] `rustc --crate-name build_script_build build.rs [..]")
        .with_stderr_contains(format!(
            "[RUNNING] `rustc --crate-name bar bar/src/lib.rs [..]--target {} [..]",
            alternate_target
        ))
        .with_stderr_contains(format!(
            "[RUNNING] `rustc --crate-name bar bar/src/main.rs [..]--target {} [..]",
            alternate_target
        ))
        .with_stderr_contains(format!(
            "[RUNNING] `rustc --crate-name foo [..]--target {} [..]",
            alternate_target
        ))
        .run();
}

#[cargo_test]
#[cfg_attr(target_env = "msvc", ignore)] // TODO(ST): rename bar (dependency) to something else and un-ignore this with RFC-3176
fn profile_override_basic() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [build-dependencies]
                bar = { path = "bar", artifact = "bin" }
                
                [dependencies]
                bar = { path = "bar", artifact = "bin" }
                
                [profile.dev.build-override]
                opt-level = 1

                [profile.dev]
                opt-level = 3
            "#,
        )
        .file("build.rs", "fn main() {}")
        .file("src/lib.rs", "")
        .file("bar/Cargo.toml", &basic_bin_manifest("bar"))
        .file("bar/src/main.rs", "fn main() {}")
        .file("bar/src/lib.rs", "pub fn bar() {}")
        .build();

    p.cargo("build -v -Z bindeps")
        .masquerade_as_nightly_cargo()
        .with_stderr_contains(
            "[RUNNING] `rustc --crate-name build_script_build [..] -C opt-level=1 [..]`",
        )
        .with_stderr_contains(
            "[RUNNING] `rustc --crate-name bar bar/src/main.rs [..] -C opt-level=3 [..]`",
        )
        .with_stderr_contains(
            "[RUNNING] `rustc --crate-name bar bar/src/main.rs [..] -C opt-level=1 [..]`",
        )
        .with_stderr_contains(
            "[RUNNING] `rustc --crate-name bar bar/src/lib.rs [..] -C opt-level=1 [..]`",
        )
        .with_stderr_contains(
            "[RUNNING] `rustc --crate-name bar bar/src/lib.rs [..] -C opt-level=3 [..]`",
        )
        .with_stderr_contains("[RUNNING] `rustc --crate-name foo [..] -C opt-level=3 [..]`")
        .run();
}

#[cargo_test]
fn dependencies_of_dependencies_work_in_artifacts() {
    Package::new("baz", "1.0.0")
        .file("src/lib.rs", "pub fn baz() {}")
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.0"
                authors = []
                resolver = "2"
                
                [build-dependencies]
                bar = { path = "bar/", artifact = "bin" }
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "build.rs",
            r#"
            fn main() {
                std::process::Command::new(std::env::var("CARGO_BIN_FILE_BAR").expect("BAR present")).status().unwrap();
            }
            "#,
        )
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.5.0"
                authors = []
                
                [dependencies]
                baz = "1.0.0"
            "#,
        )
        .file("bar/src/lib.rs", r#"pub fn bar() {baz::baz()}"#)
        .file("bar/src/main.rs", r#"fn main() {bar::bar()}"#)
        .build();
    p.cargo("build -Z bindeps")
        .masquerade_as_nightly_cargo()
        .run();

    // cargo tree sees artifacts as the dependency kind they are in and doesn't do anything special with it.
    p.cargo("tree -Z bindeps")
        .masquerade_as_nightly_cargo()
        .with_stdout(
            "\
foo v0.0.0 ([CWD])
[build-dependencies]
└── bar v0.5.0 ([CWD]/bar)
    └── baz v1.0.0
",
        )
        .run();
}

// TODO: Fix this potentially by reverting 887562bfeb8c540594d7d08e6e9a4ab7eb255865 which adds artifact information to the registry
//       followed by 0ff93733626f7cbecaf9dce9ab62b4ced0be088e which picks it up.
//       For reference, see comments by ehuss https://github.com/rust-lang/cargo/pull/9992#discussion_r801086315 and
//       joshtriplett https://github.com/rust-lang/cargo/pull/9992#issuecomment-1033394197 .
#[cargo_test]
#[ignore]
fn targets_are_picked_up_from_non_workspace_artifact_deps() {
    if cross_compile::disabled() {
        return;
    }
    let target = cross_compile::alternate();
    Package::new("artifact", "1.0.0")
        .file("src/main.rs", r#"fn main() {}"#)
        .file("src/lib.rs", r#"pub fn lib() {}"#)
        .publish();

    let mut dep = registry::Dependency::new("artifact", "1.0.0");
    Package::new("uses-artifact", "1.0.0")
        .file(
            "src/lib.rs",
            r#"pub fn uses_artifact() { let _b = include_bytes!(env!("CARGO_BIN_FILE_ARTIFACT")); }"#,
        )
        .add_dep(dep.artifact("bin", Some(target.to_string())))
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.0"
                authors = []
                
                [dependencies]
                uses-artifact = { version = "1.0.0" }
            "#,
        )
        .file(
            "src/lib.rs",
            r#"pub fn foo() { uses_artifact::uses_artifact(); }"#,
        )
        .build();

    p.cargo("build -Z bindeps")
        .masquerade_as_nightly_cargo()
        .run();
}

#[cargo_test]
fn allow_dep_renames_with_multiple_versions() {
    Package::new("bar", "1.0.0")
        .file("src/main.rs", r#"fn main() {println!("1.0.0")}"#)
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.0"
                authors = []
                resolver = "2"
                
                [build-dependencies]
                bar = { path = "bar/", artifact = "bin" }
                bar_stable = { package = "bar", version = "1.0.0", artifact = "bin" }
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "build.rs",
            r#"
            fn main() {
                std::process::Command::new(std::env::var("CARGO_BIN_FILE_BAR").expect("BAR present")).status().unwrap();
                std::process::Command::new(std::env::var("CARGO_BIN_FILE_BAR_STABLE_bar").expect("BAR STABLE present")).status().unwrap();
            }
            "#,
        )
        .file("bar/Cargo.toml", &basic_bin_manifest("bar"))
        .file("bar/src/main.rs", r#"fn main() {println!("0.5.0")}"#)
        .build();
    p.cargo("check -Z bindeps")
        .masquerade_as_nightly_cargo()
        .with_stderr_contains("[COMPILING] bar [..]")
        .with_stderr_contains("[COMPILING] foo [..]")
        .run();
    let build_script_output = build_script_output_string(&p, "foo");
    match_exact(
        "0.5.0\n1.0.0",
        &build_script_output,
        "build script output",
        "",
        None,
    )
    .unwrap();
}

#[cargo_test]
fn allow_artifact_and_non_artifact_dependency_to_same_crate_if_these_are_not_the_same_dep_kind() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.0"
                authors = []
                resolver = "2"
                
                [build-dependencies]
                bar = { path = "bar/", artifact = "bin", lib = false }
                
                [dependencies]
                bar = { path = "bar/" }
            "#,
        )
        .file("src/lib.rs", r#"
            pub fn foo() {
                bar::doit();
                assert!(option_env!("CARGO_BIN_FILE_BAR").is_none());
            }"#)
        .file(
            "build.rs",
            r#"fn main() {
               println!("{}", std::env::var("CARGO_BIN_FILE_BAR").expect("CARGO_BIN_FILE_BAR"));
               println!("{}", std::env::var("CARGO_BIN_FILE_BAR_bar").expect("CARGO_BIN_FILE_BAR_bar"));
           }"#,
        )
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file("bar/src/lib.rs", "pub fn doit() {}")
        .file("bar/src/main.rs", "fn main() {}")
        .build();
    p.cargo("build -Z bindeps")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[COMPILING] bar [..]
[COMPILING] foo [..]
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cargo_test]
fn prevent_no_lib_warning_with_artifact_dependencies() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.0"
                authors = []
                resolver = "2"
                
                [dependencies]
                bar = { path = "bar/", artifact = "bin" }
            "#,
        )
        .file(
            "src/lib.rs",
            r#"pub fn foo() { let _b = include_bytes!(env!("CARGO_BIN_FILE_BAR")); }"#,
        )
        .file("bar/Cargo.toml", &basic_bin_manifest("bar"))
        .file("bar/src/main.rs", "fn main() {}")
        .build();
    p.cargo("check -Z bindeps")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
            [COMPILING] bar v0.5.0 ([CWD]/bar)\n\
            [CHECKING] foo v0.0.0 ([CWD])\n\
            [FINISHED] dev [unoptimized + debuginfo] target(s) in [..]",
        )
        .run();
}

#[cargo_test]
fn show_no_lib_warning_with_artifact_dependencies_that_have_no_lib_but_lib_true() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.0"
                authors = []
                resolver = "2"
                
                [build-dependencies]
                bar = { path = "bar/", artifact = "bin" }
                
                [dependencies]
                bar = { path = "bar/", artifact = "bin", lib = true }
            "#,
        )
        .file("src/lib.rs", "")
        .file("src/build.rs", "fn main() {}")
        .file("bar/Cargo.toml", &basic_bin_manifest("bar"))
        .file("bar/src/main.rs", "fn main() {}")
        .build();
    p.cargo("check -Z bindeps")
        .masquerade_as_nightly_cargo()
        .with_stderr_contains("[WARNING] foo v0.0.0 ([CWD]) ignoring invalid dependency `bar` which is missing a lib target")
        .with_stderr_contains("[COMPILING] bar v0.5.0 ([CWD]/bar)")
        .with_stderr_contains("[CHECKING] foo [..]")
        .with_stderr_contains("[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]")
        .run();
}

#[cargo_test]
fn resolver_2_build_dep_without_lib() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.0"
                authors = []
                edition = "2021"
                
                [build-dependencies]
                bar = { path = "bar/", artifact = "bin" }
            "#,
        )
        .file("src/lib.rs", "")
        .file("build.rs", r#"
                fn main() {
                    let bar: std::path::PathBuf = std::env::var("CARGO_BIN_FILE_BAR").expect("CARGO_BIN_FILE_BAR").into();
                    assert!(&bar.is_file()); 
                }"#)
        .file("bar/Cargo.toml", &basic_bin_manifest("bar"))
        .file("bar/src/main.rs", "fn main() {}")
        .build();
    p.cargo("check -Z bindeps")
        .masquerade_as_nightly_cargo()
        .run();
}

#[cargo_test]
fn check_missing_crate_type_in_package_fails() {
    for crate_type in &["cdylib", "staticlib", "bin"] {
        let p = project()
            .file(
                "Cargo.toml",
                &format!(
                    r#"
                        [package]
                        name = "foo"
                        version = "0.0.0"
                        authors = []
                        
                        [dependencies]
                        bar = {{ path = "bar/", artifact = "{}" }}
                    "#,
                    crate_type
                ),
            )
            .file("src/lib.rs", "")
            .file("bar/Cargo.toml", &basic_manifest("bar", "0.0.1")) //no bin, just rlib
            .file("bar/src/lib.rs", "")
            .build();
        p.cargo("check -Z bindeps")
            .masquerade_as_nightly_cargo()
            .with_status(101)
            .with_stderr(
                "[ERROR] dependency `bar` in package `foo` requires a `[..]` artifact to be present.",
            )
            .run();
    }
}

#[cargo_test]
fn check_target_equals_target_in_non_build_dependency_errors() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.0"
                authors = []
                resolver = "2"

                [dependencies]
                bar = { path = "bar/", artifact = "bin", target = "target" }
            "#,
        )
        .file("src/lib.rs", "")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file("bar/src/main.rs", "fn main() {}")
        .build();
    p.cargo("check -Z bindeps")
        .masquerade_as_nightly_cargo()
        .with_status(101)
        .with_stderr_contains(
            "  `target = \"target\"` in normal- or dev-dependencies has no effect (bar)",
        )
        .run();
}

#[cargo_test]
fn env_vars_and_build_products_for_various_build_targets() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.0"
                authors = []
                resolver = "2"
                
                [lib]
                doctest = true
                
                [build-dependencies]
                bar = { path = "bar/", artifact = ["cdylib", "staticlib"] }
                
                [dependencies]
                bar = { path = "bar/", artifact = "bin", lib = true }
                
                [dev-dependencies]
                bar = { path = "bar/", artifact = "bin:baz" }
            "#,
        )
        .file("build.rs", r#"
            fn main() {
                let file: std::path::PathBuf = std::env::var("CARGO_CDYLIB_FILE_BAR").expect("CARGO_CDYLIB_FILE_BAR").into();
                assert!(&file.is_file()); 
                
                let file: std::path::PathBuf = std::env::var("CARGO_STATICLIB_FILE_BAR").expect("CARGO_STATICLIB_FILE_BAR").into();
                assert!(&file.is_file()); 
                
                assert!(std::env::var("CARGO_BIN_FILE_BAR").is_err());
                assert!(std::env::var("CARGO_BIN_FILE_BAR_baz").is_err());
            }
        "#)
        .file(
            "src/lib.rs",
            r#"
                //! ```
                //! bar::c();
                //! env!("CARGO_BIN_DIR_BAR");
                //! let _b = include_bytes!(env!("CARGO_BIN_FILE_BAR"));
                //! let _b = include_bytes!(env!("CARGO_BIN_FILE_BAR_bar"));
                //! let _b = include_bytes!(env!("CARGO_BIN_FILE_BAR_baz"));
                //! assert!(option_env!("CARGO_STATICLIB_FILE_BAR").is_none());
                //! assert!(option_env!("CARGO_CDYLIB_FILE_BAR").is_none());
                //! ```
                pub fn foo() {
                    bar::c();
                    env!("CARGO_BIN_DIR_BAR");
                    let _b = include_bytes!(env!("CARGO_BIN_FILE_BAR"));
                    let _b = include_bytes!(env!("CARGO_BIN_FILE_BAR_bar"));
                    let _b = include_bytes!(env!("CARGO_BIN_FILE_BAR_baz"));
                    assert!(option_env!("CARGO_STATICLIB_FILE_BAR").is_none());
                    assert!(option_env!("CARGO_CDYLIB_FILE_BAR").is_none());
                }
                
                #[cfg(test)]
                #[test]
                fn env_unit() {
                    env!("CARGO_BIN_DIR_BAR");
                    let _b = include_bytes!(env!("CARGO_BIN_FILE_BAR"));
                    let _b = include_bytes!(env!("CARGO_BIN_FILE_BAR_bar"));
                    let _b = include_bytes!(env!("CARGO_BIN_FILE_BAR_baz"));
                    assert!(option_env!("CARGO_STATICLIB_FILE_BAR").is_none());
                    assert!(option_env!("CARGO_CDYLIB_FILE_BAR").is_none());
                }
               "#,
        )
        .file(
            "tests/main.rs",
            r#"
                #[test]
                fn env_integration() {
                    env!("CARGO_BIN_DIR_BAR");
                    let _b = include_bytes!(env!("CARGO_BIN_FILE_BAR"));
                    let _b = include_bytes!(env!("CARGO_BIN_FILE_BAR_bar"));
                    let _b = include_bytes!(env!("CARGO_BIN_FILE_BAR_baz"));
                }"#,
        )
        .file("build.rs", "fn main() {}")
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.5.0"
                authors = []
                
                [lib]
                crate-type = ["staticlib", "cdylib", "rlib"]
                
                [[bin]]
                name = "bar"
                
                [[bin]]
                name = "baz"
            "#,
        )
        .file("bar/src/lib.rs", r#"pub extern "C" fn c() {}"#)
        .file("bar/src/main.rs", "fn main() {}")
        .build();
    p.cargo("test -Z bindeps")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[COMPILING] bar [..]
[COMPILING] foo [..]
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]
[RUNNING] unittests [..]
[RUNNING] tests/main.rs [..]
[DOCTEST] foo
",
        )
        .run();
}

#[cargo_test]
fn publish_artifact_dep() {
    registry::init();
    Package::new("bar", "1.0.0").publish();
    Package::new("baz", "1.0.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []
            license = "MIT"
            description = "foo"
            documentation = "foo"
            homepage = "foo"
            repository = "foo"
            resolver = "2"

            [dependencies]
            bar = { version = "1.0", artifact = "bin", lib = true }
            
            [build-dependencies]
            baz = { version = "1.0", artifact = ["bin:a", "cdylib", "staticlib"], target = "target" }
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("publish -Z bindeps --no-verify --token sekrit")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[UPDATING] [..]
[PACKAGING] foo v0.1.0 [..]
[UPLOADING] foo v0.1.0 [..]
",
        )
        .run();

    publish::validate_upload_with_contents(
        r#"
        {
          "authors": [],
          "badges": {},
          "categories": [],
          "deps": [{
              "default_features": true,
              "features": [],
              "kind": "normal",
              "name": "bar",
              "optional": false,
              "registry": "https://github.com/rust-lang/crates.io-index",
              "target": null,
              "version_req": "^1.0"
            },
            {
              "default_features": true,
              "features": [],
              "kind": "build",
              "name": "baz",
              "optional": false,
              "registry": "https://github.com/rust-lang/crates.io-index",
              "target": null,
              "version_req": "^1.0"
            }
          ],
          "description": "foo",
          "documentation": "foo",
          "features": {},
          "homepage": "foo",
          "keywords": [],
          "license": "MIT",
          "license_file": null,
          "links": null,
          "name": "foo",
          "readme": null,
          "readme_file": null,
          "repository": "foo",
          "vers": "0.1.0"
        }
        "#,
        "foo-0.1.0.crate",
        &["Cargo.toml", "Cargo.toml.orig", "src/lib.rs"],
        &[(
            "Cargo.toml",
            &format!(
                r#"{}
[package]
name = "foo"
version = "0.1.0"
authors = []
description = "foo"
homepage = "foo"
documentation = "foo"
license = "MIT"
repository = "foo"
resolver = "2"

[dependencies.bar]
version = "1.0"
artifact = ["bin"]
lib = true

[build-dependencies.baz]
version = "1.0"
artifact = [
    "bin:a",
    "cdylib",
    "staticlib",
]
target = "target""#,
                cargo::core::package::MANIFEST_PREAMBLE
            ),
        )],
    );
}

#[cargo_test]
fn doc_lib_true() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []
                resolver = "2"

                [dependencies.bar]
                path = "bar"
                artifact = "bin"
                lib = true
            "#,
        )
        .file("src/lib.rs", "extern crate bar; pub fn foo() {}")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file("bar/src/lib.rs", "pub fn bar() {}")
        .file("bar/src/main.rs", "fn main() {}")
        .build();

    p.cargo("doc -Z bindeps")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[COMPILING] bar v0.0.1 ([CWD]/bar)
[DOCUMENTING] bar v0.0.1 ([CWD]/bar)
[DOCUMENTING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();

    assert!(p.root().join("target/doc").is_dir());
    assert!(p.root().join("target/doc/foo/index.html").is_file());
    assert!(p.root().join("target/doc/bar/index.html").is_file());

    // Verify that it emits rmeta for the bin and lib dependency.
    assert_eq!(p.glob("target/debug/artifact/*.rlib").count(), 0);
    assert_eq!(p.glob("target/debug/deps/libbar-*.rmeta").count(), 2);

    p.cargo("doc -Z bindeps")
        .masquerade_as_nightly_cargo()
        .env("CARGO_LOG", "cargo::ops::cargo_rustc::fingerprint")
        .with_stdout("")
        .run();

    assert!(p.root().join("target/doc").is_dir());
    assert!(p.root().join("target/doc/foo/index.html").is_file());
    assert!(p.root().join("target/doc/bar/index.html").is_file());
}

#[cargo_test]
fn rustdoc_works_on_libs_with_artifacts_and_lib_false() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []
                resolver = "2"

                [dependencies.bar]
                path = "bar"
                artifact = ["bin", "staticlib", "cdylib"]
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
            pub fn foo() {
                env!("CARGO_BIN_DIR_BAR");
                let _b = include_bytes!(env!("CARGO_BIN_FILE_BAR"));
                let _b = include_bytes!(env!("CARGO_CDYLIB_FILE_BAR"));
                let _b = include_bytes!(env!("CARGO_CDYLIB_FILE_BAR_bar"));
                let _b = include_bytes!(env!("CARGO_STATICLIB_FILE_BAR"));
                let _b = include_bytes!(env!("CARGO_STATICLIB_FILE_BAR_bar"));
            }"#,
        )
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.5.0"
                authors = []
                
                [lib]
                crate-type = ["staticlib", "cdylib"]
            "#,
        )
        .file("bar/src/lib.rs", "pub fn bar() {}")
        .file("bar/src/main.rs", "fn main() {}")
        .build();

    p.cargo("doc -Z bindeps")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[COMPILING] bar v0.5.0 ([CWD]/bar)
[DOCUMENTING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();

    assert!(p.root().join("target/doc").is_dir());
    assert!(p.root().join("target/doc/foo/index.html").is_file());
    assert!(
        !p.root().join("target/doc/bar/index.html").is_file(),
        "bar is not a lib dependency and thus remains undocumented"
    );
}

fn assert_artifact_executable_output(
    p: &Project,
    target_name: &str,
    dep_name: &str,
    bin_name: &str,
) {
    if cfg!(target_env = "msvc") {
        assert_eq!(
            p.glob(format!(
                "target/{}/deps/artifact/{}-*/bin/{}{}",
                target_name,
                dep_name,
                bin_name,
                std::env::consts::EXE_SUFFIX
            ))
            .count(),
            1,
            "artifacts are placed into their own output directory to not possibly clash"
        );
    } else {
        assert_eq!(
            p.glob(format!(
                "target/{}/deps/artifact/{}-*/bin/{}-*{}",
                target_name,
                dep_name,
                bin_name,
                std::env::consts::EXE_SUFFIX
            ))
            .filter_map(Result::ok)
            .filter(|f| f.extension().map_or(true, |ext| ext != "o" && ext != "d"))
            .count(),
            1,
            "artifacts are placed into their own output directory to not possibly clash"
        );
    }
}

fn build_script_output_string(p: &Project, package_name: &str) -> String {
    let paths = p
        .glob(format!("target/debug/build/{}-*/output", package_name))
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(paths.len(), 1);
    std::fs::read_to_string(&paths[0]).unwrap()
}
