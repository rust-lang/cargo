use std::io::{fs, File};
use time;

use support::{project, execs};
use support::{COMPILING, cargo_dir, ResultTest, FRESH};
use hamcrest::{assert_that, existing_file};

fn setup() {}

test!(modifying_and_moving {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            authors = []
            version = "0.0.1"
        "#)
        .file("src/main.rs", r#"
            mod a; fn main() {}
        "#)
        .file("src/a.rs", "");

    assert_that(p.cargo_process("cargo-build"),
                execs().with_status(0).with_stdout(format!("\
{compiling} foo v0.0.1 (file:{dir})
", compiling = COMPILING, dir = p.root().display())));

    assert_that(p.process(cargo_dir().join("cargo-build")),
                execs().with_status(0).with_stdout(format!("\
{fresh} foo v0.0.1 (file:{dir})
", fresh = FRESH, dir = p.root().display())));

    File::create(&p.root().join("src/a.rs")).write_str("fn main() {}").assert();
    assert_that(p.process(cargo_dir().join("cargo-build")),
                execs().with_status(0).with_stdout(format!("\
{compiling} foo v0.0.1 (file:{dir})
", compiling = COMPILING, dir = p.root().display())));

    fs::rename(&p.root().join("src/a.rs"), &p.root().join("src/b.rs")).assert();
    assert_that(p.process(cargo_dir().join("cargo-build")),
                execs().with_status(101));
})

test!(modify_only_some_files {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            authors = []
            version = "0.0.1"
        "#)
        .file("src/lib.rs", "mod a;")
        .file("src/a.rs", "")
        .file("src/main.rs", r#"
            mod b;
            fn main() {}
        "#)
        .file("src/b.rs", "")
        .file("tests/test.rs", "");

    assert_that(p.cargo_process("cargo-build"),
                execs().with_status(0).with_stdout(format!("\
{compiling} foo v0.0.1 (file:{dir})
", compiling = COMPILING, dir = p.root().display())));
    assert_that(p.process(cargo_dir().join("cargo-test")),
                execs().with_status(0));

    assert_that(&p.bin("foo"), existing_file());

    let past = time::precise_time_ns() / 1_000_000 - 5000;

    let lib = p.root().join("src/lib.rs");
    let bin = p.root().join("src/b.rs");
    let test = p.root().join("tests/test.rs");

    File::create(&lib).write_str("invalid rust code").assert();
    fs::change_file_times(&lib, past, past).assert();

    File::create(&bin).write_str("fn foo() {}").assert();

    // Make sure the binary is rebuilt, not the lib
    assert_that(p.process(cargo_dir().join("cargo-build")),
                execs().with_status(0).with_stdout(format!("\
{compiling} foo v0.0.1 (file:{dir})
", compiling = COMPILING, dir = p.root().display())));
    assert_that(&p.bin("foo"), existing_file());

    // Make sure the tests don't recompile the lib
    File::create(&test).write_str("fn foo() {}").assert();
    assert_that(p.process(cargo_dir().join("cargo-test")),
                execs().with_status(0));
})
