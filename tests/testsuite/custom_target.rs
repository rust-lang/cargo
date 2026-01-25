//! Tests for custom json target specifications.

use std::fs;

use crate::prelude::*;
use cargo_test_support::basic_manifest;
use cargo_test_support::project;
use cargo_test_support::str;
use cargo_test_support::target_spec_json;

const MINIMAL_LIB: &str = r#"
#![allow(internal_features)]
#![feature(no_core)]
#![feature(lang_items)]
#![no_core]

#[lang = "pointee_sized"]
pub trait PointeeSized {
    // Empty.
}

#[lang = "meta_sized"]
pub trait MetaSized: PointeeSized {
    // Empty.
}

#[lang = "sized"]
pub trait Sized: MetaSized {
    // Empty.
}
#[lang = "copy"]
pub trait Copy {
    // Empty.
}
"#;

#[cargo_test]
fn custom_target_gated() {
    // Checks that .json targets require the -Z option.
    let p = project()
        .file("src/lib.rs", "")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.0.0"))
        .file("bar/src/main.rs", "fn main() {}")
        .build();
    p.cargo("build --target custom-target.json")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] `.json` target specs require -Zjson-target-spec

"#]])
        .run();

    // Same with config settings.
    p.cargo("build")
        .env("CARGO_BUILD_TARGET", "custom-target.json")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] `.json` target specs require -Zjson-target-spec

"#]])
        .run();

    // And artifact dependencies.
    p.change_file(
        "Cargo.toml",
        r#"
            [package]
            name = "foo"
            edition = "2015"

            [dependencies]
            bar = { path = "bar/", artifact = "bin", target = "custom-target.json" }
        "#,
    );
    p.cargo("build -Z bindeps")
        .masquerade_as_nightly_cargo(&["bindeps"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  `.json` target specs require -Zjson-target-spec

"#]])
        .run();
}

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
        .file("custom-target.json", target_spec_json())
        .build();

    p.cargo("build --lib --target custom-target.json -v -Zjson-target-spec")
        .masquerade_as_nightly_cargo(&["json_target_spec"])
        .run();
    p.cargo("build --lib --target src/../custom-target.json -v -Zjson-target-spec")
        .masquerade_as_nightly_cargo(&["json_target_spec"])
        .run();

    // Ensure that the correct style of flag is passed to --target with doc tests.
    p.cargo("test --doc --target src/../custom-target.json -v -Zjson-target-spec")
        .masquerade_as_nightly_cargo(&["no_core", "lang_items", "json-target-spec"])
        .with_stderr_data(str![[r#"
[FRESH] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[DOCTEST] foo
[RUNNING] `rustdoc [..]--target [..]foo/custom-target.json[..]

"#]])
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
                edition = "2015"
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
        .file("custom-target.json", target_spec_json())
        .build();

    p.cargo("build --lib --target custom-target.json -v -Zjson-target-spec")
        .masquerade_as_nightly_cargo(&["json_target_spec"])
        .run();
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
        .file("custom-bin-target.json", target_spec_json())
        .build();

    p.cargo("build --target custom-bin-target.json -v -Zjson-target-spec")
        .masquerade_as_nightly_cargo(&["json_target_spec"])
        .run();
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
        .file("custom-target.json", target_spec_json())
        .build();

    p.cargo("build --lib --target custom-target.json -v -Zjson-target-spec")
        .masquerade_as_nightly_cargo(&["json_target_spec"])
        .run();
    p.cargo("build --lib --target custom-target.json -v -Zjson-target-spec")
        .masquerade_as_nightly_cargo(&["json_target_spec"])
        .with_stderr_data(str![[r#"
[FRESH] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    let spec_path = p.root().join("custom-target.json");
    let spec = fs::read_to_string(&spec_path).unwrap();
    // Some arbitrary change that I hope is safe.
    let spec = spec.replace('{', "{\n\"vendor\": \"unknown\",\n");
    fs::write(&spec_path, spec).unwrap();
    p.cargo("build --lib --target custom-target.json -v -Zjson-target-spec")
        .masquerade_as_nightly_cargo(&["json_target_spec"])
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo --edition=2015 src/lib.rs [..]
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test(nightly, reason = "requires features no_core, lang_items")]
// This is randomly crashing in lld. See https://github.com/rust-lang/rust/issues/115985
#[cfg_attr(all(windows, target_env = "gnu"), ignore = "windows-gnu lld crashing")]
fn changing_spec_relearns_crate_types() {
    // Changing the .json file will invalidate the cache of crate types.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

                [lib]
                crate-type = ["cdylib"]
            "#,
        )
        .file("src/lib.rs", MINIMAL_LIB)
        .file("custom-target.json", target_spec_json())
        .build();

    p.cargo("build --lib --target custom-target.json -v -Zjson-target-spec")
        .masquerade_as_nightly_cargo(&["json_target_spec"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] cannot produce cdylib for `foo v0.1.0 ([ROOT]/foo)` [..]

"#]])
        .run();

    // Enable dynamic linking.
    let spec_path = p.root().join("custom-target.json");
    let spec = fs::read_to_string(&spec_path).unwrap();
    let spec = spec.replace('{', "{\n\"dynamic-linking\": true,\n");
    fs::write(&spec_path, spec).unwrap();

    p.cargo("build --lib --target custom-target.json -v -Zjson-target-spec")
        .masquerade_as_nightly_cargo(&["json_target_spec"])
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.1.0 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo --edition=2015 src/lib.rs [..]
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
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
        .file("b/custom-target.json", target_spec_json())
        .file("a/custom-target.json", target_spec_json())
        .build();

    // Should build the library the first time.
    p.cargo("build --lib --target a/custom-target.json -Zjson-target-spec")
        .masquerade_as_nightly_cargo(&["json_target_spec"])
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    // But not the second time, even though the path to the custom target is different.
    p.cargo("build --lib --target b/custom-target.json -Zjson-target-spec")
        .masquerade_as_nightly_cargo(&["json_target_spec"])
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}
