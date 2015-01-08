use std::path;

use support::{project, cargo_dir, execs, path2url};
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

    assert_that(p.cargo_process("run"),
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
});

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
                assert_eq!(std::os::args()[1].as_slice(), "hello");
                assert_eq!(std::os::args()[2].as_slice(), "world");
            }
        "#);

    assert_that(p.cargo_process("run").arg("hello").arg("world"),
                execs().with_status(0));
});

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

    assert_that(p.cargo_process("run"),
                execs().with_status(2));
});

test!(no_main_file {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/lib.rs", "");

    assert_that(p.cargo_process("run"),
                execs().with_status(101)
                       .with_stderr("a bin target must be available \
                                     for `cargo run`\n"));
});

test!(too_many_bins {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/lib.rs", "")
        .file("src/bin/a.rs", "")
        .file("src/bin/b.rs", "");

    assert_that(p.cargo_process("run"),
                execs().with_status(101)
                       .with_stderr("`cargo run` requires that a project only \
                                     have one executable. Use the `--bin` option \
                                     to specify which one to run\n"));
});

test!(specify_name {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/lib.rs", "")
        .file("src/bin/a.rs", r#"
            extern crate foo;
            fn main() { println!("hello a.rs"); }
        "#)
        .file("src/bin/b.rs", r#"
            extern crate foo;
            fn main() { println!("hello b.rs"); }
        "#);

    assert_that(p.cargo_process("run").arg("--bin").arg("a"),
                execs().with_status(0).with_stdout(format!("\
{compiling} foo v0.0.1 ({dir})
{running} `target{sep}a`
hello a.rs
",
        compiling = COMPILING,
        running = RUNNING,
        dir = path2url(p.root()),
        sep = path::SEP).as_slice()));

    assert_that(p.process(cargo_dir().join("cargo")).arg("run").arg("--bin").arg("b"),
                execs().with_status(0).with_stdout(format!("\
{running} `target{sep}b`
hello b.rs
",
        running = RUNNING,
        sep = path::SEP).as_slice()));
});

test!(run_example {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/lib.rs", "")
        .file("examples/a.rs", r#"
            fn main() { println!("example"); }
        "#)
        .file("src/bin/a.rs", r#"
            fn main() { println!("bin"); }
        "#);

    assert_that(p.cargo_process("run").arg("--example").arg("a"),
                execs().with_status(0).with_stdout(format!("\
{compiling} foo v0.0.1 ({dir})
{running} `target{sep}examples{sep}a`
example
",
        compiling = COMPILING,
        running = RUNNING,
        dir = path2url(p.root()),
        sep = path::SEP).as_slice()));
});

test!(either_name_or_example {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/bin/a.rs", r#"
            fn main() { println!("hello a.rs"); }
        "#)
        .file("examples/b.rs", r#"
            fn main() { println!("hello b.rs"); }
        "#);

    assert_that(p.cargo_process("run").arg("--bin").arg("a").arg("--example").arg("b"),
                execs().with_status(1)
                       .with_stderr("specify either `--bin` or `--example`, \
                                     not both"));
});

test!(one_bin_multiple_examples {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/lib.rs", "")
        .file("src/bin/main.rs", r#"
            fn main() { println!("hello main.rs"); }
        "#)
        .file("examples/a.rs", r#"
            fn main() { println!("hello a.rs"); }
        "#)
        .file("examples/b.rs", r#"
            fn main() { println!("hello b.rs"); }
        "#);

    assert_that(p.cargo_process("run"),
                execs().with_status(0).with_stdout(format!("\
{compiling} foo v0.0.1 ({dir})
{running} `target{sep}main`
hello main.rs
",
        compiling = COMPILING,
        running = RUNNING,
        dir = path2url(p.root()),
        sep = path::SEP).as_slice()));
});

test!(example_with_release_flag {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.bar]
            version = "*"
            path = "bar"
        "#)
        .file("examples/a.rs", r#"
            extern crate bar;

            fn main() {
                if cfg!(ndebug) {
                    println!("fast1")
                } else {
                    println!("slow1")
                }
                bar::baz();
            }
        "#)
        .file("bar/Cargo.toml", r#"
            [project]
            name = "bar"
            version = "0.0.1"
            authors = []

            [lib]
            name = "bar"
        "#)
        .file("bar/src/bar.rs", r#"
            pub fn baz() {
                if cfg!(ndebug) {
                    println!("fast2")
                } else {
                    println!("slow2")
                }
            }
        "#);

    assert_that(p.cargo_process("run").arg("-v").arg("--release").arg("--example").arg("a"),
                execs().with_status(0).with_stdout(format!("\
{compiling} bar v0.0.1 ({url})
{running} `rustc src{sep}bar.rs --crate-name bar --crate-type lib \
        -C opt-level=3 \
        --cfg ndebug \
        -C metadata=[..] \
        -C extra-filename=[..] \
        --out-dir {dir}{sep}target{sep}release{sep}deps \
        --emit=dep-info,link \
        -L dependency={dir}{sep}target{sep}release{sep}deps \
        -L dependency={dir}{sep}target{sep}release{sep}deps`
{compiling} foo v0.0.1 ({url})
{running} `rustc {dir}{sep}examples{sep}a.rs --crate-name a --crate-type bin \
        -C opt-level=3 \
        --cfg ndebug \
        --out-dir {dir}{sep}target{sep}release{sep}examples \
        --emit=dep-info,link \
        -L dependency={dir}{sep}target{sep}release \
        -L dependency={dir}{sep}target{sep}release{sep}deps \
         --extern bar={dir}{sep}target{sep}release{sep}deps{sep}libbar-[..].rlib`
{running} `target{sep}release{sep}examples{sep}a`
fast1
fast2
",
        compiling = COMPILING,
        running = RUNNING,
        dir = p.root().display(),
        url = path2url(p.root()),
        sep = path::SEP).as_slice()));

    assert_that(p.process(cargo_dir().join("cargo")).arg("run").arg("-v").arg("--example").arg("a"),
                execs().with_status(0).with_stdout(format!("\
{compiling} bar v0.0.1 ({url})
{running} `rustc src{sep}bar.rs --crate-name bar --crate-type lib \
        -g \
        -C metadata=[..] \
        -C extra-filename=[..] \
        --out-dir {dir}{sep}target{sep}deps \
        --emit=dep-info,link \
        -L dependency={dir}{sep}target{sep}deps \
        -L dependency={dir}{sep}target{sep}deps`
{compiling} foo v0.0.1 ({url})
{running} `rustc {dir}{sep}examples{sep}a.rs --crate-name a --crate-type bin \
        -g \
        --out-dir {dir}{sep}target{sep}examples \
        --emit=dep-info,link \
        -L dependency={dir}{sep}target \
        -L dependency={dir}{sep}target{sep}deps \
         --extern bar={dir}{sep}target{sep}deps{sep}libbar-[..].rlib`
{running} `target{sep}examples{sep}a`
slow1
slow2
",
        compiling = COMPILING,
        running = RUNNING,
        dir = p.root().display(),
        url = path2url(p.root()),
        sep = path::SEP).as_slice()));
});

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

    assert_that(p.cargo_process("run").arg("hello").arg("world"),
                execs().with_status(0));
});

test!(release_works {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/main.rs", r#"
            fn main() { if !cfg!(ndebug) { panic!() } }
        "#);

    assert_that(p.cargo_process("run").arg("--release"),
                execs().with_status(0).with_stdout(format!("\
{compiling} foo v0.0.1 ({dir})
{running} `target{sep}release{sep}foo`
",
        compiling = COMPILING,
        running = RUNNING,
        dir = path2url(p.root()),
        sep = path::SEP).as_slice()));
    assert_that(&p.release_bin("foo"), existing_file());
});

test!(run_bin_different_name {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [[bin]]
            name = "bar"
        "#)
        .file("src/bar.rs", r#"
            fn main() { }
        "#);

    assert_that(p.cargo_process("run"), execs().with_status(0));
});
