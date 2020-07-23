//! Tests for the `cargo doc` command.

use cargo_test_support::paths::CargoPathExt;
use cargo_test_support::registry::Package;
use cargo_test_support::{basic_lib_manifest, basic_manifest, git, project};
use cargo_test_support::{is_nightly, rustc_host};
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
    assert!(p.root().join("target/doc").is_dir());
    assert!(p.root().join("target/doc/foo/index.html").is_file());
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

    assert!(p.root().join("target/doc").is_dir());
    assert!(p.root().join("target/doc/foo/index.html").is_file());
    assert!(p.root().join("target/doc/bar/index.html").is_file());

    // Verify that it only emits rmeta for the dependency.
    assert_eq!(p.glob("target/debug/**/*.rlib").count(), 0);
    assert_eq!(p.glob("target/debug/deps/libbar-*.rmeta").count(), 1);

    p.cargo("doc")
        .env("CARGO_LOG", "cargo::ops::cargo_rustc::fingerprint")
        .with_stdout("")
        .run();

    assert!(p.root().join("target/doc").is_dir());
    assert!(p.root().join("target/doc/foo/index.html").is_file());
    assert!(p.root().join("target/doc/bar/index.html").is_file());
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

    assert!(p.root().join("target/doc").is_dir());
    assert!(p.root().join("target/doc/foo/index.html").is_file());
    assert!(!p.root().join("target/doc/bar/index.html").is_file());
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

    assert!(p.root().join("target/doc").is_dir());
    assert!(p.root().join("target/doc/bar/index.html").is_file());
    assert!(p.root().join("target/doc/foo/index.html").is_file());
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
        .with_stderr_contains("[..] library `foo_lib` is specified [..]")
        .with_stderr_contains("[..] `foo v0.1.0[..]` [..]")
        .with_stderr_contains("[..] `bar v0.1.0[..]` [..]")
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
        .with_stderr_contains("[DOCUMENTING] foo v0.1.0 ([CWD]/foo)")
        .with_stderr_contains("[DOCUMENTING] bar v0.1.0 ([CWD]/bar)")
        .with_stderr_contains("[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]")
        .run();
    assert!(p.root().join("target/doc").is_dir());
    let doc_file = p.root().join("target/doc/foo_lib/index.html");
    assert!(doc_file.is_file());
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
        "#,
        )
        .file("bar/src/foo-cli.rs", "")
        .build();

    p.cargo("doc --workspace")
        .with_status(101)
        .with_stderr_contains("[..] binary `foo_cli` is specified [..]")
        .with_stderr_contains("[..] `foo v0.1.0[..]` [..]")
        .with_stderr_contains("[..] `bar v0.1.0[..]` [..]")
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
    let doc_html = p.read_file("target/doc/foo/index.html");
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
    let doc_html = p.read_file("target/doc/foo/index.html");
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
    let doc_html = p.read_file("target/doc/foo/index.html");
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
    let doc_html = p.read_file("target/doc/foo/index.html");
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

    assert!(p.root().join("target/doc").is_dir());
    assert!(p.root().join("target/doc/bar/index.html").is_file());
    assert!(p.root().join("target/doc/baz/index.html").is_file());
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
    p.cargo("doc --features foo").run();
    assert!(p.root().join("target/doc").is_dir());
    assert!(p.root().join("target/doc/foo/fn.foo.html").is_file());
    assert!(p.root().join("target/doc/bar/fn.bar.html").is_file());
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
    assert!(p.root().join("target/doc/foo/index.html").is_file());

    fs::remove_dir_all(p.root().join("target/doc/foo")).unwrap();

    p.cargo("doc").run();
    assert!(p.root().join("target/doc/foo/index.html").is_file());
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
    assert!(p.root().join("target/doc/foo/index.html").is_file());
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
        .with_stderr_contains("[..] Opening [..]/foo/index.html")
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
        .with_stderr_contains("[..] [CWD]/target/doc/foolib/index.html")
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
        .with_stderr_contains("[..] Opening [CWD]/target/doc/foobin/index.html")
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
        .with_stderr_contains("[..] Opening [CWD]/target/doc/foolib/index.html")
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

    assert!(foo.root().join("target/doc").is_dir());
    assert!(foo
        .root()
        .join("target/doc/foo/private/index.html")
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
        .file("b/src/main.rs", "fn main() {}")
        .build();
    p.cargo("doc --workspace --bins --lib --document-private-items -v")
        .with_stderr_contains(
            "[RUNNING] `rustdoc [..] a/src/lib.rs [..]--document-private-items[..]",
        )
        .with_stderr_contains(
            "[RUNNING] `rustdoc [..] b/src/lib.rs [..]--document-private-items[..]",
        )
        .with_stderr_contains(
            "[RUNNING] `rustdoc [..] b/src/main.rs [..]--document-private-items[..]",
        )
        .run();
}

const BAD_INTRA_LINK_LIB: &str = r#"
#![deny(intra_doc_link_resolution_failure)]

/// [bad_link]
pub fn foo() {}
"#;

#[cargo_test]
fn doc_cap_lints() {
    if !is_nightly() {
        // This can be removed once intra_doc_link_resolution_failure fails on stable.
        return;
    }
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
    if !is_nightly() {
        // This can be removed once intra_doc_link_resolution_failure fails on stable.
        return;
    }
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
                    "message": "[..]",
                    "rendered": "[..]",
                    "spans": "{...}"
                },
                "package_id": "foo [..]",
                "reason": "compiler-message",
                "target": "{...}"
            }
            "#,
        )
        .run();
}

#[cargo_test]
fn short_message_format() {
    if !is_nightly() {
        // This can be removed once intra_doc_link_resolution_failure fails on stable.
        return;
    }
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

    assert!(p.root().join("target/doc/foo/index.html").is_file());
    assert!(p.root().join("target/doc/foo/fn.foo_pub.html").is_file());
    assert!(p.root().join("target/doc/foo/fn.foo_priv.html").is_file());
    assert!(p
        .root()
        .join("target/doc/foo/struct.FooStruct.html")
        .is_file());
    assert!(p.root().join("target/doc/foo/enum.FooEnum.html").is_file());
    assert!(p
        .root()
        .join("target/doc/foo/trait.FooTrait.html")
        .is_file());
    assert!(p.root().join("target/doc/foo/type.FooType.html").is_file());
    assert!(p.root().join("target/doc/foo/foo_mod/index.html").is_file());
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

    assert!(p.root().join("target/doc/foo/index.html").is_file());
    assert!(p.root().join("target/doc/foo/fn.foo_pub.html").is_file());
    assert!(p.root().join("target/doc/foo/fn.foo_priv.html").is_file());

    assert!(p.root().join("target/doc/bar/index.html").is_file());
    assert!(p.root().join("target/doc/bar/fn.bar_pub.html").is_file());
    assert!(!p.root().join("target/doc/bar/fn.bar_priv.html").exists());
}

#[cargo_test]
fn crate_versions() {
    // Testing flag that will reach stable on 1.44
    if !is_nightly() {
        return;
    }
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

    p.cargo("-Z crate-versions doc")
        .masquerade_as_nightly_cargo()
        .run();

    let output_path = p.root().join("target/doc/foo/index.html");
    let output_documentation = fs::read_to_string(&output_path).unwrap();

    assert!(output_documentation.contains("Version 1.2.4"));
}

#[cargo_test]
fn crate_versions_flag_is_overridden() {
    // Testing flag that will reach stable on 1.44
    if !is_nightly() {
        return;
    }
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
        let output_path = p.root().join("target/doc/foo/index.html");
        fs::read_to_string(&output_path).unwrap()
    };
    let asserts = |html: String| {
        assert!(!html.contains("1.2.4"));
        assert!(html.contains("Version 2.0.3"));
    };

    p.cargo("-Z crate-versions doc")
        .masquerade_as_nightly_cargo()
        .env("RUSTDOCFLAGS", "--crate-version 2.0.3")
        .run();
    asserts(output_documentation());

    p.build_dir().rm_rf();

    p.cargo("-Z crate-versions rustdoc -- --crate-version 2.0.3")
        .masquerade_as_nightly_cargo()
        .run();
    asserts(output_documentation());
}
