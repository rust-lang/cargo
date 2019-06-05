use crate::support::is_nightly;
use crate::support::{basic_manifest, project};

#[cargo_test]
fn custom_target_minimal() {
    if !is_nightly() {
        // Requires features no_core, lang_items
        return;
    }
    let p = project()
        .file(
            "src/lib.rs",
            r#"
            #![feature(no_core)]
            #![feature(lang_items)]
            #![no_core]

            pub fn foo() -> u32 {
                42
            }

            #[lang = "sized"]
            pub trait Sized {
                // Empty.
            }
            #[lang = "copy"]
            pub trait Copy {
                // Empty.
            }
        "#,
        )
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

    p.cargo("build --lib --target custom-target.json -v").run();
    p.cargo("build --lib --target src/../custom-target.json -v")
        .run();
}

#[cargo_test]
fn custom_target_dependency() {
    if !is_nightly() {
        // Requires features no_core, lang_items, optin_builtin_traits
        return;
    }
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
            #![feature(no_core)]
            #![feature(lang_items)]
            #![feature(optin_builtin_traits)]
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
            r#"
            #![feature(no_core)]
            #![feature(lang_items)]
            #![no_core]

            pub fn bar() -> u32 {
                42
            }

            #[lang = "sized"]
            pub trait Sized {
                // Empty.
            }
            #[lang = "copy"]
            pub trait Copy {
                // Empty.
            }
        "#,
        )
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

    p.cargo("build --lib --target custom-target.json -v").run();
}
