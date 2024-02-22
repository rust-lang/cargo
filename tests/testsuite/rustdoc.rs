//! Tests for the `cargo rustdoc` command.

use cargo_test_support::{basic_manifest, cross_compile, project};

#[cargo_test]
fn rustdoc_simple() {
    let p = project().file("src/lib.rs", "").build();

    p.cargo("rustdoc -v")
        .with_stderr(
            "\
[DOCUMENTING] foo v0.0.1 ([CWD])
[RUNNING] `rustdoc [..]--crate-name foo src/lib.rs [..]\
        -o [CWD]/target/doc \
        [..] \
        -L dependency=[CWD]/target/debug/deps [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [..]
[GENERATED] [CWD]/target/doc/foo/index.html
",
        )
        .run();
}

#[cargo_test]
fn rustdoc_simple_html() {
    let p = project().file("src/lib.rs", "").build();

    p.cargo("rustdoc --output-format html --open -v")
        .with_status(101)
        .with_stderr(
            "\
error: the `--output-format` flag is unstable, and only available on the nightly channel of Cargo, but this is the `stable` channel
[..]
See https://github.com/rust-lang/cargo/issues/12103 for more information about the `--output-format` flag.
",
        )
        .run();
}

#[cargo_test(nightly, reason = "--output-format is unstable")]
fn rustdoc_simple_json() {
    let p = project().file("src/lib.rs", "").build();

    p.cargo("rustdoc -Z unstable-options --output-format json -v")
        .masquerade_as_nightly_cargo(&["rustdoc-output-format"])
        .with_stderr(
            "\
[DOCUMENTING] foo v0.0.1 ([CWD])
[RUNNING] `rustdoc [..]--crate-name foo [..]-o [CWD]/target/doc [..]--output-format=json[..]
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [..]
[GENERATED] [CWD]/target/doc/foo.json
",
        )
        .run();
    assert!(p.root().join("target/doc/foo.json").is_file());
}

#[cargo_test]
fn rustdoc_invalid_output_format() {
    let p = project().file("src/lib.rs", "").build();

    p.cargo("rustdoc -Z unstable-options --output-format pdf -v")
        .masquerade_as_nightly_cargo(&["rustdoc-output-format"])
        .with_status(1)
        .with_stderr(
            "\
error: invalid value 'pdf' for '--output-format <FMT>'
  [possible values: html, json]

For more information, try '--help'.
",
        )
        .run();
}

#[cargo_test]
fn rustdoc_json_stable() {
    let p = project().file("src/lib.rs", "").build();

    p.cargo("rustdoc -Z unstable-options --output-format json -v")
        .with_status(101)
        .with_stderr(
            "\
error: the `-Z` flag is only accepted on the nightly channel of Cargo, but this is the `stable` channel
[..]
",
	    )
        .run();
}

#[cargo_test]
fn rustdoc_json_without_unstable_options() {
    let p = project().file("src/lib.rs", "").build();

    p.cargo("rustdoc --output-format json -v")
        .masquerade_as_nightly_cargo(&["rustdoc-output-format"])
        .with_status(101)
        .with_stderr(
            "\
error: the `--output-format` flag is unstable, pass `-Z unstable-options` to enable it
[..]
",
        )
        .run();
}

#[cargo_test]
fn rustdoc_args() {
    let p = project().file("src/lib.rs", "").build();

    p.cargo("rustdoc -v -- --cfg=foo")
        .with_stderr(
            "\
[DOCUMENTING] foo v0.0.1 ([CWD])
[RUNNING] `rustdoc [..]--crate-name foo src/lib.rs [..]\
        -o [CWD]/target/doc \
        [..] \
        --cfg=foo \
        -C metadata=[..] \
        -L dependency=[CWD]/target/debug/deps [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [..]
[GENERATED] [CWD]/target/doc/foo/index.html
",
        )
        .run();
}

#[cargo_test]
fn rustdoc_binary_args_passed() {
    let p = project().file("src/main.rs", "").build();

    p.cargo("rustdoc -v")
        .arg("--")
        .arg("--markdown-no-toc")
        .with_stderr_contains("[RUNNING] `rustdoc [..] --markdown-no-toc[..]`")
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
        .with_stderr(
            "\
[CHECKING] bar v0.0.1 ([..])
[RUNNING] `rustc [..]bar/src/lib.rs [..]`
[DOCUMENTING] foo v0.0.1 ([CWD])
[RUNNING] `rustdoc [..]--crate-name foo src/lib.rs [..]\
        -o [CWD]/target/doc \
        [..] \
        --cfg=foo \
        -C metadata=[..] \
        -L dependency=[CWD]/target/debug/deps \
        --extern [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [..]
[GENERATED] [CWD]/target/doc/foo/index.html
",
        )
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
        .with_stderr(
            "\
[DOCUMENTING] bar v0.0.1 ([..])
[RUNNING] `rustdoc [..]--crate-name bar [..]bar/src/lib.rs [..]\
        -o [CWD]/target/doc \
        [..] \
        --cfg=foo \
        -C metadata=[..] \
        -L dependency=[CWD]/target/debug/deps [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [..]
[GENERATED] [CWD]/target/doc/bar/index.html
",
        )
        .run();
}

#[cargo_test]
fn rustdoc_same_name_documents_lib() {
    let p = project()
        .file("src/main.rs", "fn main() {}")
        .file("src/lib.rs", r#" "#)
        .build();

    p.cargo("rustdoc -v -- --cfg=foo")
        .with_stderr(
            "\
[DOCUMENTING] foo v0.0.1 ([..])
[RUNNING] `rustdoc [..]--crate-name foo src/lib.rs [..]\
        -o [CWD]/target/doc \
        [..] \
        --cfg=foo \
        -C metadata=[..] \
        -L dependency=[CWD]/target/debug/deps [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [..]
[GENERATED] [CWD]/target/doc/foo/index.html
",
        )
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
        .with_stderr_contains("[..]feature=[..]quux[..]")
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
        .with_stderr_contains(
            "\
[RUNNING] `rustdoc --edition=2015 --crate-type proc-macro [..]`
",
        )
        .run();
}

#[cargo_test]
fn rustdoc_target() {
    if cross_compile::disabled() {
        return;
    }

    let p = project().file("src/lib.rs", "").build();

    p.cargo("rustdoc --verbose --target")
        .arg(cross_compile::alternate())
        .with_stderr(format!(
            "\
[DOCUMENTING] foo v0.0.1 ([..])
[RUNNING] `rustdoc [..]--crate-name foo src/lib.rs [..]\
    --target {target} \
    -o [CWD]/target/{target}/doc \
    [..] \
    -L dependency=[CWD]/target/{target}/debug/deps \
    -L dependency=[CWD]/target/debug/deps[..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [..]
[GENERATED] [CWD]/target/[..]/doc/foo/index.html",
            target = cross_compile::alternate()
        ))
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
        .with_stderr("[ERROR] Glob patterns on package selection are not supported.")
        .run();
}
