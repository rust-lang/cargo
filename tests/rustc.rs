extern crate cargotest;
extern crate hamcrest;

use std::path::MAIN_SEPARATOR as SEP;

use cargotest::support::{execs, project};
use hamcrest::assert_that;

const CARGO_RUSTC_ERROR: &'static str =
"[ERROR] extra arguments to `rustc` can only be passed to one target, consider filtering
the package by passing e.g. `--lib` or `--bin NAME` to specify a single target";

#[test]
fn build_lib_for_foo() {
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

    assert_that(p.cargo_process("rustc").arg("--lib").arg("-v"),
                execs()
                .with_status(0)
                .with_stderr(format!("\
[COMPILING] foo v0.0.1 ({url})
[RUNNING] `rustc src{sep}lib.rs --crate-name foo --crate-type lib -g \
        --out-dir {dir}{sep}target{sep}debug \
        --emit=dep-info,link \
        -L dependency={dir}{sep}target{sep}debug \
        -L dependency={dir}{sep}target{sep}debug{sep}deps`
[FINISHED] debug [unoptimized + debuginfo] target(s) in [..]
", sep = SEP,
            dir = p.root().display(), url = p.url())));
}

#[test]
fn lib() {
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

    assert_that(p.cargo_process("rustc").arg("--lib").arg("-v")
                .arg("--").arg("-C").arg("debug-assertions=off"),
                execs()
                .with_status(0)
                .with_stderr(format!("\
[COMPILING] foo v0.0.1 ({url})
[RUNNING] `rustc src{sep}lib.rs --crate-name foo --crate-type lib -g \
        -C debug-assertions=off \
        --out-dir {dir}{sep}target{sep}debug \
        --emit=dep-info,link \
        -L dependency={dir}{sep}target{sep}debug \
        -L dependency={dir}{sep}target{sep}debug{sep}deps`
[FINISHED] debug [unoptimized + debuginfo] target(s) in [..]
", sep = SEP,
            dir = p.root().display(), url = p.url())))
}

#[test]
fn build_main_and_allow_unstable_options() {
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

    assert_that(p.cargo_process("rustc").arg("-v").arg("--bin").arg("foo")
                .arg("--").arg("-C").arg("debug-assertions"),
                execs()
                .with_status(0)
                .with_stderr(&format!("\
[COMPILING] {name} v{version} ({url})
[RUNNING] `rustc src{sep}lib.rs --crate-name {name} --crate-type lib -g \
        --out-dir {dir}{sep}target{sep}debug \
        --emit=dep-info,link \
        -L dependency={dir}{sep}target{sep}debug \
        -L dependency={dir}{sep}target{sep}debug{sep}deps`
[RUNNING] `rustc src{sep}main.rs --crate-name {name} --crate-type bin -g \
        -C debug-assertions \
        --out-dir {dir}{sep}target{sep}debug \
        --emit=dep-info,link \
        -L dependency={dir}{sep}target{sep}debug \
        -L dependency={dir}{sep}target{sep}debug{sep}deps \
        --extern {name}={dir}{sep}target{sep}debug{sep}lib{name}.rlib`
[FINISHED] debug [unoptimized + debuginfo] target(s) in [..]
", sep = SEP,
            dir = p.root().display(), url = p.url(),
            name = "foo", version = "0.0.1")));
}

#[test]
fn fails_when_trying_to_build_main_and_lib_with_args() {
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

    assert_that(p.cargo_process("rustc").arg("-v")
                .arg("--").arg("-C").arg("debug-assertions"),
                execs()
                .with_status(101)
                .with_stderr(CARGO_RUSTC_ERROR));
}

#[test]
fn build_with_args_to_one_of_multiple_binaries() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/bin/foo.rs", r#"
            fn main() {}
        "#)
        .file("src/bin/bar.rs", r#"
            fn main() {}
        "#)
        .file("src/bin/baz.rs", r#"
            fn main() {}
        "#)
        .file("src/lib.rs", r#" "#);

    assert_that(p.cargo_process("rustc").arg("-v").arg("--bin").arg("bar")
                .arg("--").arg("-C").arg("debug-assertions"),
                execs()
                .with_status(0)
                .with_stderr(format!("\
[COMPILING] foo v0.0.1 ({url})
[RUNNING] `rustc src{sep}lib.rs --crate-name foo --crate-type lib -g \
        --out-dir {dir}{sep}target{sep}debug [..]`
[RUNNING] `rustc src{sep}bin{sep}bar.rs --crate-name bar --crate-type bin -g \
        -C debug-assertions [..]`
[FINISHED] debug [unoptimized + debuginfo] target(s) in [..]
", sep = SEP,
                dir = p.root().display(), url = p.url())));
}

#[test]
fn fails_with_args_to_all_binaries() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/bin/foo.rs", r#"
            fn main() {}
        "#)
        .file("src/bin/bar.rs", r#"
            fn main() {}
        "#)
        .file("src/bin/baz.rs", r#"
            fn main() {}
        "#)
        .file("src/lib.rs", r#" "#);

    assert_that(p.cargo_process("rustc").arg("-v")
                .arg("--").arg("-C").arg("debug-assertions"),
                execs()
                .with_status(101)
                .with_stderr(CARGO_RUSTC_ERROR));
}

#[test]
fn build_with_args_to_one_of_multiple_tests() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("tests/foo.rs", r#" "#)
        .file("tests/bar.rs", r#" "#)
        .file("tests/baz.rs", r#" "#)
        .file("src/lib.rs", r#" "#);

    assert_that(p.cargo_process("rustc").arg("-v").arg("--test").arg("bar")
                .arg("--").arg("-C").arg("debug-assertions"),
                execs()
                .with_status(0)
                .with_stderr(format!("\
[COMPILING] foo v0.0.1 ({url})
[RUNNING] `rustc src{sep}lib.rs --crate-name foo --crate-type lib -g \
        --out-dir {dir}{sep}target{sep}debug [..]`
[RUNNING] `rustc tests{sep}bar.rs --crate-name bar -g \
        -C debug-assertions [..]--test[..]`
[FINISHED] debug [unoptimized + debuginfo] target(s) in [..]
", sep = SEP,
                dir = p.root().display(), url = p.url())));
}

#[test]
fn build_foo_with_bar_dependency() {
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
            version = "0.1.0"
            authors = []
        "#)
        .file("src/lib.rs", r#"
            pub fn baz() {}
        "#);
    bar.build();

    assert_that(foo.cargo_process("rustc").arg("-v").arg("--").arg("-C").arg("debug-assertions"),
                execs()
                .with_status(0)
                .with_stderr(format!("\
[COMPILING] bar v0.1.0 ([..])
[RUNNING] `[..] -g -C [..]`
[COMPILING] foo v0.0.1 ({url})
[RUNNING] `[..] -g -C debug-assertions [..]`
[FINISHED] debug [unoptimized + debuginfo] target(s) in [..]
",
                url = foo.url())));
}

#[test]
fn build_only_bar_dependency() {
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
            version = "0.1.0"
            authors = []
        "#)
        .file("src/lib.rs", r#"
            pub fn baz() {}
        "#);
    bar.build();

    assert_that(foo.cargo_process("rustc").arg("-v").arg("-p").arg("bar")
                .arg("--").arg("-C").arg("debug-assertions"),
                execs()
                .with_status(0)
                .with_stderr("\
[COMPILING] bar v0.1.0 ([..])
[RUNNING] `[..]--crate-name bar --crate-type lib [..] -C debug-assertions [..]`
[FINISHED] debug [unoptimized + debuginfo] target(s) in [..]
"));
}

#[test]
fn fail_with_multiple_packages() {
    let foo = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.bar]
                path = "../bar"

            [dependencies.baz]
                path = "../baz"
        "#)
        .file("src/main.rs", r#"
            fn main() {}
        "#);
    foo.build();

    let bar = project("bar")
        .file("Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.1.0"
            authors = []
        "#)
        .file("src/main.rs", r#"
            fn main() {
                if cfg!(flag = "1") { println!("Yeah from bar!"); }
            }
        "#);
    bar.build();

    let baz = project("baz")
        .file("Cargo.toml", r#"
            [package]
            name = "baz"
            version = "0.1.0"
            authors = []
        "#)
        .file("src/main.rs", r#"
            fn main() {
                if cfg!(flag = "1") { println!("Yeah from baz!"); }
            }
        "#);
    baz.build();

    assert_that(foo.cargo("rustc").arg("-v").arg("-p").arg("bar")
                                          .arg("-p").arg("baz"),
                execs().with_status(1).with_stderr("\
[ERROR] Invalid arguments.

Usage:
    cargo rustc [options] [--] [<opts>...]"));
}

#[test]
fn rustc_with_other_profile() {
    let foo = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dev-dependencies]
            a = { path = "a" }
        "#)
        .file("src/main.rs", r#"
            #[cfg(test)] extern crate a;

            #[test]
            fn foo() {}
        "#)
        .file("a/Cargo.toml", r#"
            [package]
            name = "a"
            version = "0.1.0"
            authors = []
        "#)
        .file("a/src/lib.rs", "");
    foo.build();

    assert_that(foo.cargo("rustc").arg("--profile").arg("test"),
                execs().with_status(0));
}
