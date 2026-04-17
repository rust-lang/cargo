//! Tests for the `cargo rustdoc` command.

use std::fs;

use crate::prelude::*;
use cargo_test_support::str;
use cargo_test_support::{basic_manifest, cross_compile, project};

use crate::utils::cross_compile::disabled as cross_compile_disabled;

#[cargo_test]
fn rustdoc_simple() {
    let p = project().file("src/lib.rs", "").build();

    p.cargo("rustdoc -v")
        .with_stderr_data(str![[r#"
[DOCUMENTING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustdoc [..] --crate-name foo src/lib.rs -o [ROOT]/foo/target/doc [..] -L dependency=[ROOT]/foo/target/debug/deps [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[GENERATED] [ROOT]/foo/target/doc/foo/index.html

"#]])
        .run();
}

#[cargo_test]
fn rustdoc_simple_html() {
    let p = project().file("src/lib.rs", "").build();

    p.cargo("rustdoc --output-format html --open -v")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] the `--output-format` flag is unstable, and only available on the nightly channel of Cargo, but this is the `stable` channel
See https://doc.rust-lang.org/book/[..].html for more information about Rust release channels.
See https://github.com/rust-lang/cargo/issues/12103 for more information about the `--output-format` flag.

"#]])
        .run();
}

#[cargo_test(nightly, reason = "--output-format is unstable")]
fn rustdoc_simple_json() {
    let p = project().file("src/lib.rs", "").build();

    p.cargo("rustdoc -Z unstable-options --output-format json -v")
        .masquerade_as_nightly_cargo(&["rustdoc-output-format"])
        .with_stderr_data(str![[r#"
[DOCUMENTING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustdoc [..] --crate-name foo [..]-o [ROOT]/foo/target/debug/build/foo-[HASH]/out [..] --output-format=json[..]
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[GENERATED] [ROOT]/foo/target/doc/foo.json

"#]])
        .run();
    assert!(p.root().join("target/doc/foo.json").is_file());
}

#[cargo_test(nightly, reason = "--output-format is unstable")]
fn rustdoc_json_with_new_layout() {
    let p = project().file("src/lib.rs", "").build();

    p.cargo("rustdoc -Z unstable-options -Z build-dir-new-layout  --output-format json -v")
        .masquerade_as_nightly_cargo(&["rustdoc-output-format"])
        .with_stderr_data(str![[r#"
[DOCUMENTING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustdoc [..] --crate-name foo [..]-o [ROOT]/foo/target/debug/build/foo/[HASH]/out [..] --output-format=json[..]
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[GENERATED] [ROOT]/foo/target/doc/foo.json

"#]])
        .run();
    assert!(p.root().join("target/doc/foo.json").is_file());
}

#[cargo_test]
fn rustdoc_invalid_output_format() {
    let p = project().file("src/lib.rs", "").build();

    p.cargo("rustdoc -Z unstable-options --output-format pdf -v")
        .masquerade_as_nightly_cargo(&["rustdoc-output-format"])
        .with_status(1)
        .with_stderr_data(str![[r#"
[ERROR] invalid value 'pdf' for '--output-format <FMT>'
  [possible values: html, json]

For more information, try '--help'.

"#]])
        .run();
}

#[cargo_test]
fn rustdoc_json_stable() {
    let p = project().file("src/lib.rs", "").build();

    p.cargo("rustdoc -Z unstable-options --output-format json -v")
        .with_status(101)
        .with_stderr_data(
            str![[r#"
[ERROR] the `-Z` flag is only accepted on the nightly channel of Cargo, but this is the `stable` channel
See https://doc.rust-lang.org/book/[..].html for more information about Rust release channels.

"#]]
	    )
        .run();
}

#[cargo_test]
fn rustdoc_json_without_unstable_options() {
    let p = project().file("src/lib.rs", "").build();

    p.cargo("rustdoc --output-format json -v")
        .masquerade_as_nightly_cargo(&["rustdoc-output-format"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] the `--output-format` flag is unstable, pass `-Z unstable-options` to enable it
See https://github.com/rust-lang/cargo/issues/12103 for more information about the `--output-format` flag.

"#]])
        .run();
}

#[cargo_test]
fn rustdoc_args() {
    let p = project().file("src/lib.rs", "").build();

    p.cargo("rustdoc -v -- --cfg=foo")
        .with_stderr_data(str![[r#"
[DOCUMENTING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustdoc [..] --crate-name foo src/lib.rs -o [ROOT]/foo/target/doc [..]-C metadata=[..] -L dependency=[ROOT]/foo/target/debug/deps [..]--cfg=foo[..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[GENERATED] [ROOT]/foo/target/doc/foo/index.html

"#]])
        .run();
}

#[cargo_test]
fn rustdoc_binary_args_passed() {
    let p = project().file("src/main.rs", "").build();

    p.cargo("rustdoc -v")
        .arg("--")
        .arg("--markdown-no-toc")
        .with_stderr_data(str![[r#"
...
[RUNNING] `rustdoc [..] --markdown-no-toc[..]`
...
"#]])
        .run();
}

#[cargo_test]
fn rustdoc_foo_with_bar_dependency() {
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies.bar]
                path = "../bar"
            "#,
        )
        .file("src/lib.rs", "extern crate bar; pub fn foo() {}")
        .build();
    let _bar = project()
        .at("bar")
        .file("Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file("src/lib.rs", "pub fn baz() {}")
        .build();

    foo.cargo("rustdoc -v -- --cfg=foo")
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to latest compatible version
[CHECKING] bar v0.0.1 ([ROOT]/bar)
[RUNNING] `rustc [..] [ROOT]/bar/src/lib.rs [..]`
[DOCUMENTING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustdoc [..] --crate-name foo src/lib.rs -o [ROOT]/foo/target/doc [..]-C metadata=[..] -L dependency=[ROOT]/foo/target/debug/deps --extern [..]--cfg=foo[..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[GENERATED] [ROOT]/foo/target/doc/foo/index.html

"#]])
        .run();
}

#[cargo_test]
fn rustdoc_only_bar_dependency() {
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies.bar]
                path = "../bar"
            "#,
        )
        .file("src/main.rs", "extern crate bar; fn main() { bar::baz() }")
        .build();
    let _bar = project()
        .at("bar")
        .file("Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file("src/lib.rs", "pub fn baz() {}")
        .build();

    foo.cargo("rustdoc -v -p bar -- --cfg=foo")
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to latest compatible version
[DOCUMENTING] bar v0.0.1 ([ROOT]/bar)
[RUNNING] `rustdoc [..] --crate-name bar [ROOT]/bar/src/lib.rs -o [ROOT]/foo/target/doc [..]-C metadata=[..] -L dependency=[ROOT]/foo/target/debug/deps [..]--cfg=foo[..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[GENERATED] [ROOT]/foo/target/doc/bar/index.html

"#]])
        .run();
}

#[cargo_test]
fn rustdoc_same_name_documents_lib() {
    let p = project()
        .file("src/main.rs", "fn main() {}")
        .file("src/lib.rs", r#" "#)
        .build();

    p.cargo("rustdoc -v -- --cfg=foo")
        .with_stderr_data(str![[r#"
[DOCUMENTING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustdoc [..] --crate-name foo src/lib.rs -o [ROOT]/foo/target/doc [..]-C metadata=[..] -L dependency=[ROOT]/foo/target/debug/deps [..]--cfg=foo[..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[GENERATED] [ROOT]/foo/target/doc/foo/index.html

"#]])
        .run();
}

#[cargo_test]
fn features() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [features]
                quux = []
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("rustdoc --verbose --features quux")
        .with_stderr_data(str![[r#"
...
[RUNNING] `rustdoc [..]feature=[..]quux[..]`
...
"#]])
        .run();
}

#[cargo_test]
fn proc_macro_crate_type() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [lib]
                proc-macro = true

            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("rustdoc --verbose")
        .with_stderr_data(str![[r#"
...
[RUNNING] `rustdoc --edition=2015 --crate-type proc-macro [..]`
...
"#]])
        .run();
}

#[cargo_test]
fn rustdoc_target() {
    if cross_compile_disabled() {
        return;
    }

    let p = project().file("src/lib.rs", "").build();

    p.cargo("rustdoc --verbose --target")
        .arg(cross_compile::alternate())
        .with_stderr_data(str![[r#"
[DOCUMENTING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustdoc [..]--crate-name foo src/lib.rs [..]--target [ALT_TARGET] -o [ROOT]/foo/target/[ALT_TARGET]/doc [..] -L dependency=[ROOT]/foo/target/[ALT_TARGET]/debug/deps -L dependency=[ROOT]/foo/target/debug/deps[..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[GENERATED] [ROOT]/foo/target/[..]/doc/foo/index.html

"#]])
        .run();
}

#[cargo_test]
fn fail_with_glob() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["bar"]
            "#,
        )
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "pub fn bar() {  break_the_build(); }")
        .build();

    p.cargo("rustdoc -p '*z'")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] glob patterns on package selection are not supported.

"#]])
        .run();
}

#[cargo_test(nightly, reason = "--output-format is unstable")]
fn rustdoc_json_same_crate_different_version() {
    let entry = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "entry"
                version = "0.1.0"
                edition = "2021"

                [dependencies]
                dep_v1 = { path = "../dep_v1", package = "dep" }
                dep_v2 = { path = "../dep_v2", package = "dep" }
            "#,
        )
        .file("src/lib.rs", "pub fn entry() {}")
        .build();

    let _dep_v1 = project()
        .at("dep_v1")
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "dep"
                version = "1.0.0"
                edition = "2021"
            "#,
        )
        .file("src/lib.rs", "pub fn dep_v1_fn() {}")
        .build();

    let _dep_v2 = project()
        .at("dep_v2")
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "dep"
                version = "2.0.0"
                edition = "2021"
            "#,
        )
        .file("src/lib.rs", "pub fn dep_v2_fn() {}")
        .build();

    entry
        .cargo("rustdoc -v -Z unstable-options --output-format json -p dep@1.0.0")
        .masquerade_as_nightly_cargo(&["rustdoc-output-format"])
        .with_stderr_data(str![[r#"
[LOCKING] 2 packages to latest compatible versions
[DOCUMENTING] dep v1.0.0 ([ROOT]/dep_v1)
[RUNNING] `rustdoc [..] --crate-name dep [ROOT]/dep_v1/src/lib.rs [..] --output-format=json[..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[GENERATED] [ROOT]/foo/target/doc/dep.json

"#]])
        .run();

    let dep_json = fs::read_to_string(entry.root().join("target/doc/dep.json")).unwrap();
    assert!(dep_json.contains("dep_v1_fn"));
    assert!(!dep_json.contains("dep_v2_fn"));

    entry
        .cargo("rustdoc -v -Z unstable-options --output-format json -p dep@2.0.0")
        .masquerade_as_nightly_cargo(&["rustdoc-output-format"])
        .with_stderr_data(str![[r#"
[DOCUMENTING] dep v2.0.0 ([ROOT]/dep_v2)
[RUNNING] `rustdoc [..] --crate-name dep [ROOT]/dep_v2/src/lib.rs [..] --output-format=json[..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[GENERATED] [ROOT]/foo/target/doc/dep.json

"#]])
        .run();

    let dep_json = fs::read_to_string(entry.root().join("target/doc/dep.json")).unwrap();
    assert!(!dep_json.contains("dep_v1_fn"));
    assert!(dep_json.contains("dep_v2_fn"));

    entry
        .cargo("rustdoc -v -Z unstable-options --output-format json -p dep@1.0.0")
        .masquerade_as_nightly_cargo(&["rustdoc-output-format"])
        .with_stderr_data(str![[r#"
[FRESH] dep v1.0.0 ([ROOT]/dep_v1)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[GENERATED] [ROOT]/foo/target/doc/dep.json

"#]])
        .run();

    let dep_json = fs::read_to_string(entry.root().join("target/doc/dep.json")).unwrap();
    assert!(dep_json.contains("dep_v1_fn"));
    assert!(!dep_json.contains("dep_v2_fn"));
}
