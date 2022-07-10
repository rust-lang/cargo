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
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
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
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
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
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
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
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
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
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
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
[RUNNING] `rustdoc --crate-type proc-macro [..]`
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
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]",
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
