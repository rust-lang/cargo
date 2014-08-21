use support::{project, execs, cargo_dir, path2url};
use support::{COMPILING, FRESH};
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

    assert_that(p.cargo_process("doc"),
                execs().with_status(0).with_stdout(format!("\
{compiling} foo v0.0.1 ({dir})
",
        compiling = COMPILING,
        dir = path2url(p.root())).as_slice()));
    assert_that(&p.root().join("target/doc"), existing_dir());
    assert_that(&p.root().join("target/doc/foo/index.html"), existing_file());
})

test!(doc_no_libs {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [[bin]]
            name = "foo"
            doc = false
        "#)
        .file("src/main.rs", r#"
            bad code
        "#);

    assert_that(p.cargo_process("doc"),
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

    assert_that(p.cargo_process("doc"),
                execs().with_status(0).with_stdout(format!("\
{compiling} foo v0.0.1 ({dir})
",
        compiling = COMPILING,
        dir = path2url(p.root())).as_slice()));

    assert_that(p.process(cargo_dir().join("cargo")).arg("doc"),
                execs().with_status(0).with_stdout(format!("\
{fresh} foo v0.0.1 ({dir})
",
        fresh = FRESH,
        dir = path2url(p.root())).as_slice()));
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

    assert_that(p.cargo_process("doc"),
                execs().with_status(0).with_stdout(format!("\
{compiling} bar v0.0.1 ({dir})
{compiling} foo v0.0.1 ({dir})
",
        compiling = COMPILING,
        dir = path2url(p.root())).as_slice()));

    assert_that(&p.root().join("target/doc"), existing_dir());
    assert_that(&p.root().join("target/doc/foo/index.html"), existing_file());
    assert_that(&p.root().join("target/doc/bar/index.html"), existing_file());

    assert_that(p.process(cargo_dir().join("cargo")).arg("doc")
                 .env("RUST_LOG", Some("cargo::ops::cargo_rustc::fingerprint")),
                execs().with_status(0).with_stdout(format!("\
{fresh} bar v0.0.1 ({dir})
{fresh} foo v0.0.1 ({dir})
",
        fresh = FRESH,
        dir = path2url(p.root())).as_slice()));

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

    assert_that(p.cargo_process("doc").arg("--no-deps"),
                execs().with_status(0).with_stdout(format!("\
{compiling} bar v0.0.1 ({dir})
{compiling} foo v0.0.1 ({dir})
",
        compiling = COMPILING,
        dir = path2url(p.root())).as_slice()));

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

    assert_that(p.cargo_process("doc"),
                execs().with_status(0));

    assert_that(&p.root().join("target/doc"), existing_dir());
    assert_that(&p.root().join("target/doc/bar/index.html"), existing_file());
    assert_that(&p.root().join("target/doc/foo/index.html"), existing_file());
})

test!(doc_lib_bin_same_name {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/main.rs", "fn main() {}")
        .file("src/lib.rs", "fn foo() {}");

    assert_that(p.cargo_process("doc"),
                execs().with_status(101)
                       .with_stderr("\
Cannot document a package where a library and a binary have the same name. \
Consider renaming one or marking the target as `doc = false`
"));
})
