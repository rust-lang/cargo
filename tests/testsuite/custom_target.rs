//! Tests for custom json target specifications.

use cargo_test_support::{basic_manifest, project};
use std::fs;

const MINIMAL_LIB: &str = r#"
#![allow(internal_features)]
#![feature(no_core)]
#![feature(lang_items)]
#![no_core]

#[lang = "sized"]
pub trait Sized {
    // Empty.
}
#[lang = "copy"]
pub trait Copy {
    // Empty.
}
"#;

const SIMPLE_SPEC: &str = r#"
{
    "llvm-target": "x86_64-unknown-none-gnu",
    "data-layout": "e-m:e-i64:64-f80:128-n8:16:32:64-S128",
    "arch": "x86_64",
    "target-endian": "little",
    "target-pointer-width": "64",
    "target-c-int-width": "32",
    "os": "none",
    "linker-flavor": "ld.lld",
    "linker": "rust-lld",
    "executables": true
}
"#;

#[cargo_test(nightly, reason = "requires features no_core, lang_items")]
fn custom_target_minimal() {
    let p = project()
        .file(
            "src/lib.rs",
            &"
                __MINIMAL_LIB__

                pub fn foo() -> u32 {
                    42
                }
            "
            .replace("__MINIMAL_LIB__", MINIMAL_LIB),
        )
        .file("custom-target.json", SIMPLE_SPEC)
        .build();

    p.cargo("build --lib --target custom-target.json -v").run();
    p.cargo("build --lib --target src/../custom-target.json -v")
        .run();

    // Ensure that the correct style of flag is passed to --target with doc tests.
    p.cargo("test --doc --target src/../custom-target.json -v -Zdoctest-xcompile")
        .masquerade_as_nightly_cargo(&["doctest-xcompile", "no_core", "lang_items"])
        .with_stderr_contains("[RUNNING] `rustdoc [..]--target [..]foo/custom-target.json[..]")
        .run();
}

#[cargo_test(nightly, reason = "requires features no_core, lang_items, auto_traits")]
fn custom_target_dependency() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]

                name = "foo"
                version = "0.0.1"
                authors = ["author@example.com"]

                [dependencies]
                bar = { path = "bar" }
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
                #![allow(internal_features)]
                #![feature(no_core)]
                #![feature(lang_items)]
                #![feature(auto_traits)]
                #![no_core]

                extern crate bar;

                pub fn foo() -> u32 {
                    bar::bar()
                }

                #[lang = "freeze"]
                unsafe auto trait Freeze {}
            "#,
        )
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file(
            "bar/src/lib.rs",
            &"
                __MINIMAL_LIB__

                pub fn bar() -> u32 {
                    42
                }
            "
            .replace("__MINIMAL_LIB__", MINIMAL_LIB),
        )
        .file("custom-target.json", SIMPLE_SPEC)
        .build();

    p.cargo("build --lib --target custom-target.json -v").run();
}

#[cargo_test(nightly, reason = "requires features no_core, lang_items")]
// This is randomly crashing in lld. See https://github.com/rust-lang/rust/issues/115985
#[cfg_attr(all(windows, target_env = "gnu"), ignore = "windows-gnu lld crashing")]
fn custom_bin_target() {
    let p = project()
        .file(
            "src/main.rs",
            &"
                #![no_main]
                __MINIMAL_LIB__
            "
            .replace("__MINIMAL_LIB__", MINIMAL_LIB),
        )
        .file("custom-bin-target.json", SIMPLE_SPEC)
        .build();

    p.cargo("build --target custom-bin-target.json -v").run();
}

#[cargo_test(nightly, reason = "requires features no_core, lang_items")]
fn changing_spec_rebuilds() {
    // Changing the .json file will trigger a rebuild.
    let p = project()
        .file(
            "src/lib.rs",
            &"
                __MINIMAL_LIB__

                pub fn foo() -> u32 {
                    42
                }
            "
            .replace("__MINIMAL_LIB__", MINIMAL_LIB),
        )
        .file("custom-target.json", SIMPLE_SPEC)
        .build();

    p.cargo("build --lib --target custom-target.json -v").run();
    p.cargo("build --lib --target custom-target.json -v")
        .with_stderr(
            "\
[FRESH] foo [..]
[FINISHED] [..]
",
        )
        .run();
    let spec_path = p.root().join("custom-target.json");
    let spec = fs::read_to_string(&spec_path).unwrap();
    // Some arbitrary change that I hope is safe.
    let spec = spec.replace('{', "{\n\"vendor\": \"unknown\",\n");
    fs::write(&spec_path, spec).unwrap();
    p.cargo("build --lib --target custom-target.json -v")
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 [..]
[RUNNING] `rustc [..]
[FINISHED] [..]
",
        )
        .run();
}

#[cargo_test(nightly, reason = "requires features no_core, lang_items")]
fn changing_spec_relearns_crate_types() {
    // Changing the .json file will invalidate the cache of crate types.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [lib]
                crate-type = ["cdylib"]
            "#,
        )
        .file("src/lib.rs", MINIMAL_LIB)
        .file("custom-target.json", SIMPLE_SPEC)
        .build();

    p.cargo("build --lib --target custom-target.json -v")
        .with_status(101)
        .with_stderr("error: cannot produce cdylib for `foo [..]")
        .run();

    // Enable dynamic linking.
    let spec_path = p.root().join("custom-target.json");
    let spec = fs::read_to_string(&spec_path).unwrap();
    let spec = spec.replace('{', "{\n\"dynamic-linking\": true,\n");
    fs::write(&spec_path, spec).unwrap();

    p.cargo("build --lib --target custom-target.json -v")
        .with_stderr(
            "\
[COMPILING] foo [..]
[RUNNING] `rustc [..]
[FINISHED] [..]
",
        )
        .run();
}

#[cargo_test(nightly, reason = "requires features no_core, lang_items")]
fn custom_target_ignores_filepath() {
    // Changing the path of the .json file will not trigger a rebuild.
    let p = project()
        .file(
            "src/lib.rs",
            &"
                __MINIMAL_LIB__

                pub fn foo() -> u32 {
                    42
                }
            "
            .replace("__MINIMAL_LIB__", MINIMAL_LIB),
        )
        .file("b/custom-target.json", SIMPLE_SPEC)
        .file("a/custom-target.json", SIMPLE_SPEC)
        .build();

    // Should build the library the first time.
    p.cargo("build --lib --target a/custom-target.json")
        .with_stderr(
            "\
[..]Compiling foo v0.0.1 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();

    // But not the second time, even though the path to the custom target is dfferent.
    p.cargo("build --lib --target b/custom-target.json")
        .with_stderr("[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]")
        .run();
}
