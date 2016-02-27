use std::str;
use std::fs;

use support::{project, execs, path2url};
use support::{COMPILING, DOCUMENTING, RUNNING, ERROR};
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
            build = "build.rs"
        "#)
        .file("build.rs", "fn main() {}")
        .file("src/lib.rs", r#"
            pub fn foo() {}
        "#);

    assert_that(p.cargo_process("doc"),
                execs().with_status(0).with_stdout(&format!("\
[..] foo v0.0.1 ({dir})
[..] foo v0.0.1 ({dir})
",
        dir = path2url(p.root()))));
    assert_that(&p.root().join("target/doc"), existing_dir());
    assert_that(&p.root().join("target/doc/foo/index.html"), existing_file());
});

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
});

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
                execs().with_status(0).with_stdout(&format!("\
{documenting} foo v0.0.1 ({dir})
",
        documenting = DOCUMENTING,
        dir = path2url(p.root()))));

    assert_that(p.cargo("doc"),
                execs().with_status(0).with_stdout(""))
});

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
                execs().with_status(0).with_stdout(&format!("\
[..] bar v0.0.1 ({dir}/bar)
[..] bar v0.0.1 ({dir}/bar)
{documenting} foo v0.0.1 ({dir})
",
        documenting = DOCUMENTING,
        dir = path2url(p.root()))));

    assert_that(&p.root().join("target/doc"), existing_dir());
    assert_that(&p.root().join("target/doc/foo/index.html"), existing_file());
    assert_that(&p.root().join("target/doc/bar/index.html"), existing_file());

    assert_that(p.cargo("doc")
                 .env("RUST_LOG", "cargo::ops::cargo_rustc::fingerprint"),
                execs().with_status(0).with_stdout(""));

    assert_that(&p.root().join("target/doc"), existing_dir());
    assert_that(&p.root().join("target/doc/foo/index.html"), existing_file());
    assert_that(&p.root().join("target/doc/bar/index.html"), existing_file());
});

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
                execs().with_status(0).with_stdout(&format!("\
{compiling} bar v0.0.1 ({dir}/bar)
{documenting} foo v0.0.1 ({dir})
",
        documenting = DOCUMENTING, compiling = COMPILING,
        dir = path2url(p.root()))));

    assert_that(&p.root().join("target/doc"), existing_dir());
    assert_that(&p.root().join("target/doc/foo/index.html"), existing_file());
    assert_that(&p.root().join("target/doc/bar/index.html"), is_not(existing_file()));
});

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

    assert_that(p.cargo_process("doc").arg("-v"),
                execs().with_status(0));

    assert_that(&p.root().join("target/doc"), existing_dir());
    assert_that(&p.root().join("target/doc/bar/index.html"), existing_file());
    assert_that(&p.root().join("target/doc/foo/index.html"), existing_file());
});

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
                       .with_stderr(&format!("\
{error} cannot document a package where a library and a binary have the same name. \
Consider renaming one or marking the target as `doc = false`
",
error = ERROR)));
});

test!(doc_dash_p {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.a]
            path = "a"
        "#)
        .file("src/lib.rs", "extern crate a;")
        .file("a/Cargo.toml", r#"
            [package]
            name = "a"
            version = "0.0.1"
            authors = []

            [dependencies.b]
            path = "../b"
        "#)
        .file("a/src/lib.rs", "extern crate b;")
        .file("b/Cargo.toml", r#"
            [package]
            name = "b"
            version = "0.0.1"
            authors = []
        "#)
        .file("b/src/lib.rs", "");

    assert_that(p.cargo_process("doc").arg("-p").arg("a"),
                execs().with_status(0)
                       .with_stdout(&format!("\
[..] b v0.0.1 (file://[..])
[..] b v0.0.1 (file://[..])
{documenting} a v0.0.1 (file://[..])
", documenting = DOCUMENTING)));
});

test!(doc_same_name {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/lib.rs", "")
        .file("src/bin/main.rs", "fn main() {}")
        .file("examples/main.rs", "fn main() {}")
        .file("tests/main.rs", "fn main() {}");

    assert_that(p.cargo_process("doc"),
                execs().with_status(0));
});

test!(doc_target {
    const TARGET: &'static str = "arm-unknown-linux-gnueabihf";

    if !::is_nightly() { return }

    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/lib.rs", r#"
            #![feature(no_core)]
            #![no_core]

            extern {
                pub static A: u32;
            }
        "#);

    assert_that(p.cargo_process("doc").arg("--target").arg(TARGET).arg("--verbose"),
                execs().with_status(0));
    assert_that(&p.root().join(&format!("target/{}/doc", TARGET)), existing_dir());
    assert_that(&p.root().join(&format!("target/{}/doc/foo/index.html", TARGET)), existing_file());
});

test!(target_specific_not_documented {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [target.foo.dependencies]
            a = { path = "a" }
        "#)
        .file("src/lib.rs", "")
        .file("a/Cargo.toml", r#"
            [package]
            name = "a"
            version = "0.0.1"
            authors = []
        "#)
        .file("a/src/lib.rs", "not rust");

    assert_that(p.cargo_process("doc"),
                execs().with_status(0));
});

test!(output_not_captured {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            a = { path = "a" }
        "#)
        .file("src/lib.rs", "")
        .file("a/Cargo.toml", r#"
            [package]
            name = "a"
            version = "0.0.1"
            authors = []
        "#)
        .file("a/src/lib.rs", "
            /// ```
            /// ☃
            /// ```
            pub fn foo() {}
        ");

    let output = p.cargo_process("doc").exec_with_output().err().unwrap()
                                                          .output.unwrap();
    let stderr = str::from_utf8(&output.stderr).unwrap();
    assert!(stderr.contains("☃"), "no snowman\n{}", stderr);
    assert!(stderr.contains("unknown start of token"), "no message\n{}", stderr);
});

test!(target_specific_documented {
    let p = project("foo")
        .file("Cargo.toml", &format!(r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [target.foo.dependencies]
            a = {{ path = "a" }}
            [target.{}.dependencies]
            a = {{ path = "a" }}
        "#, ::rustc_host()))
        .file("src/lib.rs", "
            extern crate a;

            /// test
            pub fn foo() {}
        ")
        .file("a/Cargo.toml", r#"
            [package]
            name = "a"
            version = "0.0.1"
            authors = []
        "#)
        .file("a/src/lib.rs", "
            /// test
            pub fn foo() {}
        ");

    assert_that(p.cargo_process("doc"),
                execs().with_status(0));
});

test!(no_document_build_deps {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [build-dependencies]
            a = { path = "a" }
        "#)
        .file("src/lib.rs", "
            pub fn foo() {}
        ")
        .file("a/Cargo.toml", r#"
            [package]
            name = "a"
            version = "0.0.1"
            authors = []
        "#)
        .file("a/src/lib.rs", "
            /// ```
            /// ☃
            /// ```
            pub fn foo() {}
        ");

    assert_that(p.cargo_process("doc"),
                execs().with_status(0));
});

test!(doc_release {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/lib.rs", "");

    assert_that(p.cargo_process("build").arg("--release"),
                execs().with_status(0));
    assert_that(p.cargo("doc").arg("--release").arg("-v"),
                execs().with_status(0)
                       .with_stdout(&format!("\
{documenting} foo v0.0.1 ([..])
{running} `rustdoc src[..]lib.rs [..]`
", documenting = DOCUMENTING, running = RUNNING)));
});

test!(doc_multiple_deps {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.bar]
            path = "bar"

            [dependencies.baz]
            path = "baz"
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
        "#)
        .file("baz/Cargo.toml", r#"
            [package]
            name = "baz"
            version = "0.0.1"
            authors = []
        "#)
        .file("baz/src/lib.rs", r#"
            pub fn baz() {}
        "#);

    assert_that(p.cargo_process("doc")
                  .arg("-p").arg("bar")
                  .arg("-p").arg("baz")
                  .arg("-v"),
                execs().with_status(0));

    assert_that(&p.root().join("target/doc"), existing_dir());
    assert_that(&p.root().join("target/doc/bar/index.html"), existing_file());
    assert_that(&p.root().join("target/doc/baz/index.html"), existing_file());
});

test!(features {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.bar]
            path = "bar"

            [features]
            foo = ["bar/bar"]
        "#)
        .file("src/lib.rs", r#"
            #[cfg(feature = "foo")]
            pub fn foo() {}
        "#)
        .file("bar/Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.0.1"
            authors = []

            [features]
            bar = []
        "#)
        .file("bar/build.rs", r#"
            fn main() {
                println!("cargo:rustc-cfg=bar");
            }
        "#)
        .file("bar/src/lib.rs", r#"
            #[cfg(feature = "bar")]
            pub fn bar() {}
        "#);
    assert_that(p.cargo_process("doc").arg("--features").arg("foo"),
                execs().with_status(0));
    assert_that(&p.root().join("target/doc"), existing_dir());
    assert_that(&p.root().join("target/doc/foo/fn.foo.html"), existing_file());
    assert_that(&p.root().join("target/doc/bar/fn.bar.html"), existing_file());
});

test!(rerun_when_dir_removed {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/lib.rs", r#"
            /// dox
            pub fn foo() {}
        "#);
    assert_that(p.cargo_process("doc"),
                execs().with_status(0));
    assert_that(&p.root().join("target/doc/foo/index.html"), existing_file());

    fs::remove_dir_all(p.root().join("target/doc/foo")).unwrap();

    assert_that(p.cargo_process("doc"),
                execs().with_status(0));
    assert_that(&p.root().join("target/doc/foo/index.html"), existing_file());
});
