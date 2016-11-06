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
[RUNNING] `rustdoc src[/]lib.rs --crate-name foo \
        -o {dir}[/]target[/]doc \
        -L dependency={dir}[/]target[/]debug[/]deps`
[FINISHED] debug [unoptimized + debuginfo] target(s) in [..]
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

    assert_that(p.cargo_process("rustdoc").arg("-v").arg("--").arg("--no-defaults"),
                execs()
                .with_status(0)
                .with_stderr(format!("\
[DOCUMENTING] foo v0.0.1 ({url})
[RUNNING] `rustdoc src[/]lib.rs --crate-name foo \
        -o {dir}[/]target[/]doc \
        --no-defaults \
        -L dependency={dir}[/]target[/]debug[/]deps`
[FINISHED] debug [unoptimized + debuginfo] target(s) in [..]
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

    assert_that(foo.cargo_process("rustdoc").arg("-v").arg("--").arg("--no-defaults"),
                execs()
                .with_status(0)
                .with_stderr(format!("\
[COMPILING] bar v0.0.1 ([..])
[RUNNING] `rustc [..]bar[/]src[/]lib.rs [..]`
[DOCUMENTING] foo v0.0.1 ({url})
[RUNNING] `rustdoc src[/]lib.rs --crate-name foo \
        -o {dir}[/]target[/]doc \
        --no-defaults \
        -L dependency={dir}[/]target[/]debug[/]deps \
        --extern [..]`
[FINISHED] debug [unoptimized + debuginfo] target(s) in [..]
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
                                            .arg("--").arg("--no-defaults"),
                execs()
                .with_status(0)
                .with_stderr(format!("\
[DOCUMENTING] bar v0.0.1 ([..])
[RUNNING] `rustdoc [..]bar[/]src[/]lib.rs --crate-name bar \
        -o {dir}[/]target[/]doc \
        --no-defaults \
        -L dependency={dir}[/]target[/]debug[/]deps`
[FINISHED] debug [unoptimized + debuginfo] target(s) in [..]
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
                 .arg("--").arg("--no-defaults"),
                execs()
                .with_status(101)
                .with_stderr("[ERROR] cannot document a package where a library and a \
                              binary have the same name. Consider renaming one \
                              or marking the target as `doc = false`"));
}
