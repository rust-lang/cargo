//! Tests for the `cargo doc` command.

use cargo::core::compiler::RustDocFingerprint;
use cargo_test_support::paths::CargoPathExt;
use cargo_test_support::registry::Package;
use cargo_test_support::{basic_lib_manifest, basic_manifest, git, project};
use cargo_test_support::{is_nightly, rustc_host, symlink_supported};
use std::fs;
use std::str;

#[cargo_test]
fn simple() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []
                build = "build.rs"
            "#,
        )
        .file("build.rs", "fn main() {}")
        .file("src/lib.rs", "pub fn foo() {}")
        .build();

    p.cargo("doc")
        .with_stderr(
            "\
[..] foo v0.0.1 ([CWD])
[..] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
    assert!(p
        .root()
        .join("target")
        .join(rustc_host())
        .join("doc")
        .is_dir());
    assert!(p
        .root()
        .join("target")
        .join(rustc_host())
        .join("doc/foo/index.html")
        .is_file());
}

#[cargo_test]
fn doc_no_libs() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [[bin]]
                name = "foo"
                doc = false
            "#,
        )
        .file("src/main.rs", "bad code")
        .build();

    p.cargo("doc").run();
}

#[cargo_test]
fn doc_twice() {
    let p = project().file("src/lib.rs", "pub fn foo() {}").build();

    p.cargo("doc")
        .with_stderr(
            "\
[DOCUMENTING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();

    p.cargo("doc").with_stdout("").run();
}

#[cargo_test]
fn doc_deps() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies.bar]
                path = "bar"
            "#,
        )
        .file("src/lib.rs", "extern crate bar; pub fn foo() {}")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file("bar/src/lib.rs", "pub fn bar() {}")
        .build();

    p.cargo("doc")
        .with_stderr(
            "\
[..] bar v0.0.1 ([CWD]/bar)
[..] bar v0.0.1 ([CWD]/bar)
[DOCUMENTING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();

    assert!(p
        .root()
        .join("target")
        .join(rustc_host())
        .join("doc")
        .is_dir());
    assert!(p
        .root()
        .join("target")
        .join(rustc_host())
        .join("doc/foo/index.html")
        .is_file());
    assert!(p
        .root()
        .join("target")
        .join(rustc_host())
        .join("doc/bar/index.html")
        .is_file());

    // Verify that it only emits rmeta for the dependency.
    assert_eq!(
        p.glob(format!("target/{}/debug/**/*.rlib", rustc_host()))
            .count(),
        0
    );
    assert_eq!(
        p.glob(format!("target/{}/debug/deps/libbar-*.rmeta", rustc_host()))
            .count(),
        1
    );

    p.cargo("doc")
        .env("CARGO_LOG", "cargo::ops::cargo_rustc::fingerprint")
        .with_stdout("")
        .run();

    assert!(p
        .root()
        .join("target")
        .join(rustc_host())
        .join("doc")
        .is_dir());
    assert!(p
        .root()
        .join("target")
        .join(rustc_host())
        .join("doc/foo/index.html")
        .is_file());
    assert!(p
        .root()
        .join("target")
        .join(rustc_host())
        .join("doc/bar/index.html")
        .is_file());
}

#[cargo_test]
fn doc_no_deps() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies.bar]
                path = "bar"
            "#,
        )
        .file("src/lib.rs", "extern crate bar; pub fn foo() {}")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file("bar/src/lib.rs", "pub fn bar() {}")
        .build();

    p.cargo("doc --no-deps")
        .with_stderr(
            "\
[CHECKING] bar v0.0.1 ([CWD]/bar)
[DOCUMENTING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();

    assert!(p
        .root()
        .join("target")
        .join(rustc_host())
        .join("doc")
        .is_dir());
    assert!(p
        .root()
        .join("target")
        .join(rustc_host())
        .join("doc/foo/index.html")
        .is_file());
    assert!(!p
        .root()
        .join("target")
        .join(rustc_host())
        .join("doc/bar/index.html")
        .is_file());
}

#[cargo_test]
fn doc_only_bin() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies.bar]
                path = "bar"
            "#,
        )
        .file("src/main.rs", "extern crate bar; pub fn foo() {}")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file("bar/src/lib.rs", "pub fn bar() {}")
        .build();

    p.cargo("doc -v").run();

    assert!(p
        .root()
        .join("target")
        .join(rustc_host())
        .join("doc")
        .is_dir());
    assert!(p
        .root()
        .join("target")
        .join(rustc_host())
        .join("doc/bar/index.html")
        .is_file());
    assert!(p
        .root()
        .join("target")
        .join(rustc_host())
        .join("doc/foo/index.html")
        .is_file());
}

#[cargo_test]
fn doc_multiple_targets_same_name_lib() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["foo", "bar"]
            "#,
        )
        .file(
            "foo/Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                [lib]
                name = "foo_lib"
            "#,
        )
        .file("foo/src/lib.rs", "")
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.1.0"
                [lib]
                name = "foo_lib"
            "#,
        )
        .file("bar/src/lib.rs", "")
        .build();

    p.cargo("doc --workspace")
        .with_status(101)
        .with_stderr(
            "\
error: document output filename collision
The lib `foo_lib` in package `foo v0.1.0 ([ROOT]/foo/foo)` has the same name as \
the lib `foo_lib` in package `bar v0.1.0 ([ROOT]/foo/bar)`.
Only one may be documented at once since they output to the same path.
Consider documenting only one, renaming one, or marking one with `doc = false` in Cargo.toml.
",
        )
        .run();
}

#[cargo_test]
fn doc_multiple_targets_same_name() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["foo", "bar"]
            "#,
        )
        .file(
            "foo/Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                [[bin]]
                name = "foo_lib"
                path = "src/foo_lib.rs"
            "#,
        )
        .file("foo/src/foo_lib.rs", "")
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.1.0"
                [lib]
                name = "foo_lib"
            "#,
        )
        .file("bar/src/lib.rs", "")
        .build();

    p.cargo("doc --workspace")
        .with_stderr_unordered(&format!(
            "\
warning: output filename collision.
The bin target `foo_lib` in package `foo v0.1.0 ([ROOT]/foo/foo)` \
has the same output filename as the lib target `foo_lib` in package \
`bar v0.1.0 ([ROOT]/foo/bar)`.
Colliding filename is: [ROOT]/foo/target/{}/doc/foo_lib/index.html
The targets should have unique names.
This is a known bug where multiple crates with the same name use
the same path; see <https://github.com/rust-lang/cargo/issues/6313>.
[DOCUMENTING] bar v0.1.0 ([ROOT]/foo/bar)
[DOCUMENTING] foo v0.1.0 ([ROOT]/foo/foo)
[FINISHED] [..]
",
            rustc_host()
        ))
        .run();
}

#[cargo_test]
fn doc_multiple_targets_same_name_bin() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["foo", "bar"]
            "#,
        )
        .file(
            "foo/Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
            "#,
        )
        .file("foo/src/bin/foo-cli.rs", "")
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.1.0"
            "#,
        )
        .file("bar/src/bin/foo-cli.rs", "")
        .build();

    p.cargo("doc --workspace")
        .with_status(101)
        .with_stderr(
            "\
error: document output filename collision
The bin `foo-cli` in package `foo v0.1.0 ([ROOT]/foo/foo)` has the same name as \
the bin `foo-cli` in package `bar v0.1.0 ([ROOT]/foo/bar)`.
Only one may be documented at once since they output to the same path.
Consider documenting only one, renaming one, or marking one with `doc = false` in Cargo.toml.
",
        )
        .run();
}

#[cargo_test]
fn doc_multiple_targets_same_name_undoced() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["foo", "bar"]
            "#,
        )
        .file(
            "foo/Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                [[bin]]
                name = "foo-cli"
            "#,
        )
        .file("foo/src/foo-cli.rs", "")
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.1.0"
                [[bin]]
                name = "foo-cli"
                doc = false
            "#,
        )
        .file("bar/src/foo-cli.rs", "")
        .build();

    p.cargo("doc --workspace").run();
}

#[cargo_test]
fn doc_lib_bin_same_name_documents_lib() {
    let p = project()
        .file(
            "src/main.rs",
            r#"
                //! Binary documentation
                extern crate foo;
                fn main() {
                    foo::foo();
                }
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
                //! Library documentation
                pub fn foo() {}
            "#,
        )
        .build();

    p.cargo("doc")
        .with_stderr(
            "\
[DOCUMENTING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
    let doc_html = p.read_file(&format!("target/{}/doc/foo/index.html", rustc_host()));
    assert!(doc_html.contains("Library"));
    assert!(!doc_html.contains("Binary"));
}

#[cargo_test]
fn doc_lib_bin_same_name_documents_lib_when_requested() {
    let p = project()
        .file(
            "src/main.rs",
            r#"
                //! Binary documentation
                extern crate foo;
                fn main() {
                    foo::foo();
                }
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
                //! Library documentation
                pub fn foo() {}
            "#,
        )
        .build();

    p.cargo("doc --lib")
        .with_stderr(
            "\
[DOCUMENTING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
    let doc_html = p.read_file(&format!("target/{}/doc/foo/index.html", rustc_host()));
    assert!(doc_html.contains("Library"));
    assert!(!doc_html.contains("Binary"));
}

#[cargo_test]
fn doc_lib_bin_same_name_documents_named_bin_when_requested() {
    let p = project()
        .file(
            "src/main.rs",
            r#"
                //! Binary documentation
                extern crate foo;
                fn main() {
                    foo::foo();
                }
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
                //! Library documentation
                pub fn foo() {}
            "#,
        )
        .build();

    p.cargo("doc --bin foo")
        .with_stderr(
            "\
[CHECKING] foo v0.0.1 ([CWD])
[DOCUMENTING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
    let doc_html = p.read_file(&format!("target/{}/doc/foo/index.html", rustc_host()));
    assert!(!doc_html.contains("Library"));
    assert!(doc_html.contains("Binary"));
}

#[cargo_test]
fn doc_lib_bin_same_name_documents_bins_when_requested() {
    let p = project()
        .file(
            "src/main.rs",
            r#"
                //! Binary documentation
                extern crate foo;
                fn main() {
                    foo::foo();
                }
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
                //! Library documentation
                pub fn foo() {}
            "#,
        )
        .build();

    p.cargo("doc --bins")
        .with_stderr(
            "\
[CHECKING] foo v0.0.1 ([CWD])
[DOCUMENTING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
    let doc_html = p.read_file(&format!("target/{}/doc/foo/index.html", rustc_host()));
    assert!(!doc_html.contains("Library"));
    assert!(doc_html.contains("Binary"));
}

#[cargo_test]
fn doc_dash_p() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies.a]
                path = "a"
            "#,
        )
        .file("src/lib.rs", "extern crate a;")
        .file(
            "a/Cargo.toml",
            r#"
                [package]
                name = "a"
                version = "0.0.1"
                authors = []

                [dependencies.b]
                path = "../b"
            "#,
        )
        .file("a/src/lib.rs", "extern crate b;")
        .file("b/Cargo.toml", &basic_manifest("b", "0.0.1"))
        .file("b/src/lib.rs", "")
        .build();

    p.cargo("doc -p a")
        .with_stderr(
            "\
[..] b v0.0.1 ([CWD]/b)
[..] b v0.0.1 ([CWD]/b)
[DOCUMENTING] a v0.0.1 ([CWD]/a)
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cargo_test]
fn doc_all_exclude() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["bar", "baz"]
            "#,
        )
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "pub fn bar() {}")
        .file("baz/Cargo.toml", &basic_manifest("baz", "0.1.0"))
        .file("baz/src/lib.rs", "pub fn baz() { break_the_build(); }")
        .build();

    p.cargo("doc --workspace --exclude baz")
        .with_stderr_does_not_contain("[DOCUMENTING] baz v0.1.0 [..]")
        .with_stderr(
            "\
[DOCUMENTING] bar v0.1.0 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cargo_test]
fn doc_all_exclude_glob() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["bar", "baz"]
            "#,
        )
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "pub fn bar() {}")
        .file("baz/Cargo.toml", &basic_manifest("baz", "0.1.0"))
        .file("baz/src/lib.rs", "pub fn baz() { break_the_build(); }")
        .build();

    p.cargo("doc --workspace --exclude '*z'")
        .with_stderr_does_not_contain("[DOCUMENTING] baz v0.1.0 [..]")
        .with_stderr(
            "\
[DOCUMENTING] bar v0.1.0 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cargo_test]
fn doc_same_name() {
    let p = project()
        .file("src/lib.rs", "")
        .file("src/bin/main.rs", "fn main() {}")
        .file("examples/main.rs", "fn main() {}")
        .file("tests/main.rs", "fn main() {}")
        .build();

    p.cargo("doc").run();
}

#[cargo_test]
fn doc_target() {
    if !is_nightly() {
        // no_core, lang_items requires nightly.
        return;
    }
    const TARGET: &str = "arm-unknown-linux-gnueabihf";

    let p = project()
        .file(
            "src/lib.rs",
            r#"
                #![feature(no_core, lang_items)]
                #![no_core]

                #[lang = "sized"]
                trait Sized {}

                extern {
                    pub static A: u32;
                }
            "#,
        )
        .build();

    p.cargo("doc --verbose --target").arg(TARGET).run();
    assert!(p.root().join(&format!("target/{}/doc", TARGET)).is_dir());
    assert!(p
        .root()
        .join(&format!("target/{}/doc/foo/index.html", TARGET))
        .is_file());
}

#[cargo_test]
fn target_specific_not_documented() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [target.foo.dependencies]
                a = { path = "a" }
            "#,
        )
        .file("src/lib.rs", "")
        .file("a/Cargo.toml", &basic_manifest("a", "0.0.1"))
        .file("a/src/lib.rs", "not rust")
        .build();

    p.cargo("doc").run();
}

#[cargo_test]
fn output_not_captured() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies]
                a = { path = "a" }
            "#,
        )
        .file("src/lib.rs", "")
        .file("a/Cargo.toml", &basic_manifest("a", "0.0.1"))
        .file(
            "a/src/lib.rs",
            "
            /// ```
            /// ☃
            /// ```
            pub fn foo() {}
        ",
        )
        .build();

    p.cargo("doc")
        .without_status()
        .with_stderr_contains("[..]☃")
        .with_stderr_contains(r"[..]unknown start of token: \u{2603}")
        .run();
}

#[cargo_test]
fn target_specific_documented() {
    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"
                    version = "0.0.1"
                    authors = []

                    [target.foo.dependencies]
                    a = {{ path = "a" }}
                    [target.{}.dependencies]
                    a = {{ path = "a" }}
                "#,
                rustc_host()
            ),
        )
        .file(
            "src/lib.rs",
            "
            extern crate a;

            /// test
            pub fn foo() {}
        ",
        )
        .file("a/Cargo.toml", &basic_manifest("a", "0.0.1"))
        .file(
            "a/src/lib.rs",
            "
            /// test
            pub fn foo() {}
        ",
        )
        .build();

    p.cargo("doc").run();
}

#[cargo_test]
fn no_document_build_deps() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [build-dependencies]
                a = { path = "a" }
            "#,
        )
        .file("src/lib.rs", "pub fn foo() {}")
        .file("a/Cargo.toml", &basic_manifest("a", "0.0.1"))
        .file(
            "a/src/lib.rs",
            "
            /// ```
            /// ☃
            /// ```
            pub fn foo() {}
        ",
        )
        .build();

    p.cargo("doc").run();
}

#[cargo_test]
fn doc_release() {
    let p = project().file("src/lib.rs", "").build();

    p.cargo("build --release").run();
    p.cargo("doc --release -v")
        .with_stderr(
            "\
[DOCUMENTING] foo v0.0.1 ([..])
[RUNNING] `rustdoc [..] src/lib.rs [..]`
[FINISHED] release [optimized] target(s) in [..]
",
        )
        .run();
}

#[cargo_test]
fn doc_multiple_deps() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies.bar]
                path = "bar"

                [dependencies.baz]
                path = "baz"
            "#,
        )
        .file("src/lib.rs", "extern crate bar; pub fn foo() {}")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file("bar/src/lib.rs", "pub fn bar() {}")
        .file("baz/Cargo.toml", &basic_manifest("baz", "0.0.1"))
        .file("baz/src/lib.rs", "pub fn baz() {}")
        .build();

    p.cargo("doc -p bar -p baz -v").run();

    assert!(p
        .root()
        .join("target")
        .join(rustc_host())
        .join("doc")
        .is_dir());
    assert!(p
        .root()
        .join("target")
        .join(rustc_host())
        .join("doc/bar/index.html")
        .is_file());
    assert!(p
        .root()
        .join("target")
        .join(rustc_host())
        .join("doc/baz/index.html")
        .is_file());
}

#[cargo_test]
fn features() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies.bar]
                path = "bar"

                [features]
                foo = ["bar/bar"]
            "#,
        )
        .file("src/lib.rs", r#"#[cfg(feature = "foo")] pub fn foo() {}"#)
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.0.1"
                authors = []

                [features]
                bar = []
            "#,
        )
        .file(
            "bar/build.rs",
            r#"
                fn main() {
                    println!("cargo:rustc-cfg=bar");
                }
            "#,
        )
        .file(
            "bar/src/lib.rs",
            r#"#[cfg(feature = "bar")] pub fn bar() {}"#,
        )
        .build();
    p.cargo("doc --features foo")
        .with_stderr(
            "\
[COMPILING] bar v0.0.1 [..]
[DOCUMENTING] bar v0.0.1 [..]
[DOCUMENTING] foo v0.0.1 [..]
[FINISHED] [..]
",
        )
        .run();
    assert!(p
        .root()
        .join("target")
        .join(rustc_host())
        .join("doc")
        .is_dir());
    assert!(p
        .root()
        .join("target")
        .join(rustc_host())
        .join("doc/foo/fn.foo.html")
        .is_file());
    assert!(p
        .root()
        .join("target")
        .join(rustc_host())
        .join("doc/bar/fn.bar.html")
        .is_file());
    // Check that turning the feature off will remove the files.
    p.cargo("doc")
        .with_stderr(
            "\
[COMPILING] bar v0.0.1 [..]
[DOCUMENTING] bar v0.0.1 [..]
[DOCUMENTING] foo v0.0.1 [..]
[FINISHED] [..]
",
        )
        .run();
    assert!(!p
        .root()
        .join("target")
        .join(rustc_host())
        .join("doc/foo/fn.foo.html")
        .is_file());
    assert!(!p
        .root()
        .join("target")
        .join(rustc_host())
        .join("doc/bar/fn.bar.html")
        .is_file());
    // And switching back will rebuild and bring them back.
    p.cargo("doc --features foo")
        .with_stderr(
            "\
[DOCUMENTING] bar v0.0.1 [..]
[DOCUMENTING] foo v0.0.1 [..]
[FINISHED] [..]
",
        )
        .run();
    assert!(p
        .root()
        .join("target")
        .join(rustc_host())
        .join("doc/foo/fn.foo.html")
        .is_file());
    assert!(p
        .root()
        .join("target")
        .join(rustc_host())
        .join("doc/bar/fn.bar.html")
        .is_file());
}

#[cargo_test]
fn rerun_when_dir_removed() {
    let p = project()
        .file(
            "src/lib.rs",
            r#"
                /// dox
                pub fn foo() {}
            "#,
        )
        .build();

    p.cargo("doc").run();
    assert!(p
        .root()
        .join("target")
        .join(rustc_host())
        .join("doc/foo/index.html")
        .is_file());

    fs::remove_dir_all(p.root().join("target").join(rustc_host()).join("doc/foo")).unwrap();

    p.cargo("doc").run();
    assert!(p
        .root()
        .join("target")
        .join(rustc_host())
        .join("doc/foo/index.html")
        .is_file());
}

#[cargo_test]
fn document_only_lib() {
    let p = project()
        .file(
            "src/lib.rs",
            r#"
                /// dox
                pub fn foo() {}
            "#,
        )
        .file(
            "src/bin/bar.rs",
            r#"
                /// ```
                /// ☃
                /// ```
                pub fn foo() {}
                fn main() { foo(); }
            "#,
        )
        .build();
    p.cargo("doc --lib").run();
    assert!(p
        .root()
        .join("target")
        .join(rustc_host())
        .join("doc/foo/index.html")
        .is_file());
}

#[cargo_test]
fn plugins_no_use_target() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [lib]
                proc-macro = true
            "#,
        )
        .file("src/lib.rs", "")
        .build();
    p.cargo("doc --target=x86_64-unknown-openbsd -v").run();
}

#[cargo_test]
fn doc_all_workspace() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.1.0"

                [dependencies]
                bar = { path = "bar" }

                [workspace]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "pub fn bar() {}")
        .build();

    // The order in which bar is compiled or documented is not deterministic
    p.cargo("doc --workspace")
        .with_stderr_contains("[..] Documenting bar v0.1.0 ([..])")
        .with_stderr_contains("[..] Checking bar v0.1.0 ([..])")
        .with_stderr_contains("[..] Documenting foo v0.1.0 ([..])")
        .run();
}

#[cargo_test]
fn doc_all_virtual_manifest() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["bar", "baz"]
            "#,
        )
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "pub fn bar() {}")
        .file("baz/Cargo.toml", &basic_manifest("baz", "0.1.0"))
        .file("baz/src/lib.rs", "pub fn baz() {}")
        .build();

    // The order in which bar and baz are documented is not guaranteed
    p.cargo("doc --workspace")
        .with_stderr_contains("[..] Documenting baz v0.1.0 ([..])")
        .with_stderr_contains("[..] Documenting bar v0.1.0 ([..])")
        .run();
}

#[cargo_test]
fn doc_virtual_manifest_all_implied() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["bar", "baz"]
            "#,
        )
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "pub fn bar() {}")
        .file("baz/Cargo.toml", &basic_manifest("baz", "0.1.0"))
        .file("baz/src/lib.rs", "pub fn baz() {}")
        .build();

    // The order in which bar and baz are documented is not guaranteed
    p.cargo("doc")
        .with_stderr_contains("[..] Documenting baz v0.1.0 ([..])")
        .with_stderr_contains("[..] Documenting bar v0.1.0 ([..])")
        .run();
}

#[cargo_test]
fn doc_virtual_manifest_one_project() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["bar", "baz"]
            "#,
        )
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "pub fn bar() {}")
        .file("baz/Cargo.toml", &basic_manifest("baz", "0.1.0"))
        .file("baz/src/lib.rs", "pub fn baz() { break_the_build(); }")
        .build();

    p.cargo("doc -p bar")
        .with_stderr_does_not_contain("[DOCUMENTING] baz v0.1.0 [..]")
        .with_stderr(
            "\
[DOCUMENTING] bar v0.1.0 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cargo_test]
fn doc_virtual_manifest_glob() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["bar", "baz"]
            "#,
        )
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "pub fn bar() {  break_the_build(); }")
        .file("baz/Cargo.toml", &basic_manifest("baz", "0.1.0"))
        .file("baz/src/lib.rs", "pub fn baz() {}")
        .build();

    p.cargo("doc -p '*z'")
        .with_stderr_does_not_contain("[DOCUMENTING] bar v0.1.0 [..]")
        .with_stderr(
            "\
[DOCUMENTING] baz v0.1.0 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cargo_test]
fn doc_all_member_dependency_same_name() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["bar"]
            "#,
        )
        .file(
            "bar/Cargo.toml",
            r#"
                [project]
                name = "bar"
                version = "0.1.0"

                [dependencies]
                bar = "0.1.0"
            "#,
        )
        .file("bar/src/lib.rs", "pub fn bar() {}")
        .build();

    Package::new("bar", "0.1.0").publish();

    p.cargo("doc --workspace")
        .with_stderr_contains("[..] Updating `[..]` index")
        .with_stderr_contains("[..] Documenting bar v0.1.0 ([..])")
        .run();
}

#[cargo_test]
#[cfg(not(windows))] // `echo` may not be available
fn doc_workspace_open_help_message() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["foo", "bar"]
            "#,
        )
        .file("foo/Cargo.toml", &basic_manifest("foo", "0.1.0"))
        .file("foo/src/lib.rs", "")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "")
        .build();

    // The order in which bar is compiled or documented is not deterministic
    p.cargo("doc --workspace --open")
        .env("BROWSER", "echo")
        .with_stderr_contains("[..] Documenting bar v0.1.0 ([..])")
        .with_stderr_contains("[..] Documenting foo v0.1.0 ([..])")
        .with_stderr_contains("[..] Opening [..]/bar/index.html")
        .run();
}

#[cargo_test]
#[cfg(not(windows))] // `echo` may not be available
fn doc_extern_map_local() {
    if !is_nightly() {
        // -Zextern-html-root-url is unstable
        return;
    }

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .file(".cargo/config.toml", "doc.extern-map.std = 'local'")
        .build();

    p.cargo("doc -v --no-deps -Zrustdoc-map --open")
        .env("BROWSER", "echo")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "\
[DOCUMENTING] foo v0.1.0 [..]
[RUNNING] `rustdoc --crate-type lib --crate-name foo src/lib.rs [..]--crate-version 0.1.0`
[FINISHED] [..]
     Opening [CWD]/target/doc/foo/index.html
",
        )
        .run();
}

#[cargo_test]
#[cfg(not(windows))] // `echo` may not be available
fn doc_workspace_open_different_library_and_package_names() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["foo"]
            "#,
        )
        .file(
            "foo/Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                [lib]
                name = "foolib"
            "#,
        )
        .file("foo/src/lib.rs", "")
        .build();

    p.cargo("doc --open")
        .env("BROWSER", "echo")
        .with_stderr_contains("[..] Documenting foo v0.1.0 ([..])")
        .with_stderr_contains(&format!(
            "[..] [CWD]/target/{}/doc/foolib/index.html",
            rustc_host()
        ))
        .with_stdout_contains(&format!(
            "[CWD]/target/{}/doc/foolib/index.html",
            rustc_host()
        ))
        .run();

    p.change_file(
        ".cargo/config.toml",
        r#"
        [doc]
        browser = ["echo", "a"]
    "#,
    );

    // check that the cargo config overrides the browser env var
    p.cargo("doc --open")
        .env("BROWSER", "true")
        .with_stdout_contains(&format!(
            "a [CWD]/target/{}/doc/foolib/index.html",
            rustc_host()
        ))
        .run();
}

#[cargo_test]
#[cfg(not(windows))] // `echo` may not be available
fn doc_workspace_open_binary() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["foo"]
            "#,
        )
        .file(
            "foo/Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                [[bin]]
                name = "foobin"
                path = "src/main.rs"
            "#,
        )
        .file("foo/src/main.rs", "")
        .build();

    p.cargo("doc --open")
        .env("BROWSER", "echo")
        .with_stderr_contains("[..] Documenting foo v0.1.0 ([..])")
        .with_stderr_contains(&format!(
            "[..] Opening [CWD]/target/{}/doc/foobin/index.html",
            rustc_host()
        ))
        .run();
}

#[cargo_test]
#[cfg(not(windows))] // `echo` may not be available
fn doc_workspace_open_binary_and_library() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["foo"]
            "#,
        )
        .file(
            "foo/Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                [lib]
                name = "foolib"
                [[bin]]
                name = "foobin"
                path = "src/main.rs"
            "#,
        )
        .file("foo/src/lib.rs", "")
        .file("foo/src/main.rs", "")
        .build();

    p.cargo("doc --open")
        .env("BROWSER", "echo")
        .with_stderr_contains("[..] Documenting foo v0.1.0 ([..])")
        .with_stderr_contains(&format!(
            "[..] Opening [CWD]/target/{}/doc/foolib/index.html",
            rustc_host()
        ))
        .run();
}

#[cargo_test]
fn doc_edition() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []
                edition = "2018"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("doc -v")
        .with_stderr_contains("[RUNNING] `rustdoc [..]--edition=2018[..]")
        .run();

    p.cargo("test -v")
        .with_stderr_contains("[RUNNING] `rustdoc [..]--edition=2018[..]")
        .run();
}

#[cargo_test]
fn doc_target_edition() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [lib]
                edition = "2018"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("doc -v")
        .with_stderr_contains("[RUNNING] `rustdoc [..]--edition=2018[..]")
        .run();

    p.cargo("test -v")
        .with_stderr_contains("[RUNNING] `rustdoc [..]--edition=2018[..]")
        .run();
}

// Tests an issue where depending on different versions of the same crate depending on `cfg`s
// caused `cargo doc` to fail.
#[cargo_test]
fn issue_5345() {
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [target.'cfg(all(windows, target_arch = "x86"))'.dependencies]
                bar = "0.1"

                [target.'cfg(not(all(windows, target_arch = "x86")))'.dependencies]
                bar = "0.2"
            "#,
        )
        .file("src/lib.rs", "extern crate bar;")
        .build();
    Package::new("bar", "0.1.0").publish();
    Package::new("bar", "0.2.0").publish();

    foo.cargo("build").run();
    foo.cargo("doc").run();
}

#[cargo_test]
fn doc_private_items() {
    let foo = project()
        .file("src/lib.rs", "mod private { fn private_item() {} }")
        .build();
    foo.cargo("doc --document-private-items").run();

    assert!(foo
        .root()
        .join("target")
        .join(rustc_host())
        .join("doc")
        .is_dir());
    assert!(foo
        .root()
        .join("target")
        .join(rustc_host())
        .join("doc/foo/private/index.html")
        .is_file());
}

#[cargo_test]
fn doc_private_ws() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["a", "b"]
            "#,
        )
        .file("a/Cargo.toml", &basic_manifest("a", "0.0.1"))
        .file("a/src/lib.rs", "fn p() {}")
        .file("b/Cargo.toml", &basic_manifest("b", "0.0.1"))
        .file("b/src/lib.rs", "fn p2() {}")
        .file("b/src/bin/b-cli.rs", "fn main() {}")
        .build();
    p.cargo("doc --workspace --bins --lib --document-private-items -v")
        .with_stderr_contains(
            "[RUNNING] `rustdoc [..] a/src/lib.rs [..]--document-private-items[..]",
        )
        .with_stderr_contains(
            "[RUNNING] `rustdoc [..] b/src/lib.rs [..]--document-private-items[..]",
        )
        .with_stderr_contains(
            "[RUNNING] `rustdoc [..] b/src/bin/b-cli.rs [..]--document-private-items[..]",
        )
        .run();
}

const BAD_INTRA_LINK_LIB: &str = r#"
#![deny(broken_intra_doc_links)]

/// [bad_link]
pub fn foo() {}
"#;

#[cargo_test]
fn doc_cap_lints() {
    let a = git::new("a", |p| {
        p.file("Cargo.toml", &basic_lib_manifest("a"))
            .file("src/lib.rs", BAD_INTRA_LINK_LIB)
    });

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"
                    version = "0.0.1"
                    authors = []

                    [dependencies]
                    a = {{ git = '{}' }}
                "#,
                a.url()
            ),
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("doc")
        .with_stderr_unordered(
            "\
[UPDATING] git repository `[..]`
[DOCUMENTING] a v0.5.0 ([..])
[CHECKING] a v0.5.0 ([..])
[DOCUMENTING] foo v0.0.1 ([..])
[FINISHED] dev [..]
",
        )
        .run();

    p.root().join("target").rm_rf();

    p.cargo("doc -vv")
        .with_stderr_contains("[WARNING] [..]`bad_link`[..]")
        .run();
}

#[cargo_test]
fn doc_message_format() {
    let p = project().file("src/lib.rs", BAD_INTRA_LINK_LIB).build();

    p.cargo("doc --message-format=json")
        .with_status(101)
        .with_json_contains_unordered(
            r#"
            {
                "message": {
                    "children": "{...}",
                    "code": "{...}",
                    "level": "error",
                    "message": "{...}",
                    "rendered": "{...}",
                    "spans": "{...}"
                },
                "package_id": "foo [..]",
                "manifest_path": "[..]",
                "reason": "compiler-message",
                "target": "{...}"
            }
            "#,
        )
        .run();
}

#[cargo_test]
fn short_message_format() {
    let p = project().file("src/lib.rs", BAD_INTRA_LINK_LIB).build();
    p.cargo("doc --message-format=short")
        .with_status(101)
        .with_stderr_contains("src/lib.rs:4:6: error: [..]`bad_link`[..]")
        .run();
}

#[cargo_test]
fn doc_example() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            edition = "2018"

            [[example]]
            crate-type = ["lib"]
            name = "ex1"
            doc = true
            "#,
        )
        .file("src/lib.rs", "pub fn f() {}")
        .file(
            "examples/ex1.rs",
            r#"
            use foo::f;

            /// Example
            pub fn x() { f(); }
            "#,
        )
        .build();

    p.cargo("doc").run();
    assert!(p
        .build_dir()
        .join("doc")
        .join("ex1")
        .join("fn.x.html")
        .exists());
}

#[cargo_test]
fn bin_private_items() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []
            "#,
        )
        .file(
            "src/main.rs",
            "
            pub fn foo_pub() {}
            fn foo_priv() {}
            struct FooStruct;
            enum FooEnum {}
            trait FooTrait {}
            type FooType = u32;
            mod foo_mod {}

        ",
        )
        .build();

    p.cargo("doc")
        .with_stderr(
            "\
[DOCUMENTING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();

    assert!(p
        .root()
        .join("target")
        .join(rustc_host())
        .join("doc/foo/index.html")
        .is_file());
    assert!(p
        .root()
        .join("target")
        .join(rustc_host())
        .join("doc/foo/fn.foo_pub.html")
        .is_file());
    assert!(p
        .root()
        .join("target")
        .join(rustc_host())
        .join("doc/foo/fn.foo_priv.html")
        .is_file());
    assert!(p
        .root()
        .join("target")
        .join(rustc_host())
        .join("doc/foo/struct.FooStruct.html")
        .is_file());
    assert!(p
        .root()
        .join("target")
        .join(rustc_host())
        .join("doc/foo/enum.FooEnum.html")
        .is_file());
    assert!(p
        .root()
        .join("target")
        .join(rustc_host())
        .join("doc/foo/trait.FooTrait.html")
        .is_file());
    assert!(p
        .root()
        .join("target")
        .join(rustc_host())
        .join("doc/foo/type.FooType.html")
        .is_file());
    assert!(p
        .root()
        .join("target")
        .join(rustc_host())
        .join("doc/foo/foo_mod/index.html")
        .is_file());
}

#[cargo_test]
fn bin_private_items_deps() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies.bar]
                path = "bar"
            "#,
        )
        .file(
            "src/main.rs",
            "
            fn foo_priv() {}
            pub fn foo_pub() {}
        ",
        )
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file(
            "bar/src/lib.rs",
            "
            #[allow(dead_code)]
            fn bar_priv() {}
            pub fn bar_pub() {}
        ",
        )
        .build();

    p.cargo("doc")
        .with_stderr_unordered(
            "\
[DOCUMENTING] bar v0.0.1 ([..])
[CHECKING] bar v0.0.1 ([..])
[DOCUMENTING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();

    assert!(p
        .root()
        .join("target")
        .join(rustc_host())
        .join("doc/foo/index.html")
        .is_file());
    assert!(p
        .root()
        .join("target")
        .join(rustc_host())
        .join("doc/foo/fn.foo_pub.html")
        .is_file());
    assert!(p
        .root()
        .join("target")
        .join(rustc_host())
        .join("doc/foo/fn.foo_priv.html")
        .is_file());

    assert!(p
        .root()
        .join("target")
        .join(rustc_host())
        .join("doc/bar/index.html")
        .is_file());
    assert!(p
        .root()
        .join("target")
        .join(rustc_host())
        .join("doc/bar/fn.bar_pub.html")
        .is_file());
    assert!(!p
        .root()
        .join("target")
        .join(rustc_host())
        .join("doc/bar/fn.bar_priv.html")
        .exists());
}

#[cargo_test]
fn crate_versions() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "1.2.4"
                authors = []
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("doc -v")
        .with_stderr(
            "\
[DOCUMENTING] foo v1.2.4 [..]
[RUNNING] `rustdoc --crate-type lib --crate-name foo src/lib.rs [..]--crate-version 1.2.4`
[FINISHED] [..]
",
        )
        .run();

    let output_path = p
        .root()
        .join("target")
        .join(rustc_host())
        .join("doc/foo/index.html");
    let output_documentation = fs::read_to_string(&output_path).unwrap();

    assert!(output_documentation.contains("Version 1.2.4"));
}

#[cargo_test]
fn crate_versions_flag_is_overridden() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "1.2.4"
                authors = []
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    let output_documentation = || {
        let output_path = p
            .root()
            .join("target")
            .join(rustc_host())
            .join("doc/foo/index.html");
        fs::read_to_string(&output_path).unwrap()
    };
    let asserts = |html: String| {
        assert!(!html.contains("1.2.4"));
        assert!(html.contains("Version 2.0.3"));
    };

    p.cargo("doc")
        .env("RUSTDOCFLAGS", "--crate-version 2.0.3")
        .run();
    asserts(output_documentation());

    p.build_dir().rm_rf();

    p.cargo("rustdoc -- --crate-version 2.0.3").run();
    asserts(output_documentation());
}

#[cargo_test]
fn doc_test_in_workspace() {
    if !is_nightly() {
        // -Zdoctest-in-workspace is unstable
        return;
    }

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = [
                    "crate-a",
                    "crate-b",
                ]
            "#,
        )
        .file(
            "crate-a/Cargo.toml",
            r#"
                [project]
                name = "crate-a"
                version = "0.1.0"
            "#,
        )
        .file(
            "crate-a/src/lib.rs",
            "\
                //! ```
                //! assert_eq!(1, 1);
                //! ```
            ",
        )
        .file(
            "crate-b/Cargo.toml",
            r#"
                [project]
                name = "crate-b"
                version = "0.1.0"
            "#,
        )
        .file(
            "crate-b/src/lib.rs",
            "\
                //! ```
                //! assert_eq!(1, 1);
                //! ```
            ",
        )
        .build();
    p.cargo("test -Zdoctest-in-workspace --doc -vv")
        .masquerade_as_nightly_cargo()
        .with_stderr_contains("[DOCTEST] crate-a")
        .with_stdout_contains(
            "
running 1 test
test crate-a/src/lib.rs - (line 1) ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out[..]

",
        )
        .with_stderr_contains("[DOCTEST] crate-b")
        .with_stdout_contains(
            "
running 1 test
test crate-b/src/lib.rs - (line 1) ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out[..]

",
        )
        .run();
}

#[cargo_test]
fn doc_fingerprint_is_versioning_consistent() {
    // Random rustc verbose version
    let old_rustc_verbose_version = format!(
        "\
rustc 1.41.1 (f3e1a954d 2020-02-24)
binary: rustc
commit-hash: f3e1a954d2ead4e2fc197c7da7d71e6c61bad196
commit-date: 2020-02-24
host: {}
release: 1.41.1
LLVM version: 9.0
",
        rustc_host()
    );

    // Create the dummy project.
    let dummy_project = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "1.2.4"
            authors = []
        "#,
        )
        .file("src/lib.rs", "//! These are the docs!")
        .build();

    dummy_project.cargo("doc").run();

    let fingerprint: RustDocFingerprint = serde_json::from_str(&dummy_project.read_file(&format!(
        "target/host/{}/.rustdoc_fingerprint.json",
        rustc_host()
    )))
    .expect("JSON Serde fail");

    // Check that the fingerprint contains the actual rustc version
    // which has been used to compile the docs.
    let output = std::process::Command::new("rustc")
        .arg("-vV")
        .output()
        .expect("Failed to get actual rustc verbose version");
    assert_eq!(
        fingerprint.rustc_vv,
        (String::from_utf8_lossy(&output.stdout).as_ref())
    );

    // As the test shows above. Now we have generated the `doc/` folder and inside
    // the rustdoc fingerprint file is located with the correct rustc version.
    // So we will remove it and create a new fingerprint with an old rustc version
    // inside it. We will also place a bogus file inside of the `doc/` folder to ensure
    // it gets removed as we expect on the next doc compilation.
    dummy_project.change_file(
        &format!("target/host/{}/.rustdoc_fingerprint.json", rustc_host()),
        &old_rustc_verbose_version,
    );

    dummy_project.host_dir().join("doc").mkdir_p();
    fs::write(
        dummy_project.host_dir().join("doc/bogus_file"),
        String::from("This is a bogus file and should be removed!"),
    )
    .expect("Error writing test bogus file");

    // Now if we trigger another compilation, since the fingerprint contains an old version
    // of rustc, cargo should remove the entire `/doc` folder (including the fingerprint)
    // and generating another one with the actual version.
    // It should also remove the bogus file we created above.
    dummy_project.cargo("doc").run();

    assert!(!dummy_project.host_dir().join("doc/bogus_file").exists());

    let fingerprint: RustDocFingerprint = serde_json::from_str(&dummy_project.read_file(&format!(
        "target/host/{}/.rustdoc_fingerprint.json",
        rustc_host()
    )))
    .expect("JSON Serde fail");

    // Check that the fingerprint contains the actual rustc version
    // which has been used to compile the docs.
    assert_eq!(
        fingerprint.rustc_vv,
        (String::from_utf8_lossy(&output.stdout).as_ref())
    );
}

#[cargo_test]
fn doc_fingerprint_respects_target_paths() {
    // Random rustc verbose version
    let old_rustc_verbose_version = format!(
        "\
rustc 1.41.1 (f3e1a954d 2020-02-24)
binary: rustc
commit-hash: f3e1a954d2ead4e2fc197c7da7d71e6c61bad196
commit-date: 2020-02-24
host: {}
release: 1.41.1
LLVM version: 9.0
",
        rustc_host()
    );

    // Create the dummy project.
    let dummy_project = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "1.2.4"
            authors = []
        "#,
        )
        .file("src/lib.rs", "//! These are the docs!")
        .build();

    dummy_project.cargo("doc --target").arg(rustc_host()).run();

    let fingerprint: RustDocFingerprint = serde_json::from_str(&dummy_project.read_file(&format!(
        "target/host/{}/.rustdoc_fingerprint.json",
        rustc_host()
    )))
    .expect("JSON Serde fail");

    // Check that the fingerprint contains the actual rustc version
    // which has been used to compile the docs.
    let output = std::process::Command::new("rustc")
        .arg("-vV")
        .output()
        .expect("Failed to get actual rustc verbose version");
    assert_eq!(
        fingerprint.rustc_vv,
        (String::from_utf8_lossy(&output.stdout).as_ref())
    );

    // As the test shows above. Now we have generated the `doc/` folder and inside
    // the rustdoc fingerprint file is located with the correct rustc version.
    // So we will remove it and create a new fingerprint with an old rustc version
    // inside it. We will also place a bogus file inside of the `doc/` folder to ensure
    // it gets removed as we expect on the next doc compilation.
    dummy_project.change_file(
        &format!("target/host/{}/.rustdoc_fingerprint.json", rustc_host()),
        &old_rustc_verbose_version,
    );

    dummy_project.host_dir().join("doc").mkdir_p();
    fs::write(
        dummy_project.host_dir().join("doc/bogus_file"),
        String::from("This is a bogus file and should be removed!"),
    )
    .expect("Error writing test bogus file");

    // Now if we trigger another compilation, since the fingerprint contains an old version
    // of rustc, cargo should remove the entire `/doc` folder (including the fingerprint)
    // and generating another one with the actual version.
    // It should also remove the bogus file we created above.
    dummy_project.cargo("doc --target").arg(rustc_host()).run();

    assert!(!dummy_project.host_dir().join("doc/bogus_file").exists());

    let fingerprint: RustDocFingerprint = serde_json::from_str(&dummy_project.read_file(&format!(
        "target/host/{}/.rustdoc_fingerprint.json",
        rustc_host()
    )))
    .expect("JSON Serde fail");

    // Check that the fingerprint contains the actual rustc version
    // which has been used to compile the docs.
    assert_eq!(
        fingerprint.rustc_vv,
        (String::from_utf8_lossy(&output.stdout).as_ref())
    );
}

#[cargo_test]
fn doc_fingerprint_unusual_behavior() {
    // Checks for some unusual circumstances with clearing the doc directory.
    if !symlink_supported() {
        return;
    }
    let p = project().file("src/lib.rs", "").build();
    p.build_dir().mkdir_p();
    let real_doc = p.root().join("doc");
    real_doc.mkdir_p();
    let build_doc = p.build_dir().join("doc");
    p.symlink(&real_doc, &build_doc);
    fs::write(real_doc.join("somefile"), "test").unwrap();
    fs::write(real_doc.join(".hidden"), "test").unwrap();
    p.cargo("doc").run();
    // Make sure for the first run, it does not delete any files and does not
    // break the symlink.
    assert!(build_doc.join("somefile").exists());
    assert!(real_doc.join("somefile").exists());
    assert!(real_doc.join(".hidden").exists());
    assert!(real_doc.join("foo/index.html").exists());
    // Pretend that the last build was generated by an older version.
    p.change_file(
        &format!("target/host/{}/.rustdoc_fingerprint.json", rustc_host()),
        "{\"rustc_vv\": \"I am old\"}",
    );
    // Change file to trigger a new build.
    p.change_file("src/lib.rs", "// changed");
    p.cargo("doc")
        .with_stderr(
            "[DOCUMENTING] foo [..]\n\
             [FINISHED] [..]",
        )
        .run();
    // This will delete somefile, but not .hidden.
    assert!(!real_doc.join("somefile").exists());
    assert!(real_doc.join(".hidden").exists());
    assert!(real_doc.join("foo/index.html").exists());
    // And also check the -Z flag behavior.
    p.change_file(
        &format!("target/host/{}/.rustdoc_fingerprint.json", rustc_host()),
        "{\"rustc_vv\": \"I am old\"}",
    );
    // Change file to trigger a new build.
    p.change_file("src/lib.rs", "// changed2");
    fs::write(real_doc.join("somefile"), "test").unwrap();
    p.cargo("doc -Z skip-rustdoc-fingerprint")
        .masquerade_as_nightly_cargo()
        .with_stderr(
            "[DOCUMENTING] foo [..]\n\
             [FINISHED] [..]",
        )
        .run();
    // Should not have deleted anything.
    assert!(build_doc.join("somefile").exists());
    assert!(real_doc.join("somefile").exists());
}
