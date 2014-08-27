use std::io::{fs, File};

use support::{project, execs, path2url};
use support::{COMPILING, cargo_dir, ResultTest, FRESH};
use support::paths::PathExt;
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

    assert_that(p.cargo_process("build"),
                execs().with_status(0).with_stdout(format!("\
{compiling} foo v0.0.1 ({dir})
", compiling = COMPILING, dir = path2url(p.root()))));

    assert_that(p.process(cargo_dir().join("cargo")).arg("build"),
                execs().with_status(0).with_stdout(format!("\
{fresh} foo v0.0.1 ({dir})
", fresh = FRESH, dir = path2url(p.root()))));
    p.root().move_into_the_past().assert();

    File::create(&p.root().join("src/a.rs")).write_str("fn main() {}").assert();
    assert_that(p.process(cargo_dir().join("cargo")).arg("build"),
                execs().with_status(0).with_stdout(format!("\
{compiling} foo v0.0.1 ({dir})
", compiling = COMPILING, dir = path2url(p.root()))));

    fs::rename(&p.root().join("src/a.rs"), &p.root().join("src/b.rs")).assert();
    assert_that(p.process(cargo_dir().join("cargo")).arg("build"),
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

    assert_that(p.cargo_process("build"),
                execs().with_status(0).with_stdout(format!("\
{compiling} foo v0.0.1 ({dir})
", compiling = COMPILING, dir = path2url(p.root()))));
    assert_that(p.process(cargo_dir().join("cargo")).arg("test"),
                execs().with_status(0));

    assert_that(&p.bin("foo"), existing_file());

    let lib = p.root().join("src/lib.rs");
    let bin = p.root().join("src/b.rs");

    File::create(&lib).write_str("invalid rust code").assert();
    lib.move_into_the_past().assert();
    p.root().move_into_the_past().assert();

    File::create(&bin).write_str("fn foo() {}").assert();

    // Make sure the binary is rebuilt, not the lib
    assert_that(p.process(cargo_dir().join("cargo")).arg("build")
                 .env("RUST_LOG", Some("cargo::ops::cargo_rustc::fingerprint")),
                execs().with_status(0).with_stdout(format!("\
{compiling} foo v0.0.1 ({dir})
", compiling = COMPILING, dir = path2url(p.root()))));
    assert_that(&p.bin("foo"), existing_file());
})
