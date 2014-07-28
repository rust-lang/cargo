use support::{project, execs};
use support::{COMPILING};
use hamcrest::{assert_that, existing_file, existing_dir, is_not};

fn setup() {
}

test!(simple {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/lib.rs", r#"
            pub fn foo() {}
        "#);

    assert_that(p.cargo_process("cargo-doc"),
                execs().with_status(0).with_stdout(format!("\
{compiling} foo v0.0.1 (file:{dir})
",
        compiling = COMPILING,
        dir = p.root().display()).as_slice()));
    assert_that(&p.root().join("target/doc"), existing_dir());
    assert_that(&p.root().join("target/doc/foo/index.html"), existing_file());
})

test!(no_build_main {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/lib.rs", r#"
            pub fn foo() {}
        "#)
        .file("src/main.rs", r#"
            bad code
        "#);

    assert_that(p.cargo_process("cargo-doc"),
                execs().with_status(0).with_stdout(format!("\
{compiling} foo v0.0.1 (file:{dir})
",
        compiling = COMPILING,
        dir = p.root().display()).as_slice()));
})

test!(doc_no_libs {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/main.rs", r#"
            bad code
        "#);

    assert_that(p.cargo_process("cargo-doc"),
                execs().with_status(0));
})

test!(doc_twice {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/lib.rs", r#"
            pub fn foo() {}
        "#);

    assert_that(p.cargo_process("cargo-doc"),
                execs().with_status(0).with_stdout(format!("\
{compiling} foo v0.0.1 (file:{dir})
",
        compiling = COMPILING,
        dir = p.root().display()).as_slice()));

    assert_that(p.cargo_process("cargo-doc"),
                execs().with_status(0).with_stdout(format!("\
{compiling} foo v0.0.1 (file:{dir})
",
        compiling = COMPILING,
        dir = p.root().display()).as_slice()));
})

test!(doc_deps {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.bar]
            path = "bar"
        "#)
        .file("src/lib.rs", r#"
            extern crate bar;
            pub fn foo() {}
        "#)
        .file("bar/Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.0.1"
            authors = []
        "#)
        .file("bar/src/lib.rs", r#"
            pub fn bar() {}
        "#);

    assert_that(p.cargo_process("cargo-doc"),
                execs().with_status(0).with_stdout(format!("\
{compiling} bar v0.0.1 (file:{dir})
{compiling} foo v0.0.1 (file:{dir})
",
        compiling = COMPILING,
        dir = p.root().display()).as_slice()));

    assert_that(&p.root().join("target/doc"), existing_dir());
    assert_that(&p.root().join("target/doc/foo/index.html"), existing_file());
    assert_that(&p.root().join("target/doc/bar/index.html"), existing_file());
})

test!(doc_no_deps {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.bar]
            path = "bar"
        "#)
        .file("src/lib.rs", r#"
            extern crate bar;
            pub fn foo() {}
        "#)
        .file("bar/Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.0.1"
            authors = []
        "#)
        .file("bar/src/lib.rs", r#"
            pub fn bar() {}
        "#);

    assert_that(p.cargo_process("cargo-doc").arg("--no-deps"),
                execs().with_status(0).with_stdout(format!("\
{compiling} bar v0.0.1 (file:{dir})
{compiling} foo v0.0.1 (file:{dir})
",
        compiling = COMPILING,
        dir = p.root().display()).as_slice()));

    assert_that(&p.root().join("target/doc"), existing_dir());
    assert_that(&p.root().join("target/doc/foo/index.html"), existing_file());
    assert_that(&p.root().join("target/doc/bar/index.html"), is_not(existing_file()));
})

test!(doc_only_bin {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.bar]
            path = "bar"
        "#)
        .file("src/main.rs", r#"
            extern crate bar;
            pub fn foo() {}
        "#)
        .file("bar/Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.0.1"
            authors = []
        "#)
        .file("bar/src/lib.rs", r#"
            pub fn bar() {}
        "#);

    assert_that(p.cargo_process("cargo-doc"),
                execs().with_status(0));

    assert_that(&p.root().join("target/doc"), existing_dir());
    assert_that(&p.root().join("target/doc/bar/index.html"), existing_file());
})
