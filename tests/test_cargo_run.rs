use std::path;

use support::{project, execs, path2url};
use support::{COMPILING, RUNNING};
use hamcrest::{assert_that, existing_file};

fn setup() {
}

test!(simple {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/main.rs", r#"
            fn main() { println!("hello"); }
        "#);

    assert_that(p.cargo_process("cargo-run"),
                execs().with_status(0).with_stdout(format!("\
{compiling} foo v0.0.1 ({dir})
{running} `target{sep}foo`
hello
",
        compiling = COMPILING,
        running = RUNNING,
        dir = path2url(p.root()),
        sep = path::SEP).as_slice()));
    assert_that(&p.bin("foo"), existing_file());
})

test!(simple_with_args {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/main.rs", r#"
            fn main() {
                assert_eq!(std::os::args().get(1).as_slice(), "hello");
                assert_eq!(std::os::args().get(2).as_slice(), "world");
            }
        "#);

    assert_that(p.cargo_process("cargo-run").arg("hello").arg("world"),
                execs().with_status(0));
})

test!(exit_code {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/main.rs", r#"
            fn main() { std::os::set_exit_status(2); }
        "#);

    assert_that(p.cargo_process("cargo-run"),
                execs().with_status(2));
})

test!(no_main_file {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/lib.rs", "");

    assert_that(p.cargo_process("cargo-run"),
                execs().with_status(101)
                       .with_stderr("`src/main.rs` must be present for \
                                     `cargo run`\n"));
})

test!(run_dylib_dep {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.bar]
            path = "bar"
        "#)
        .file("src/main.rs", r#"
            extern crate bar;
            fn main() { bar::bar(); }
        "#)
        .file("bar/Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.0.1"
            authors = []

            [lib]
            name = "bar"
            crate-type = ["dylib"]
        "#)
        .file("bar/src/lib.rs", "pub fn bar() {}");

    assert_that(p.cargo_process("cargo-run").arg("hello").arg("world"),
                execs().with_status(0));
})

test!(release_works {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/main.rs", r#"
            fn main() { if !cfg!(ndebug) { fail!() } }
        "#);

    assert_that(p.cargo_process("cargo-run").arg("--release"),
                execs().with_status(0).with_stdout(format!("\
{compiling} foo v0.0.1 ({dir})
{running} `target{sep}release{sep}foo`
",
        compiling = COMPILING,
        running = RUNNING,
        dir = path2url(p.root()),
        sep = path::SEP).as_slice()));
    assert_that(&p.release_bin("foo"), existing_file());
})
