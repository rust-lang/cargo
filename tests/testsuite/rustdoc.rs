use support::{basic_manifest, execs, project};
use support::hamcrest::assert_that;

#[test]
fn rustdoc_simple() {
    let p = project()
        .file("src/lib.rs", "")
        .build();

    assert_that(
        p.cargo("rustdoc -v"),
        execs().with_stderr(format!(
            "\
[DOCUMENTING] foo v0.0.1 ({url})
[RUNNING] `rustdoc --crate-name foo src/lib.rs \
        -o {dir}/target/doc \
        -L dependency={dir}/target/debug/deps`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
            dir = p.root().display(),
            url = p.url()
        )),
    );
}

#[test]
fn rustdoc_args() {
    let p = project()
        .file("src/lib.rs", "")
        .build();

    assert_that(
        p.cargo("rustdoc -v -- --cfg=foo"),
        execs().with_stderr(format!(
            "\
[DOCUMENTING] foo v0.0.1 ({url})
[RUNNING] `rustdoc --crate-name foo src/lib.rs \
        -o {dir}/target/doc \
        --cfg=foo \
        -L dependency={dir}/target/debug/deps`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
            dir = p.root().display(),
            url = p.url()
        )),
    );
}

#[test]
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
    let _bar = project().at("bar")
        .file("Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file("src/lib.rs", "pub fn baz() {}")
        .build();

    assert_that(
        foo.cargo("rustdoc -v -- --cfg=foo"),
        execs().with_stderr(format!(
            "\
[CHECKING] bar v0.0.1 ([..])
[RUNNING] `rustc [..]bar/src/lib.rs [..]`
[DOCUMENTING] foo v0.0.1 ({url})
[RUNNING] `rustdoc --crate-name foo src/lib.rs \
        -o {dir}/target/doc \
        --cfg=foo \
        -L dependency={dir}/target/debug/deps \
        --extern [..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
            dir = foo.root().display(),
            url = foo.url()
        )),
    );
}

#[test]
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
    let _bar = project().at("bar")
        .file("Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file("src/lib.rs", "pub fn baz() {}")
        .build();

    assert_that(
        foo.cargo("rustdoc -v -p bar -- --cfg=foo"),
        execs().with_stderr(format!(
            "\
[DOCUMENTING] bar v0.0.1 ([..])
[RUNNING] `rustdoc --crate-name bar [..]bar/src/lib.rs \
        -o {dir}/target/doc \
        --cfg=foo \
        -L dependency={dir}/target/debug/deps`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
            dir = foo.root().display()
        )),
    );
}

#[test]
fn rustdoc_same_name_documents_lib() {
    let p = project()
        .file("src/main.rs", "fn main() {}")
        .file("src/lib.rs", r#" "#)
        .build();

    assert_that(
        p.cargo("rustdoc -v -- --cfg=foo"),
        execs().with_stderr(format!(
            "\
[DOCUMENTING] foo v0.0.1 ([..])
[RUNNING] `rustdoc --crate-name foo src/lib.rs \
        -o {dir}/target/doc \
        --cfg=foo \
        -L dependency={dir}/target/debug/deps`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
            dir = p.root().display()
        )),
    );
}

#[test]
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

    assert_that(
        p.cargo("rustdoc --verbose --features quux"),
        execs()
            .with_stderr_contains("[..]feature=[..]quux[..]"),
    );
}

#[test]
#[cfg(all(target_arch = "x86_64", target_os = "linux", target_env = "gnu"))]
fn rustdoc_target() {
    let p = project()
        .file("src/lib.rs", "")
        .build();

    assert_that(
        p.cargo("rustdoc --verbose --target x86_64-unknown-linux-gnu"),
        execs().with_stderr("\
[DOCUMENTING] foo v0.0.1 ([..])
[RUNNING] `rustdoc --crate-name foo src/lib.rs \
    --target x86_64-unknown-linux-gnu \
    -o [..]foo/target/x86_64-unknown-linux-gnu/doc \
    -L dependency=[..]foo/target/x86_64-unknown-linux-gnu/debug/deps \
    -L dependency=[..]foo/target/debug/deps`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]"),
    );
}
