extern crate cargotest;
extern crate hamcrest;

use cargotest::support::{execs, project};
use hamcrest::{assert_that};

#[test]
fn rustdoc_simple() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/lib.rs", r#" "#);

    assert_that(p.cargo_process("rustdoc").arg("-v"),
                execs()
                .with_status(0)
                .with_stderr(format!("\
[DOCUMENTING] foo v0.0.1 ({url})
[RUNNING] `rustdoc --crate-name foo src[/]lib.rs \
        -o {dir}[/]target[/]doc \
        -L dependency={dir}[/]target[/]debug[/]deps`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
", dir = p.root().display(), url = p.url())));
}

#[test]
fn rustdoc_args() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/lib.rs", r#" "#);

    assert_that(p.cargo_process("rustdoc").arg("-v").arg("--").arg("--cfg=foo"),
                execs()
                .with_status(0)
                .with_stderr(format!("\
[DOCUMENTING] foo v0.0.1 ({url})
[RUNNING] `rustdoc --crate-name foo src[/]lib.rs \
        -o {dir}[/]target[/]doc \
        --cfg=foo \
        -L dependency={dir}[/]target[/]debug[/]deps`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
", dir = p.root().display(), url = p.url())));
}



#[test]
fn rustdoc_foo_with_bar_dependency() {
    let foo = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.bar]
            path = "../bar"
        "#)
        .file("src/lib.rs", r#"
            extern crate bar;
            pub fn foo() {}
        "#);
    let bar = project("bar")
        .file("Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/lib.rs", r#"
            pub fn baz() {}
        "#);
    bar.build();

    assert_that(foo.cargo_process("rustdoc").arg("-v").arg("--").arg("--cfg=foo"),
                execs()
                .with_status(0)
                .with_stderr(format!("\
[COMPILING] bar v0.0.1 ([..])
[RUNNING] `rustc [..]bar[/]src[/]lib.rs [..]`
[DOCUMENTING] foo v0.0.1 ({url})
[RUNNING] `rustdoc --crate-name foo src[/]lib.rs \
        -o {dir}[/]target[/]doc \
        --cfg=foo \
        -L dependency={dir}[/]target[/]debug[/]deps \
        --extern [..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
", dir = foo.root().display(), url = foo.url())));
}

#[test]
fn rustdoc_only_bar_dependency() {
    let foo = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.bar]
            path = "../bar"
        "#)
        .file("src/main.rs", r#"
            extern crate bar;
            fn main() {
                bar::baz()
            }
        "#);
    let bar = project("bar")
        .file("Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/lib.rs", r#"
            pub fn baz() {}
        "#);
    bar.build();

    assert_that(foo.cargo_process("rustdoc").arg("-v").arg("-p").arg("bar")
                                            .arg("--").arg("--cfg=foo"),
                execs()
                .with_status(0)
                .with_stderr(format!("\
[DOCUMENTING] bar v0.0.1 ([..])
[RUNNING] `rustdoc --crate-name bar [..]bar[/]src[/]lib.rs \
        -o {dir}[/]target[/]doc \
        --cfg=foo \
        -L dependency={dir}[/]target[/]debug[/]deps`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
", dir = foo.root().display())));
}


#[test]
fn rustdoc_same_name_err() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/main.rs", r#"
            fn main() {}
        "#)
        .file("src/lib.rs", r#" "#);

    assert_that(p.cargo_process("rustdoc").arg("-v")
                 .arg("--").arg("--cfg=foo"),
                execs()
                .with_status(101)
                .with_stderr("[ERROR] The target `foo` is specified as a \
library and as a binary by package `foo [..]`. It can be documented[..]"));
}
