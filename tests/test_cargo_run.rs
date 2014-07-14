use std::path;

use support::{project, execs};
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
{compiling} foo v0.0.1 (file:{dir})
{running} `target{sep}main`
hello
",
        compiling = COMPILING,
        running = RUNNING,
        dir = p.root().display(),
        sep = path::SEP).as_slice()));
    assert_that(&p.bin("main"), existing_file());
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
