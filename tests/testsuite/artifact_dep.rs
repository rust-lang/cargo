//! Tests specific to artifact dependencies, designated using
//! the new `dep = { artifact = "bin", … }` syntax in manifests.

use crate::prelude::*;
use crate::utils::cross_compile::{
    can_run_on_host as cross_compile_can_run_on_host, disabled as cross_compile_disabled,
};
use cargo_test_support::compare::assert_e2e;
use cargo_test_support::registry::{Package, RegistryBuilder};
use cargo_test_support::str;
use cargo_test_support::{
    Project, basic_bin_manifest, basic_manifest, cross_compile, project, publish, registry,
    rustc_host,
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
                edition = "2015"
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
        .masquerade_as_nightly_cargo(&["bindeps"])
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  'unknown' is not a valid artifact specifier

"#]])
        .with_status(101)
        .run();

    fn run_cargo_with_and_without_bindeps_feature(
        p: &Project,
        cmd: &str,
        assert: &dyn Fn(&mut cargo_test_support::Execs),
    ) {
        assert(
            p.cargo(&format!("{} -Z bindeps", cmd))
                .masquerade_as_nightly_cargo(&["bindeps"]),
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
                edition = "2015"
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
            .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  'lib' specifier cannot be used without an 'artifact = …' value (bar)

"#]])
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
                edition = "2015"
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
            .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  'target' specifier cannot be used without an 'artifact = …' value (bar)

"#]])
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
                edition = "2015"
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
        .masquerade_as_nightly_cargo(&["bindeps"])
        .with_stderr_data(str![[r#"
[ERROR] failed to run `rustc` to learn about target-specific information

Caused by:
  process didn't exit successfully: `rustc - --crate-name ___ --print=file-names --target unknown-target-triple [..]` ([EXIT_STATUS]: 1)
  --- stderr
...


"#]])
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
                edition = "2015"
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
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  `artifact = …` requires `-Z bindeps` (bar)

"#]])
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
                edition = "2015"
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
        .masquerade_as_nightly_cargo(&["bindeps"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to latest compatible version
[WARNING] foo v0.0.0 ([ROOT]/foo) ignoring invalid dependency `bar_stable` which is missing a lib target
[ERROR] the crate `foo v0.0.0 ([ROOT]/foo)` depends on crate `bar v0.5.0 ([ROOT]/foo/bar)` multiple times with different names

"#]])
        .run();
}

#[cargo_test]
fn features_are_unified_among_lib_and_bin_dep_of_same_target() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
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
                edition = "2015"
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
                edition = "2015"
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
        .masquerade_as_nightly_cargo(&["bindeps"])
        .with_stderr_data(str![[r#"
[LOCKING] 2 packages to latest compatible versions
[COMPILING] d2 v0.0.1 ([ROOT]/foo/d2)
[COMPILING] d1 v0.0.1 ([ROOT]/foo/d1)
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn features_are_not_unified_among_lib_and_bin_dep_of_different_target() {
    if cross_compile_disabled() {
        return;
    }
    let target = cross_compile::alternate();
    let p = project()
        .file(
            "Cargo.toml",
            &r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
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
                edition = "2015"
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
                edition = "2015"
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
        .masquerade_as_nightly_cargo(&["bindeps"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[LOCKING] 2 packages to latest compatible versions
[COMPILING] d2 v0.0.1 ([ROOT]/foo/d2)
[COMPILING] d1 v0.0.1 ([ROOT]/foo/d1)
error[E0425]: cannot find function `f2` in crate `d2`
...

For more information about this error, try `rustc --explain E0425`.
[ERROR] could not compile `d1` (bin "d1") due to 1 previous error
...

"#]])
        .run();
}

#[cargo_test]
fn feature_resolution_works_for_cfg_target_specification() {
    if cross_compile_disabled() {
        return;
    }
    let target = cross_compile::alternate();
    let p = project()
        .file(
            "Cargo.toml",
            &r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []
                resolver = "2"

                [dependencies.d1]
                path = "d1"
                artifact = "bin"
                target = "$TARGET"
            "#
            .replace("$TARGET", target),
        )
        .file(
            "src/main.rs",
            r#"
                fn main() {
                    let _b = include_bytes!(env!("CARGO_BIN_FILE_D1"));
                }
            "#,
        )
        .file(
            "d1/Cargo.toml",
            &r#"
                [package]
                name = "d1"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [target.'$TARGET'.dependencies]
                d2 = { path = "../d2" }
            "#
            .replace("$TARGET", target),
        )
        .file(
            "d1/src/main.rs",
            r#"fn main() {
                d1::f();
            }"#,
        )
        .file("d1/build.rs", r#"fn main() { }"#)
        .file(
            "d1/src/lib.rs",
            &r#"pub fn f() {
                #[cfg(target = "$TARGET")]
                d2::f();
            }
            "#
            .replace("$TARGET", target),
        )
        .file(
            "d2/Cargo.toml",
            r#"
                [package]
                name = "d2"
                version = "0.0.1"
                edition = "2015"
                authors = []
            "#,
        )
        .file("d2/build.rs", r#"fn main() { }"#)
        .file("d2/src/lib.rs", "pub fn f() {}")
        .build();

    p.cargo("test -Z bindeps")
        .masquerade_as_nightly_cargo(&["bindeps"])
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
                edition = "2015"
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
                edition = "2015"
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
        .masquerade_as_nightly_cargo(&["bindeps"])
        .with_stderr_data(
            str![[r#"
[LOCKING] 1 package to latest compatible version
[COMPILING] bar v0.5.0 ([ROOT]/foo/bar)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[COMPILING] foo v0.0.0 ([ROOT]/foo)

"#]]
            .unordered(),
        )
        .run();

    let build_script_output = build_script_output_string(&p, "foo");
    // we need the binary directory for this artifact along with all binary paths
    if cfg!(target_env = "msvc") {
        assert_e2e().eq(
            &build_script_output,
            str![[r#"
[ROOT]/foo/target/debug/deps/artifact/bar-[HASH]/bin/baz[EXE]
[ROOT]/foo/target/debug/deps/artifact/bar-[HASH]/staticlib/bar-[HASH].lib
[ROOT]/foo/target/debug/deps/artifact/bar-[HASH]/cdylib/bar.dll
[ROOT]/foo/target/debug/deps/artifact/bar-[HASH]/bin
[ROOT]/foo/target/debug/deps/artifact/bar-[HASH]/bin/bar[EXE]
[ROOT]/foo/target/debug/deps/artifact/bar-[HASH]/bin/bar[EXE]

"#]],
        );
    } else {
        assert_e2e().eq(
            &build_script_output,
            str![[r#"
[ROOT]/foo/target/debug/deps/artifact/bar-[HASH]/bin/baz-[HASH][EXE]
[ROOT]/foo/target/debug/deps/artifact/bar-[HASH]/staticlib/libbar-[HASH].a
[ROOT]/foo/target/debug/deps/artifact/bar-[HASH]/cdylib/[..]bar.[..]
[ROOT]/foo/target/debug/deps/artifact/bar-[HASH]/bin
[ROOT]/foo/target/debug/deps/artifact/bar-[HASH]/bin/bar-[HASH][EXE]
[ROOT]/foo/target/debug/deps/artifact/bar-[HASH]/bin/bar-[HASH][EXE]

"#]],
        );
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
                edition = "2015"
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
        .masquerade_as_nightly_cargo(&["bindeps"])
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
                edition = "2015"
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
        .masquerade_as_nightly_cargo(&["bindeps"])
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
                edition = "2015"
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
                edition = "2015"
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
        .masquerade_as_nightly_cargo(&["bindeps"])
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to latest compatible version
[COMPILING] bar-baz v0.5.0 ([ROOT]/foo/bar)
[COMPILING] foo v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    let build_script_output = build_script_output_string(&p, "foo");
    // we need the binary directory for this artifact and the binary itself
    if cfg!(target_env = "msvc") {
        assert_e2e().eq(
            &build_script_output,
            str![[r#"
[ROOT]/foo/target/debug/deps/artifact/bar-baz-[HASH]/bin
[ROOT]/foo/target/debug/deps/artifact/bar-baz-[HASH]/bin/baz_suffix[EXE]

"#]],
        );
    } else {
        assert_e2e().eq(
            &build_script_output,
            str![[r#"
[ROOT]/foo/target/debug/deps/artifact/bar-baz-[HASH]/bin
[ROOT]/foo/target/debug/deps/artifact/bar-baz-[HASH]/bin/baz_suffix-[HASH][EXE]

"#]],
        );
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
                edition = "2015"
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
                let _b = include_bytes!(env!("CARGO_STATICLIB_FILE_BAR_BAZ_bar_baz"));
                let _b = include_bytes!(env!("CARGO_CDYLIB_FILE_BAR_BAZ"));
                let _b = include_bytes!(env!("CARGO_CDYLIB_FILE_BAR_BAZ_bar-baz"));
                let _b = include_bytes!(env!("CARGO_CDYLIB_FILE_BAR_BAZ_bar_baz"));
            }
        "#,
        )
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar-baz"
                version = "0.5.0"
                edition = "2015"
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
        .masquerade_as_nightly_cargo(&["bindeps"])
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to latest compatible version
[COMPILING] bar-baz v0.5.0 ([ROOT]/foo/bar)
[COMPILING] foo v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
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
                edition = "2015"
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
        .masquerade_as_nightly_cargo(&["bindeps"])
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to latest compatible version
[COMPILING] bar v0.5.0 ([ROOT]/foo/bar)
[COMPILING] foo v0.0.0 ([ROOT]/foo)
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] unittests src/lib.rs (target/debug/deps/foo-[HASH][EXE])
[DOCTEST] foo

"#]])
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
                edition = "2015"
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
        .masquerade_as_nightly_cargo(&["bindeps"])
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
                edition = "2015"
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
        .masquerade_as_nightly_cargo(&["bindeps"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to latest compatible version
[ERROR] dependency `bar` in package `foo` requires a `bin:one-example` artifact to be present.

"#]])
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
                edition = "2015"
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
        .masquerade_as_nightly_cargo(&["bindeps"])
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to latest compatible version
[COMPILING] bar v0.5.0 ([ROOT]/foo/bar)
[COMPILING] foo v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn build_script_deps_adopt_specified_target_unconditionally() {
    if cross_compile_disabled() {
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
                edition = "2015"
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
        .masquerade_as_nightly_cargo(&["bindeps"])
        .with_stderr_does_not_contain(
            "[RUNNING] `rustc --crate-name build_script_build --edition=2015 build.rs [..]--target [ALT_TARGET] [..]",
        )
        .with_stderr_contains("[RUNNING] `rustc --crate-name build_script_build --edition=2015 build.rs [..]")
        .with_stderr_contains(
            "[RUNNING] `rustc --crate-name bar --edition=2015 bar/src/lib.rs [..]--target [ALT_TARGET] [..]",
        )
        .with_stderr_contains(
            "[RUNNING] `rustc --crate-name bar --edition=2015 bar/src/main.rs [..]--target [ALT_TARGET] [..]",
        )
        .with_stderr_does_not_contain(
            "[RUNNING] `rustc --crate-name foo [..]--target [ALT_TARGET] [..]",
        )
        .with_stderr_contains("[RUNNING] `rustc --crate-name foo [..]")
        .run();
}

/// inverse RFC-3176
#[cargo_test]
fn build_script_deps_adopt_do_not_allow_multiple_targets_under_different_name_and_same_version() {
    if cross_compile_disabled() {
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
                edition = "2015"
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
        .masquerade_as_nightly_cargo(&["bindeps"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to latest compatible version
[ERROR] the crate `foo v0.0.0 ([ROOT]/foo)` depends on crate `bar v0.5.0 ([ROOT]/foo/bar)` multiple times with different names

"#]])
        .run();
}

#[cargo_test]
fn non_build_script_deps_adopt_specified_target_unconditionally() {
    if cross_compile_disabled() {
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
                edition = "2015"
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
        .masquerade_as_nightly_cargo(&["bindeps"])
        .with_stderr_contains(
            "[RUNNING] `rustc --crate-name bar --edition=2015 bar/src/lib.rs [..]--target [ALT_TARGET] [..]",
        )
        .with_stderr_contains(
            "[RUNNING] `rustc --crate-name bar --edition=2015 bar/src/main.rs [..]--target [ALT_TARGET] [..]",
        )
        .with_stderr_does_not_contain(
            "[RUNNING] `rustc --crate-name foo [..]--target [ALT_TARGET] [..]",
        )
        .with_stderr_contains("[RUNNING] `rustc --crate-name foo [..]")
        .run();
}

#[cargo_test]
fn cross_doctests_works_with_artifacts() {
    if cross_compile_disabled() {
        return;
    }

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
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
        .masquerade_as_nightly_cargo(&["bindeps"])
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to latest compatible version
[COMPILING] bar v0.5.0 ([ROOT]/foo/bar)
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] unittests src/lib.rs (target/[HOST_TARGET]/debug/deps/foo-[HASH][EXE])
[DOCTEST] foo

"#]])
        .run();

    println!("c");
    let target = cross_compile::alternate();

    if !cross_compile_can_run_on_host() {
        return;
    }

    p.cargo("test -Z bindeps -v --target")
        .arg(&target)
        .masquerade_as_nightly_cargo(&["bindeps"])
        .with_stderr_data(str![[r#"
[COMPILING] bar v0.5.0 ([ROOT]/foo/bar)
[RUNNING] `rustc --crate-name bar --edition=2015 bar/src/lib.rs [..]--target [ALT_TARGET] [..]
[RUNNING] `rustc --crate-name bar --edition=2015 bar/src/main.rs [..]--target [ALT_TARGET] [..]
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo [..]
[RUNNING] `rustc --crate-name foo [..]
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `[ROOT]/foo/target/[ALT_TARGET]/debug/deps/foo-[HASH][EXE]`
[DOCTEST] foo
[RUNNING] `rustdoc [..]--test src/lib.rs --test-run-directory [ROOT]/foo --target [ALT_TARGET] [..]

"#]])
        .run();
}

#[cargo_test]
fn build_script_deps_adopts_target_platform_if_target_equals_target() {
    if cross_compile_disabled() {
        return;
    }

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.0"
                edition = "2015"
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
        .masquerade_as_nightly_cargo(&["bindeps"])
        .with_stderr_does_not_contain(
            "[RUNNING] `rustc --crate-name build_script_build --edition=2015 build.rs [..]--target [ALT_TARGET] [..]",
        )
        .with_stderr_contains("[RUNNING] `rustc --crate-name build_script_build --edition=2015 build.rs [..]")
        .with_stderr_contains(
            "[RUNNING] `rustc --crate-name bar --edition=2015 bar/src/lib.rs [..]--target [ALT_TARGET] [..]",
        )
        .with_stderr_contains(
            "[RUNNING] `rustc --crate-name bar --edition=2015 bar/src/main.rs [..]--target [ALT_TARGET] [..]",
        )
        .with_stderr_contains(
            "[RUNNING] `rustc --crate-name foo [..]--target [ALT_TARGET] [..]",
        )
        .run();
}

#[cargo_test]
// TODO(ST): rename bar (dependency) to something else and un-ignore this with RFC-3176
#[cfg_attr(target_env = "msvc", ignore = "msvc not working")]
fn profile_override_basic() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
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
        .masquerade_as_nightly_cargo(&["bindeps"])
        .with_stderr_data(
            str![[r#"
[LOCKING] 1 package to latest compatible version
[COMPILING] bar v0.5.0 ([ROOT]/foo/bar)
[RUNNING] `rustc --crate-name build_script_build [..] -C opt-level=1 [..]`
[RUNNING] `rustc --crate-name bar --edition=2015 bar/src/main.rs [..] -C opt-level=3 [..]`
[RUNNING] `rustc --crate-name bar --edition=2015 bar/src/main.rs [..] -C opt-level=1 [..]`
[RUNNING] `rustc --crate-name bar --edition=2015 bar/src/lib.rs [..] -C opt-level=1 [..]`
[RUNNING] `rustc --crate-name bar --edition=2015 bar/src/lib.rs [..] -C opt-level=3 [..]`
[RUNNING] `rustc --crate-name foo [..] -C opt-level=3 [..]`
[RUNNING] `[ROOT]/foo/target/debug/build/foo-[HASH]/build-script-build`
[FINISHED] `dev` profile [optimized + debuginfo] target(s) in [ELAPSED]s
[COMPILING] foo v0.0.1 ([ROOT]/foo)

"#]]
            .unordered(),
        )
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
                edition = "2015"
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
                edition = "2015"
                authors = []

                [dependencies]
                baz = "1.0.0"
            "#,
        )
        .file("bar/src/lib.rs", r#"pub fn bar() {baz::baz()}"#)
        .file("bar/src/main.rs", r#"fn main() {bar::bar()}"#)
        .build();
    p.cargo("build -Z bindeps")
        .masquerade_as_nightly_cargo(&["bindeps"])
        .run();

    // cargo tree sees artifacts as the dependency kind they are in and doesn't do anything special with it.
    p.cargo("tree -Z bindeps")
        .masquerade_as_nightly_cargo(&["bindeps"])
        .with_stdout_data(str![[r#"
foo v0.0.0 ([ROOT]/foo)
[build-dependencies]
└── bar v0.5.0 ([ROOT]/foo/bar)
    └── baz v1.0.0

"#]])
        .run();
}

#[cargo_test]
fn artifact_dep_target_specified() {
    if cross_compile_disabled() {
        return;
    }
    let target = cross_compile::alternate();

    let p = project()
        .file(
            "Cargo.toml",
            &r#"
                [package]
                name = "foo"
                version = "0.0.0"
                authors = []
                resolver = "2"
                edition = "2015"

                [dependencies]
                bindep = { path = "bindep", artifact = "bin", target = "$TARGET" }
            "#
            .replace("$TARGET", target),
        )
        .file("src/lib.rs", "")
        .file("bindep/Cargo.toml", &basic_manifest("bindep", "0.0.0"))
        .file("bindep/src/main.rs", "fn main() {}")
        .build();

    p.cargo("check -Z bindeps")
        .masquerade_as_nightly_cargo(&["bindeps"])
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to latest compatible version
[COMPILING] bindep v0.0.0 ([ROOT]/foo/bindep)
[CHECKING] foo v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .with_status(0)
        .run();

    p.cargo("tree -Z bindeps")
        .masquerade_as_nightly_cargo(&["bindeps"])
        .with_stdout_data(str![[r#"
foo v0.0.0 ([ROOT]/foo)
└── bindep v0.0.0 ([ROOT]/foo/bindep)

"#]])
        .with_status(0)
        .run();
}

/// From issue #10593
/// The case where:
/// *   artifact dep is { target = <specified> }
/// *   dependency of that artifact dependency specifies the same target
/// *   the target is not activated.
#[cargo_test]
fn dep_of_artifact_dep_same_target_specified() {
    if cross_compile_disabled() {
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
                    version = "0.1.0"
                    edition = "2015"
                    resolver = "2"

                    [dependencies]
                    bar = {{ path = "bar", artifact = "bin", target = "{target}" }}
                "#,
            ),
        )
        .file("src/lib.rs", "")
        .file(
            "bar/Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "bar"
                    version = "0.1.0"

                    [target.{target}.dependencies]
                    baz = {{ path = "../baz" }}
                "#,
            ),
        )
        .file("bar/src/main.rs", "fn main() {}")
        .file(
            "baz/Cargo.toml",
            r#"
                [package]
                name = "baz"
                version = "0.1.0"

            "#,
        )
        .file("baz/src/lib.rs", "")
        .build();

    p.cargo("check -Z bindeps")
        .masquerade_as_nightly_cargo(&["bindeps"])
        .with_stderr_data(str![[r#"
[LOCKING] 2 packages to latest compatible versions
[COMPILING] baz v0.1.0 ([ROOT]/foo/baz)
[COMPILING] bar v0.1.0 ([ROOT]/foo/bar)
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .with_status(0)
        .run();

    p.cargo("tree -Z bindeps")
        .masquerade_as_nightly_cargo(&["bindeps"])
        .with_stdout_data(
            r#"...
foo v0.1.0 ([ROOT]/foo)
└── bar v0.1.0 ([ROOT]/foo/bar)
    └── baz v0.1.0 ([ROOT]/foo/baz)
"#,
        )
        .with_status(0)
        .run();
}

#[cargo_test]
fn targets_are_picked_up_from_non_workspace_artifact_deps() {
    if cross_compile_disabled() {
        return;
    }
    let target = cross_compile::alternate();
    Package::new("artifact", "1.0.0")
        .file("src/main.rs", r#"fn main() {}"#)
        .file("src/lib.rs", r#"pub fn lib() {}"#)
        .publish();

    let mut dep = registry::Dependency::new("artifact", "1.0.0");
    Package::new("uses-artifact", "1.0.0")
        .schema_version(3)
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
                edition = "2015"
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
        .masquerade_as_nightly_cargo(&["bindeps"])
        .run();
}

#[cargo_test]
fn index_version_filtering() {
    if cross_compile_disabled() {
        return;
    }
    let target = cross_compile::alternate();

    Package::new("artifact", "1.0.0")
        .file("src/main.rs", r#"fn main() {}"#)
        .file("src/lib.rs", r#"pub fn lib() {}"#)
        .publish();

    let mut dep = registry::Dependency::new("artifact", "1.0.0");

    Package::new("bar", "1.0.0").publish();
    Package::new("bar", "1.0.1")
        .schema_version(3)
        .add_dep(dep.artifact("bin", Some(target.to_string())))
        .publish();

    // Verify that without `-Zbindeps` that it does not use 1.0.1.
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
        .file("src/lib.rs", "")
        .build();

    p.cargo("tree")
        .with_stdout_data(str![[r#"
foo v0.1.0 ([ROOT]/foo)
└── bar v1.0.0

"#]])
        .run();

    // And with -Zbindeps it can use 1.0.1.
    p.cargo("update -Zbindeps")
        .masquerade_as_nightly_cargo(&["bindeps"])
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 2 packages to latest compatible versions
[ADDING] artifact v1.0.0
[UPDATING] bar v1.0.0 -> v1.0.1

"#]])
        .run();

    // And without -Zbindeps, now that 1.0.1 is in Cargo.lock, it should fail.
    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[ERROR] failed to select a version for the requirement `bar = "^1.0"` (locked to 1.0.1)
  version 1.0.1 requires a Cargo version that supports index version 3
location searched: `dummy-registry` index (which is replacing registry `crates-io`)
required by package `foo v0.1.0 ([ROOT]/foo)`

"#]])
        .run();
}

#[cargo_test]
fn proc_macro_in_artifact_dep() {
    // Forcing FeatureResolver to check a proc-macro for a dependency behind a
    // target dependency.
    if cross_compile_disabled() {
        return;
    }
    Package::new("pm", "1.0.0")
        .file("src/lib.rs", "")
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "pm"
                version = "1.0.0"
                edition = "2015"

                [lib]
                proc-macro = true

            "#,
        )
        .publish();
    let alternate = cross_compile::alternate();
    Package::new("bin-uses-pm", "1.0.0")
        .target_dep("pm", "1.0", alternate)
        .file("src/main.rs", "fn main() {}")
        .publish();
    // Simulate a network error downloading the proc-macro.
    std::fs::remove_file(cargo_test_support::paths::root().join("dl/pm/1.0.0/download")).unwrap();
    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2021"

                [dependencies]
                bin-uses-pm = {{ version = "1.0", artifact = "bin", target = "{alternate}"}}
            "#
            ),
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Z bindeps")
        .masquerade_as_nightly_cargo(&["bindeps"])
        .with_stderr_data(
            r#"...
[UPDATING] `dummy-registry` index
[LOCKING] 2 packages to latest compatible versions
[DOWNLOADING] crates ...
[ERROR] failed to download from `[ROOTURL]/dl/pm/1.0.0/download`

Caused by:
  [37] Could[..]t read a file:// file (Couldn't open file [ROOT]/dl/pm/1.0.0/download)
"#,
        )
        .with_status(101)
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
                edition = "2015"
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
        .masquerade_as_nightly_cargo(&["bindeps"])
        .with_stderr_data(
            str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 2 packages to latest compatible versions
[DOWNLOADING] crates ...
[DOWNLOADED] bar v1.0.0 (registry `dummy-registry`)
[COMPILING] bar v1.0.0
[COMPILING] bar v0.5.0 ([ROOT]/foo/bar)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[COMPILING] foo v0.0.0 ([ROOT]/foo)

"#]]
            .unordered(),
        )
        .run();
    let build_script_output = build_script_output_string(&p, "foo");
    assert_e2e().eq(
        &build_script_output,
        str![[r#"
0.5.0
1.0.0

"#]],
    );
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
                edition = "2015"
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
        .masquerade_as_nightly_cargo(&["bindeps"])
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to latest compatible version
[COMPILING] bar v0.0.1 ([ROOT]/foo/bar)
[COMPILING] foo v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
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
                edition = "2015"
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
        .masquerade_as_nightly_cargo(&["bindeps"])
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to latest compatible version
[COMPILING] bar v0.5.0 ([ROOT]/foo/bar)
[CHECKING] foo v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
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
                edition = "2015"
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
        .masquerade_as_nightly_cargo(&["bindeps"])
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to latest compatible version
[WARNING] foo v0.0.0 ([ROOT]/foo) ignoring invalid dependency `bar` which is missing a lib target
[COMPILING] bar v0.5.0 ([ROOT]/foo/bar)
[CHECKING] foo v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
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
        .masquerade_as_nightly_cargo(&["bindeps"])
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
                        edition = "2015"
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
            .masquerade_as_nightly_cargo(&["bindeps"])
            .with_status(101)
            .with_stderr_data(str![[r#"
[LOCKING] 1 package to latest compatible version
[ERROR] dependency `bar` in package `foo` requires a [..] artifact to be present.

"#]])
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
                edition = "2015"
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
        .masquerade_as_nightly_cargo(&["bindeps"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  `target = "target"` in normal- or dev-dependencies has no effect (bar)

"#]])
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
                edition = "2015"
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
                edition = "2015"
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
        .masquerade_as_nightly_cargo(&["bindeps"])
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to latest compatible version
[COMPILING] bar v0.5.0 ([ROOT]/foo/bar)
[COMPILING] foo v0.0.0 ([ROOT]/foo)
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] unittests src/lib.rs (target/debug/deps/foo-[HASH][EXE])
[RUNNING] tests/main.rs (target/debug/deps/main-[HASH][EXE])
[DOCTEST] foo

"#]])
        .run();
}

#[cargo_test]
fn publish_artifact_dep() {
    let registry = RegistryBuilder::new().http_api().http_index().build();

    Package::new("bar", "1.0.0").publish();
    Package::new("baz", "1.0.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            edition = "2015"
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

    p.cargo("publish -Z bindeps --no-verify")
        .replace_crates_io(registry.index_url())
        .masquerade_as_nightly_cargo(&["bindeps"])
        .with_stderr_data(str![[r#"
[UPDATING] crates.io index
[PACKAGING] foo v0.1.0 ([ROOT]/foo)
[UPDATING] crates.io index
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[UPLOADING] foo v0.1.0 ([ROOT]/foo)
[UPLOADED] foo v0.1.0 to registry `crates-io`
[NOTE] waiting for foo v0.1.0 to be available at registry `crates-io`
[HELP] you may press ctrl-c to skip waiting; the crate should be available shortly
[PUBLISHED] foo v0.1.0 at registry `crates-io`

"#]])
        .run();

    publish::validate_upload_with_contents(
        r#"
        {
          "authors": [],
          "badges": {},
          "categories": [],
          "deps": [{
              "artifact": ["bin"],
              "default_features": true,
              "features": [],
              "kind": "normal",
              "lib": true,
              "name": "bar",
              "optional": false,
              "target": null,
              "version_req": "^1.0"
            },
            {
              "artifact": [
                "bin:a",
                "cdylib",
                "staticlib"
              ],
              "bindep_target": "target",
              "default_features": true,
              "features": [],
              "kind": "build",
              "name": "baz",
              "optional": false,
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
          "rust_version": null,
          "vers": "0.1.0"
        }
        "#,
        "foo-0.1.0.crate",
        &["Cargo.toml", "Cargo.toml.orig", "src/lib.rs", "Cargo.lock"],
        [(
            "Cargo.toml",
            str![[r##"
# THIS FILE IS AUTOMATICALLY GENERATED BY CARGO
#
# When uploading crates to the registry Cargo will automatically
# "normalize" Cargo.toml files for maximal compatibility
# with all versions of Cargo and also rewrite `path` dependencies
# to registry (e.g., crates.io) dependencies.
#
# If you are reading this file be aware that the original Cargo.toml
# will likely look very different (and much more reasonable).
# See Cargo.toml.orig for the original contents.

[package]
edition = "2015"
name = "foo"
version = "0.1.0"
authors = []
build = false
autolib = false
autobins = false
autoexamples = false
autotests = false
autobenches = false
description = "foo"
homepage = "foo"
documentation = "foo"
readme = false
license = "MIT"
repository = "foo"
resolver = "2"

[lib]
name = "foo"
path = "src/lib.rs"

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
target = "target"

"##]],
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
                edition = "2015"
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
        .masquerade_as_nightly_cargo(&["bindeps"])
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to latest compatible version
[COMPILING] bar v0.0.1 ([ROOT]/foo/bar)
[DOCUMENTING] bar v0.0.1 ([ROOT]/foo/bar)
[DOCUMENTING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[GENERATED] [ROOT]/foo/target/doc/foo/index.html

"#]])
        .run();

    assert!(p.root().join("target/doc").is_dir());
    assert!(p.root().join("target/doc/foo/index.html").is_file());
    assert!(p.root().join("target/doc/bar/index.html").is_file());

    // Verify that it emits rmeta for the bin and lib dependency.
    assert_eq!(p.glob("target/debug/artifact/*.rlib").count(), 0);
    assert_eq!(p.glob("target/debug/deps/libbar-*.rmeta").count(), 2);

    p.cargo("doc -Z bindeps")
        .masquerade_as_nightly_cargo(&["bindeps"])
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[GENERATED] [ROOT]/foo/target/doc/foo/index.html

"#]])
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
                edition = "2015"
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
                edition = "2015"
                authors = []

                [lib]
                crate-type = ["staticlib", "cdylib"]
            "#,
        )
        .file("bar/src/lib.rs", "pub fn bar() {}")
        .file("bar/src/main.rs", "fn main() {}")
        .build();

    p.cargo("doc -Z bindeps")
        .masquerade_as_nightly_cargo(&["bindeps"])
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to latest compatible version
[COMPILING] bar v0.5.0 ([ROOT]/foo/bar)
[DOCUMENTING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[GENERATED] [ROOT]/foo/target/doc/foo/index.html

"#]])
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

#[cargo_test]
fn build_script_features_for_shared_dependency() {
    // When a build script is built and run, its features should match. Here:
    //
    // foo
    //   -> artifact on d1 with target
    //   -> common with features f1
    //
    // d1
    //   -> common with features f2
    //
    // common has features f1 and f2, with a build script.
    //
    // When common is built as a dependency of d1, it should have features
    // `f2` (for the library and the build script).
    //
    // When common is built as a dependency of foo, it should have features
    // `f1` (for the library and the build script).
    if cross_compile_disabled() {
        return;
    }
    let target = cross_compile::alternate();
    let p = project()
        .file(
            "Cargo.toml",
            &r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                resolver = "2"

                [dependencies]
                d1 = { path = "d1", artifact = "bin", target = "$TARGET" }
                common = { path = "common", features = ["f1"] }
            "#
            .replace("$TARGET", target),
        )
        .file(
            "src/main.rs",
            r#"
                fn main() {
                    let _b = include_bytes!(env!("CARGO_BIN_FILE_D1"));
                    common::f1();
                }
            "#,
        )
        .file(
            "d1/Cargo.toml",
            r#"
                [package]
                name = "d1"
                version = "0.0.1"
                edition = "2015"

                [dependencies]
                common = { path = "../common", features = ["f2"] }
            "#,
        )
        .file(
            "d1/src/main.rs",
            r#"fn main() {
                common::f2();
            }"#,
        )
        .file(
            "common/Cargo.toml",
            r#"
                [package]
                name = "common"
                version = "0.0.1"
                edition = "2015"

                [features]
                f1 = []
                f2 = []
            "#,
        )
        .file(
            "common/src/lib.rs",
            r#"
                #[cfg(feature = "f1")]
                pub fn f1() {}

                #[cfg(feature = "f2")]
                pub fn f2() {}
            "#,
        )
        .file(
            "common/build.rs",
            &r#"
                use std::env::var_os;
                fn main() {
                    assert_eq!(var_os("CARGO_FEATURE_F1").is_some(), cfg!(feature="f1"));
                    assert_eq!(var_os("CARGO_FEATURE_F2").is_some(), cfg!(feature="f2"));
                    if std::env::var("TARGET").unwrap() == "$TARGET" {
                        assert!(var_os("CARGO_FEATURE_F1").is_none());
                        assert!(var_os("CARGO_FEATURE_F2").is_some());
                    } else {
                        assert!(var_os("CARGO_FEATURE_F1").is_some());
                        assert!(var_os("CARGO_FEATURE_F2").is_none());
                    }
                }
            "#
            .replace("$TARGET", target),
        )
        .build();

    p.cargo("build -Z bindeps -v")
        .masquerade_as_nightly_cargo(&["bindeps"])
        .run();
}

#[cargo_test]
fn calc_bin_artifact_fingerprint() {
    // See rust-lang/cargo#10527
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"
                resolver = "2"

                [dependencies]
                bar = { path = "bar/", artifact = "bin" }
            "#,
        )
        .file(
            "src/main.rs",
            r#"
                fn main() {
                    let _b = include_bytes!(env!("CARGO_BIN_FILE_BAR"));
                }
            "#,
        )
        .file("bar/Cargo.toml", &basic_bin_manifest("bar"))
        .file("bar/src/main.rs", r#"fn main() { println!("foo") }"#)
        .build();
    p.cargo("check -Z bindeps")
        .masquerade_as_nightly_cargo(&["bindeps"])
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to latest compatible version
[COMPILING] bar v0.5.0 ([ROOT]/foo/bar)
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.change_file("bar/src/main.rs", r#"fn main() { println!("bar") }"#);
    // Change in artifact bin dep `bar` propagates to `foo`, triggering recompile.
    p.cargo("check -v -Z bindeps")
        .masquerade_as_nightly_cargo(&["bindeps"])
        .with_stderr_data(str![[r#"
[DIRTY] bar v0.5.0 ([ROOT]/foo/bar): the file `bar/src/main.rs` has changed ([..])
[COMPILING] bar v0.5.0 ([ROOT]/foo/bar)
[RUNNING] `rustc --crate-name bar [..]`
[DIRTY] foo v0.1.0 ([ROOT]/foo): the dependency bar was rebuilt
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    // All units are fresh. No recompile.
    p.cargo("check -v -Z bindeps")
        .masquerade_as_nightly_cargo(&["bindeps"])
        .with_stderr_data(str![[r#"
[FRESH] bar v0.5.0 ([ROOT]/foo/bar)
[FRESH] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn with_target_and_optional() {
    // See rust-lang/cargo#10526
    if cross_compile_disabled() {
        return;
    }
    let target = cross_compile::alternate();
    let p = project()
        .file(
            "Cargo.toml",
            &r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2021"
                [dependencies]
                d1 = { path = "d1", artifact = "bin", optional = true, target = "$TARGET" }
            "#
            .replace("$TARGET", target),
        )
        .file(
            "src/main.rs",
            r#"
                fn main() {
                    let _b = include_bytes!(env!("CARGO_BIN_FILE_D1"));
                }
            "#,
        )
        .file(
            "d1/Cargo.toml",
            r#"
                [package]
                name = "d1"
                version = "0.0.1"
                edition = "2021"
            "#,
        )
        .file("d1/src/main.rs", "fn main() {}")
        .build();

    p.cargo("check -Z bindeps -F d1 -v")
        .masquerade_as_nightly_cargo(&["bindeps"])
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to latest compatible version
[COMPILING] d1 v0.0.1 ([ROOT]/foo/d1)
[RUNNING] `rustc --crate-name d1 [..]--crate-type bin[..]
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo [..]--cfg[..]d1[..]
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn with_assumed_host_target_and_optional_build_dep() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2021"
                [build-dependencies]
                d1 = { path = "d1", artifact = "bin", optional = true, target = "target" }
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "build.rs",
            r#"
                fn main() {
                    std::env::var("CARGO_BIN_FILE_D1").unwrap();
                }
            "#,
        )
        .file(
            "d1/Cargo.toml",
            r#"
                [package]
                name = "d1"
                version = "0.0.1"
                edition = "2021"
            "#,
        )
        .file("d1/src/main.rs", "fn main() {}")
        .build();

    p.cargo("check -Z bindeps -F d1 -v")
        .masquerade_as_nightly_cargo(&["bindeps"])
        .with_stderr_data(
            str![[r#"
[LOCKING] 1 package to latest compatible version
[COMPILING] d1 v0.0.1 ([ROOT]/foo/d1)
[RUNNING] `rustc --crate-name build_script_build --edition=2021 [..]--crate-type bin[..]
[RUNNING] `rustc --crate-name d1 --edition=2021 [..]--crate-type bin[..]
[RUNNING] `[ROOT]/foo/target/debug/build/foo-[HASH]/build-script-build`
[RUNNING] `rustc --crate-name foo --edition=2021 [..]--cfg[..]d1[..]
[FINISHED] `dev` profile [..]
[COMPILING] foo v0.0.1 ([ROOT]/foo)

"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test]
fn decouple_same_target_transitive_dep_from_artifact_dep() {
    // See https://github.com/rust-lang/cargo/issues/11463
    let target = rustc_host();
    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2021"

                [dependencies]
                a = {{ path = "a" }}
                bar = {{ path = "bar", artifact = "bin", target = "{target}" }}
            "#
            ),
        )
        .file(
            "src/main.rs",
            r#"
                fn main() {}
            "#,
        )
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.1.0"
                edition = "2015"

                [dependencies]
                a = { path = "../a", features = ["feature"] }
            "#,
        )
        .file(
            "bar/src/main.rs",
            r#"
                fn main() {}
            "#,
        )
        .file(
            "a/Cargo.toml",
            r#"
                [package]
                name = "a"
                version = "0.1.0"
                edition = "2021"

                [dependencies]
                b = { path = "../b" }
                c = { path = "../c" }

                [features]
                feature = ["c/feature"]
            "#,
        )
        .file(
            "a/src/lib.rs",
            r#"
                use b::Trait as _;

                pub fn use_b_trait(x: &impl c::Trait) {
                    x.b();
                }
            "#,
        )
        .file(
            "b/Cargo.toml",
            r#"
                [package]
                name = "b"
                version = "0.1.0"
                edition = "2015"

                [dependencies]
                c = { path = "../c" }
            "#,
        )
        .file(
            "b/src/lib.rs",
            r#"
                pub trait Trait {
                    fn b(&self) {}
                }

                impl<T: c::Trait> Trait for T {}
            "#,
        )
        .file(
            "c/Cargo.toml",
            r#"
                [package]
                name = "c"
                version = "0.1.0"
                edition = "2015"

                [features]
                feature = []
            "#,
        )
        .file(
            "c/src/lib.rs",
            r#"
                pub trait Trait {}
            "#,
        )
        .build();
    p.cargo("build -Z bindeps")
        .masquerade_as_nightly_cargo(&["bindeps"])
        .with_stderr_data(str![[r#"
[LOCKING] 4 packages to latest compatible versions
[COMPILING] c v0.1.0 ([ROOT]/foo/c)
[COMPILING] b v0.1.0 ([ROOT]/foo/b)
[COMPILING] a v0.1.0 ([ROOT]/foo/a)
[COMPILING] bar v0.1.0 ([ROOT]/foo/bar)
[COMPILING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn decouple_same_target_transitive_dep_from_artifact_dep_lib() {
    // See https://github.com/rust-lang/cargo/issues/10837
    let target = rustc_host();
    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2021"

                [dependencies]
                a = {{ path = "a" }}
                b = {{ path = "b", features = ["feature"] }}
                bar = {{ path = "bar", artifact = "bin", lib = true, target = "{target}" }}
            "#
            ),
        )
        .file("src/lib.rs", "")
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.1.0"
                edition = "2021"

                [dependencies]
                a = { path = "../a", features = ["b"] }
                b = { path = "../b" }
            "#,
        )
        .file("bar/src/lib.rs", "")
        .file(
            "bar/src/main.rs",
            r#"
                use b::Trait;

                fn main() {
                    a::A.b()
                }
            "#,
        )
        .file(
            "a/Cargo.toml",
            r#"
                [package]
                name = "a"
                version = "0.1.0"
                edition = "2015"

                [dependencies]
                b = { path = "../b", optional = true }
            "#,
        )
        .file(
            "a/src/lib.rs",
            r#"
                pub struct A;

                #[cfg(feature = "b")]
                impl b::Trait for A {}
            "#,
        )
        .file(
            "b/Cargo.toml",
            r#"
                [package]
                name = "b"
                version = "0.1.0"
                edition = "2015"

                [features]
                feature = []
            "#,
        )
        .file(
            "b/src/lib.rs",
            r#"
                pub trait Trait {
                    fn b(&self) {}
                }
            "#,
        )
        .build();
    p.cargo("build -Z bindeps")
        .masquerade_as_nightly_cargo(&["bindeps"])
        .with_stderr_data(str![[r#"
[LOCKING] 3 packages to latest compatible versions
[COMPILING] b v0.1.0 ([ROOT]/foo/b)
[COMPILING] a v0.1.0 ([ROOT]/foo/a)
[COMPILING] bar v0.1.0 ([ROOT]/foo/bar)
[COMPILING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn decouple_same_target_transitive_dep_from_artifact_dep_and_proc_macro() {
    let target = rustc_host();
    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2021"

                [dependencies]
                c = {{ path = "c" }}
                bar = {{ path = "bar", artifact = "bin", target = "{target}" }}
            "#
            ),
        )
        .file("src/lib.rs", "")
        .file(
            "bar/Cargo.toml",
            r#"
            [package]
            name = "bar"
            version = "0.1.0"
            edition = "2015"

            [dependencies]
            b = { path = "../b" }
            "#,
        )
        .file("bar/src/main.rs", "fn main() {}")
        .file(
            "b/Cargo.toml",
            r#"
            [package]
            name = "b"
            version = "0.1.0"
            edition = "2021"

            [dependencies]
            a = { path = "../a" }

            [lib]
            proc-macro = true
            "#,
        )
        .file("b/src/lib.rs", "")
        .file(
            "c/Cargo.toml",
            r#"
            [package]
            name = "c"
            version = "0.1.0"
            edition = "2021"

            [dependencies]
            d = { path = "../d", features = ["feature"] }
            a = { path = "../a" }

            [lib]
            proc-macro = true
            "#,
        )
        .file(
            "c/src/lib.rs",
            r#"
            use a::Trait;

            fn _c() {
                d::D.a()
            }
            "#,
        )
        .file(
            "a/Cargo.toml",
            r#"
            [package]
            name = "a"
            version = "0.1.0"
            edition = "2015"

            [dependencies]
            d = { path = "../d" }
            "#,
        )
        .file(
            "a/src/lib.rs",
            r#"
            pub trait Trait {
                fn a(&self) {}
            }

            impl Trait for d::D {}
            "#,
        )
        .file(
            "d/Cargo.toml",
            r#"
            [package]
            name = "d"
            version = "0.1.0"
            edition = "2015"

            [features]
            feature = []
            "#,
        )
        .file("d/src/lib.rs", "pub struct D;")
        .build();

    p.cargo("build -Z bindeps")
        .masquerade_as_nightly_cargo(&["bindeps"])
        .with_stderr_data(
            str![[r#"
[LOCKING] 5 packages to latest compatible versions
[COMPILING] d v0.1.0 ([ROOT]/foo/d)
[COMPILING] a v0.1.0 ([ROOT]/foo/a)
[COMPILING] b v0.1.0 ([ROOT]/foo/b)
[COMPILING] c v0.1.0 ([ROOT]/foo/c)
[COMPILING] bar v0.1.0 ([ROOT]/foo/bar)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [..]
[COMPILING] foo v0.1.0 ([ROOT]/foo)

"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test]
fn same_target_artifact_dep_sharing() {
    let target = rustc_host();
    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

                [dependencies]
                a = {{ path = "a" }}
                bar = {{ path = "bar", artifact = "bin", target = "{target}" }}
            "#
            ),
        )
        .file("src/lib.rs", "")
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.1.0"
                edition = "2015"

                [dependencies]
                a = { path = "../a" }
            "#,
        )
        .file(
            "bar/src/main.rs",
            r#"
                fn main() {}
            "#,
        )
        .file(
            "a/Cargo.toml",
            r#"
                [package]
                name = "a"
                version = "0.1.0"
                edition = "2015"
            "#,
        )
        .file("a/src/lib.rs", "")
        .build();
    p.cargo(&format!("build -Z bindeps --target {target}"))
        .masquerade_as_nightly_cargo(&["bindeps"])
        .with_stderr_data(str![[r#"
[LOCKING] 2 packages to latest compatible versions
[COMPILING] a v0.1.0 ([ROOT]/foo/a)
[COMPILING] bar v0.1.0 ([ROOT]/foo/bar)
[COMPILING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn check_transitive_artifact_dependency_with_different_target() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.0"
                edition = "2015"

                [dependencies]
                bar = { path = "bar/" }
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.0.0"
                edition = "2015"

                [dependencies]
                baz = { path = "baz/", artifact = "bin", target = "custom-target" }
            "#,
        )
        .file("bar/src/lib.rs", "")
        .file(
            "bar/baz/Cargo.toml",
            r#"
                [package]
                name = "baz"
                version = "0.0.0"
                edition = "2015"

                [dependencies]
            "#,
        )
        .file("bar/baz/src/main.rs", "fn main() {}")
        .build();

    p.cargo("check -Z bindeps")
        .masquerade_as_nightly_cargo(&["bindeps"])
        .with_stderr_data(str![[r#"
[LOCKING] 2 packages to latest compatible versions
[ERROR] failed to determine target information for target `custom-target`.
  Artifact dependency `baz` in package `bar v0.0.0 ([ROOT]/foo/bar)` requires building for `custom-target`

Caused by:
  failed to run `rustc` to learn about target-specific information

Caused by:
  process didn't exit successfully: `rustc [..] ([EXIT_STATUS]: 1)
  --- stderr
...


"#]])
        .with_status(101)
        .run();
}

#[cargo_test]
fn build_only_specified_artifact_library() {
    // Create a project with:
    // - A crate `bar` with both `staticlib` and `cdylib` as crate-types.
    // - A crate `foo` which depends on either the `staticlib` or `cdylib` artifact of bar,
    //   whose build-script simply checks which library artifacts are present.
    let create_project = |artifact_lib| {
        project()
            .file(
                "bar/Cargo.toml",
                r#"
                [package]
                name = "bar"
                version = "1.0.0"

                [lib]
                crate-type = ["staticlib", "cdylib"]
                "#,
            )
            .file("bar/src/lib.rs", "")
            .file(
                "Cargo.toml",
                &format!(
                r#"
                [package]
                name = "foo"
                version = "1.0.0"

                [build-dependencies]
                bar = {{ path = "bar", artifact = "{artifact_lib}" }}
            "#),
            )
            .file("src/lib.rs", "")
            .file(
                "build.rs",
                r#"
                fn main() {
                    println!("cdylib present: {}", std::env::var_os("CARGO_CDYLIB_FILE_BAR").is_some());
                    println!("staticlib present: {}", std::env::var_os("CARGO_STATICLIB_FILE_BAR").is_some());
                }
            "#,
            )
            .build()
    };

    let cdylib = create_project("cdylib");
    cdylib
        .cargo("build -Z bindeps")
        .masquerade_as_nightly_cargo(&["bindeps"])
        .run();
    assert_e2e().eq(
        &build_script_output_string(&cdylib, "foo"),
        str![[r#"
cdylib present: true
staticlib present: false

"#]],
    );

    let staticlib = create_project("staticlib");
    staticlib
        .cargo("build -Z bindeps")
        .masquerade_as_nightly_cargo(&["bindeps"])
        .run();
    assert_e2e().eq(
        &build_script_output_string(&staticlib, "foo"),
        str![[r#"
cdylib present: false
staticlib present: true

"#]],
    );
}
