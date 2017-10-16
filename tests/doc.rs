extern crate cargotest;
extern crate hamcrest;
extern crate cargo;

use std::str;
use std::fs::{self, File};
use std::io::Read;

use cargotest::rustc_host;
use cargotest::support::{project, execs, path2url};
use cargotest::support::registry::Package;
use hamcrest::{assert_that, existing_file, existing_dir, is_not};
use cargo::util::{CargoError, CargoErrorKind};

#[test]
fn simple() {
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
        "#)
        .build();

    assert_that(p.cargo("doc"),
                execs().with_status(0).with_stderr(&format!("\
[..] foo v0.0.1 ({dir})
[..] foo v0.0.1 ({dir})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        dir = path2url(p.root()))));
    assert_that(&p.root().join("target/doc"), existing_dir());
    assert_that(&p.root().join("target/doc/foo/index.html"), existing_file());
}

#[test]
fn doc_no_libs() {
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
        "#)
        .build();

    assert_that(p.cargo("doc"),
                execs().with_status(0));
}

#[test]
fn doc_twice() {
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
        .build();

    assert_that(p.cargo("doc"),
                execs().with_status(0).with_stderr(&format!("\
[DOCUMENTING] foo v0.0.1 ({dir})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        dir = path2url(p.root()))));

    assert_that(p.cargo("doc"),
                execs().with_status(0).with_stdout(""))
}

#[test]
fn doc_deps() {
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
        "#)
        .build();

    assert_that(p.cargo("doc"),
                execs().with_status(0).with_stderr(&format!("\
[..] bar v0.0.1 ({dir}/bar)
[..] bar v0.0.1 ({dir}/bar)
[DOCUMENTING] foo v0.0.1 ({dir})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
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
}

#[test]
fn doc_no_deps() {
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
        "#)
        .build();

    assert_that(p.cargo("doc").arg("--no-deps"),
                execs().with_status(0).with_stderr(&format!("\
[COMPILING] bar v0.0.1 ({dir}/bar)
[DOCUMENTING] foo v0.0.1 ({dir})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        dir = path2url(p.root()))));

    assert_that(&p.root().join("target/doc"), existing_dir());
    assert_that(&p.root().join("target/doc/foo/index.html"), existing_file());
    assert_that(&p.root().join("target/doc/bar/index.html"), is_not(existing_file()));
}

#[test]
fn doc_only_bin() {
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
        "#)
        .build();

    assert_that(p.cargo("doc").arg("-v"),
                execs().with_status(0));

    assert_that(&p.root().join("target/doc"), existing_dir());
    assert_that(&p.root().join("target/doc/bar/index.html"), existing_file());
    assert_that(&p.root().join("target/doc/foo/index.html"), existing_file());
}

#[test]
fn doc_multiple_targets_same_name_lib() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [workspace]
            members = ["foo", "bar"]
        "#)
        .file("foo/Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.1.0"
            [lib]
            name = "foo_lib"
        "#)
        .file("foo/src/lib.rs", "")
        .file("bar/Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.1.0"
            [lib]
            name = "foo_lib"
        "#)
        .file("bar/src/lib.rs", "")
        .build();

        assert_that(p.cargo("doc").arg("--all"),
                    execs()
                    .with_status(101)
                    .with_stderr_contains("[..] library `foo_lib` is specified [..]")
                    .with_stderr_contains("[..] `foo v0.1.0[..]` [..]")
                    .with_stderr_contains("[..] `bar v0.1.0[..]` [..]"));
}

#[test]
fn doc_multiple_targets_same_name() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [workspace]
            members = ["foo", "bar"]
        "#)
        .file("foo/Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.1.0"
            [[bin]]
            name = "foo_lib"
            path = "src/foo_lib.rs"
        "#)
        .file("foo/src/foo_lib.rs", "")
        .file("bar/Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.1.0"
            [lib]
            name = "foo_lib"
        "#)
        .file("bar/src/lib.rs", "")
        .build();

        let root = path2url(p.root());

        assert_that(p.cargo("doc").arg("--all"),
                    execs()
                    .with_status(0)
                    .with_stderr_contains(&format!("[DOCUMENTING] foo v0.1.0 ({}/foo)", root))
                    .with_stderr_contains(&format!("[DOCUMENTING] bar v0.1.0 ({}/bar)", root))
                    .with_stderr_contains("[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]"));
        assert_that(&p.root().join("target/doc"), existing_dir());
        let doc_file = p.root().join("target/doc/foo_lib/index.html");
        assert_that(&doc_file, existing_file());
}

#[test]
fn doc_multiple_targets_same_name_bin() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [workspace]
            members = ["foo", "bar"]
        "#)
        .file("foo/Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.1.0"
            [[bin]]
            name = "foo-cli"
        "#)
        .file("foo/src/foo-cli.rs", "")
        .file("bar/Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.1.0"
            [[bin]]
            name = "foo-cli"
        "#)
        .file("bar/src/foo-cli.rs", "")
        .build();

        assert_that(p.cargo("doc").arg("--all"),
                    execs()
                    .with_status(101)
                    .with_stderr_contains("[..] binary `foo_cli` is specified [..]")
                    .with_stderr_contains("[..] `foo v0.1.0[..]` [..]")
                    .with_stderr_contains("[..] `bar v0.1.0[..]` [..]"));
}

#[test]
fn doc_multiple_targets_same_name_undoced() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [workspace]
            members = ["foo", "bar"]
        "#)
        .file("foo/Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.1.0"
            [[bin]]
            name = "foo-cli"
        "#)
        .file("foo/src/foo-cli.rs", "")
        .file("bar/Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.1.0"
            [[bin]]
            name = "foo-cli"
            doc = false
        "#)
        .file("bar/src/foo-cli.rs", "")
        .build();

        assert_that(p.cargo("doc").arg("--all"),
                    execs().with_status(0));
}

#[test]
fn doc_lib_bin_same_name_documents_lib() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/main.rs", r#"
            //! Binary documentation
            extern crate foo;
            fn main() {
                foo::foo();
            }
        "#)
        .file("src/lib.rs", r#"
            //! Library documentation
            pub fn foo() {}
        "#)
        .build();

    assert_that(p.cargo("doc"),
                execs().with_status(0).with_stderr(&format!("\
[DOCUMENTING] foo v0.0.1 ({dir})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
", dir = path2url(p.root()))));
    assert_that(&p.root().join("target/doc"), existing_dir());
    let doc_file = p.root().join("target/doc/foo/index.html");
    assert_that(&doc_file, existing_file());
    let mut doc_html = String::new();
    File::open(&doc_file).unwrap().read_to_string(&mut doc_html).unwrap();
    assert!(doc_html.contains("Library"));
    assert!(!doc_html.contains("Binary"));
}

#[test]
fn doc_lib_bin_same_name_documents_lib_when_requested() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/main.rs", r#"
            //! Binary documentation
            extern crate foo;
            fn main() {
                foo::foo();
            }
        "#)
        .file("src/lib.rs", r#"
            //! Library documentation
            pub fn foo() {}
        "#)
        .build();

    assert_that(p.cargo("doc").arg("--lib"),
                execs().with_status(0).with_stderr(&format!("\
[DOCUMENTING] foo v0.0.1 ({dir})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
", dir = path2url(p.root()))));
    assert_that(&p.root().join("target/doc"), existing_dir());
    let doc_file = p.root().join("target/doc/foo/index.html");
    assert_that(&doc_file, existing_file());
    let mut doc_html = String::new();
    File::open(&doc_file).unwrap().read_to_string(&mut doc_html).unwrap();
    assert!(doc_html.contains("Library"));
    assert!(!doc_html.contains("Binary"));
}

#[test]
fn doc_lib_bin_same_name_documents_named_bin_when_requested() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/main.rs", r#"
            //! Binary documentation
            extern crate foo;
            fn main() {
                foo::foo();
            }
        "#)
        .file("src/lib.rs", r#"
            //! Library documentation
            pub fn foo() {}
        "#)
        .build();

    assert_that(p.cargo("doc").arg("--bin").arg("foo"),
                execs().with_status(0).with_stderr(&format!("\
[COMPILING] foo v0.0.1 ({dir})
[DOCUMENTING] foo v0.0.1 ({dir})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
", dir = path2url(p.root()))));
    assert_that(&p.root().join("target/doc"), existing_dir());
    let doc_file = p.root().join("target/doc/foo/index.html");
    assert_that(&doc_file, existing_file());
    let mut doc_html = String::new();
    File::open(&doc_file).unwrap().read_to_string(&mut doc_html).unwrap();
    assert!(!doc_html.contains("Library"));
    assert!(doc_html.contains("Binary"));
}

#[test]
fn doc_lib_bin_same_name_documents_bins_when_requested() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/main.rs", r#"
            //! Binary documentation
            extern crate foo;
            fn main() {
                foo::foo();
            }
        "#)
        .file("src/lib.rs", r#"
            //! Library documentation
            pub fn foo() {}
        "#)
        .build();

    assert_that(p.cargo("doc").arg("--bins"),
                execs().with_status(0).with_stderr(&format!("\
[COMPILING] foo v0.0.1 ({dir})
[DOCUMENTING] foo v0.0.1 ({dir})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
", dir = path2url(p.root()))));
    assert_that(&p.root().join("target/doc"), existing_dir());
    let doc_file = p.root().join("target/doc/foo/index.html");
    assert_that(&doc_file, existing_file());
    let mut doc_html = String::new();
    File::open(&doc_file).unwrap().read_to_string(&mut doc_html).unwrap();
    assert!(!doc_html.contains("Library"));
    assert!(doc_html.contains("Binary"));
}

#[test]
fn doc_dash_p() {
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
        .file("b/src/lib.rs", "")
        .build();

    assert_that(p.cargo("doc").arg("-p").arg("a"),
                execs().with_status(0)
                       .with_stderr("\
[..] b v0.0.1 (file://[..])
[..] b v0.0.1 (file://[..])
[DOCUMENTING] a v0.0.1 (file://[..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
"));
}

#[test]
fn doc_same_name() {
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
        .file("tests/main.rs", "fn main() {}")
        .build();

    assert_that(p.cargo("doc"),
                execs().with_status(0));
}

#[test]
fn doc_target() {
    const TARGET: &'static str = "arm-unknown-linux-gnueabihf";

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
        "#)
        .build();

    assert_that(p.cargo("doc").arg("--target").arg(TARGET).arg("--verbose"),
                execs().with_status(0));
    assert_that(&p.root().join(&format!("target/{}/doc", TARGET)), existing_dir());
    assert_that(&p.root().join(&format!("target/{}/doc/foo/index.html", TARGET)), existing_file());
}

#[test]
fn target_specific_not_documented() {
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
        .file("a/src/lib.rs", "not rust")
        .build();

    assert_that(p.cargo("doc"),
                execs().with_status(0));
}

#[test]
fn output_not_captured() {
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
        ")
        .build();

    let error = p.cargo("doc").exec_with_output().err().unwrap();
    if let CargoError(CargoErrorKind::ProcessErrorKind(perr), ..) = error {
        let output = perr.output.unwrap();
        let stderr = str::from_utf8(&output.stderr).unwrap();

        assert!(stderr.contains("☃"), "no snowman\n{}", stderr);
        assert!(stderr.contains("unknown start of token"), "no message{}", stderr);
    } else {
        assert!(false, "an error kind other than ProcessErrorKind was encountered");
    }
}

#[test]
fn target_specific_documented() {
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
        "#, rustc_host()))
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
        ")
        .build();

    assert_that(p.cargo("doc"),
                execs().with_status(0));
}

#[test]
fn no_document_build_deps() {
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
        ")
        .build();

    assert_that(p.cargo("doc"),
                execs().with_status(0));
}

#[test]
fn doc_release() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/lib.rs", "")
        .build();

    assert_that(p.cargo("build").arg("--release"),
                execs().with_status(0));
    assert_that(p.cargo("doc").arg("--release").arg("-v"),
                execs().with_status(0)
                       .with_stderr("\
[DOCUMENTING] foo v0.0.1 ([..])
[RUNNING] `rustdoc [..] src[/]lib.rs [..]`
[FINISHED] release [optimized] target(s) in [..]
"));
}

#[test]
fn doc_multiple_deps() {
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
        "#)
        .build();

    assert_that(p.cargo("doc")
                  .arg("-p").arg("bar")
                  .arg("-p").arg("baz")
                  .arg("-v"),
                execs().with_status(0));

    assert_that(&p.root().join("target/doc"), existing_dir());
    assert_that(&p.root().join("target/doc/bar/index.html"), existing_file());
    assert_that(&p.root().join("target/doc/baz/index.html"), existing_file());
}

#[test]
fn features() {
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
        "#)
        .build();
    assert_that(p.cargo("doc").arg("--features").arg("foo"),
                execs().with_status(0));
    assert_that(&p.root().join("target/doc"), existing_dir());
    assert_that(&p.root().join("target/doc/foo/fn.foo.html"), existing_file());
    assert_that(&p.root().join("target/doc/bar/fn.bar.html"), existing_file());
}

#[test]
fn rerun_when_dir_removed() {
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
        "#)
        .build();

    assert_that(p.cargo("doc"),
                execs().with_status(0));
    assert_that(&p.root().join("target/doc/foo/index.html"), existing_file());

    fs::remove_dir_all(p.root().join("target/doc/foo")).unwrap();

    assert_that(p.cargo("doc"),
                execs().with_status(0));
    assert_that(&p.root().join("target/doc/foo/index.html"), existing_file());
}

#[test]
fn document_only_lib() {
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
        "#)
        .file("src/bin/bar.rs", r#"
            /// ```
            /// ☃
            /// ```
            pub fn foo() {}
            fn main() { foo(); }
        "#)
        .build();
    assert_that(p.cargo("doc").arg("--lib"),
                execs().with_status(0));
    assert_that(&p.root().join("target/doc/foo/index.html"), existing_file());
}

#[test]
fn plugins_no_use_target() {
    if !cargotest::is_nightly() {
        return
    }
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [lib]
            proc-macro = true
        "#)
        .file("src/lib.rs", "")
        .build();
    assert_that(p.cargo("doc")
                 .arg("--target=x86_64-unknown-openbsd")
                 .arg("-v"),
                execs().with_status(0));
}

#[test]
fn doc_all_workspace() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.1.0"

            [dependencies]
            bar = { path = "bar" }

            [workspace]
        "#)
        .file("src/main.rs", r#"
            fn main() {}
        "#)
        .file("bar/Cargo.toml", r#"
            [project]
            name = "bar"
            version = "0.1.0"
        "#)
        .file("bar/src/lib.rs", r#"
            pub fn bar() {}
        "#)
        .build();

    // The order in which bar is compiled or documented is not deterministic
    assert_that(p.cargo("doc")
                 .arg("--all"),
                execs().with_status(0)
                       .with_stderr_contains("[..] Documenting bar v0.1.0 ([..])")
                       .with_stderr_contains("[..] Compiling bar v0.1.0 ([..])")
                       .with_stderr_contains("[..] Documenting foo v0.1.0 ([..])"));
}

#[test]
fn doc_all_virtual_manifest() {
    let p = project("workspace")
        .file("Cargo.toml", r#"
            [workspace]
            members = ["foo", "bar"]
        "#)
        .file("foo/Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.1.0"
        "#)
        .file("foo/src/lib.rs", r#"
            pub fn foo() {}
        "#)
        .file("bar/Cargo.toml", r#"
            [project]
            name = "bar"
            version = "0.1.0"
        "#)
        .file("bar/src/lib.rs", r#"
            pub fn bar() {}
        "#)
        .build();

    // The order in which foo and bar are documented is not guaranteed
    assert_that(p.cargo("doc")
                 .arg("--all"),
                execs().with_status(0)
                       .with_stderr_contains("[..] Documenting bar v0.1.0 ([..])")
                       .with_stderr_contains("[..] Documenting foo v0.1.0 ([..])"));
}

#[test]
fn doc_virtual_manifest_all_implied() {
    let p = project("workspace")
        .file("Cargo.toml", r#"
            [workspace]
            members = ["foo", "bar"]
        "#)
        .file("foo/Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.1.0"
        "#)
        .file("foo/src/lib.rs", r#"
            pub fn foo() {}
        "#)
        .file("bar/Cargo.toml", r#"
            [project]
            name = "bar"
            version = "0.1.0"
        "#)
        .file("bar/src/lib.rs", r#"
            pub fn bar() {}
        "#)
        .build();

    // The order in which foo and bar are documented is not guaranteed
    assert_that(p.cargo("doc"),
                execs().with_status(0)
                       .with_stderr_contains("[..] Documenting bar v0.1.0 ([..])")
                       .with_stderr_contains("[..] Documenting foo v0.1.0 ([..])"));
}

#[test]
fn doc_all_member_dependency_same_name() {
    let p = project("workspace")
        .file("Cargo.toml", r#"
            [workspace]
            members = ["a"]
        "#)
        .file("a/Cargo.toml", r#"
            [project]
            name = "a"
            version = "0.1.0"

            [dependencies]
            a = "0.1.0"
        "#)
        .file("a/src/lib.rs", r#"
            pub fn a() {}
        "#)
        .build();

    Package::new("a", "0.1.0").publish();

    assert_that(p.cargo("doc")
                 .arg("--all"),
                execs().with_status(0)
                       .with_stderr_contains("[..] Updating registry `[..]`")
                       .with_stderr_contains("[..] Documenting a v0.1.0 ([..])"));
}
