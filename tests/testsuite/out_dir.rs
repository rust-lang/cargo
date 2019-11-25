//! Tests for --out-dir flag.

use std::env;
use std::fs::{self, File};
use std::path::Path;

use cargo_test_support::sleep_ms;
use cargo_test_support::{basic_manifest, project};

#[cargo_test]
fn binary_with_debug() {
    let p = project()
        .file("src/main.rs", r#"fn main() { println!("Hello, World!") }"#)
        .build();

    p.cargo("build -Z unstable-options --out-dir out")
        .masquerade_as_nightly_cargo()
        .run();
    check_dir_contents(
        &p.root().join("out"),
        &["foo"],
        &["foo", "foo.dSYM"],
        &["foo.exe", "foo.pdb"],
    );
}

#[cargo_test]
fn static_library_with_debug() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [lib]
            crate-type = ["staticlib"]
        "#,
        )
        .file(
            "src/lib.rs",
            r#"
            #[no_mangle]
            pub extern "C" fn foo() { println!("Hello, World!") }
        "#,
        )
        .build();

    p.cargo("build -Z unstable-options --out-dir out")
        .masquerade_as_nightly_cargo()
        .run();
    check_dir_contents(
        &p.root().join("out"),
        &["libfoo.a"],
        &["libfoo.a"],
        &["foo.lib"],
    );
}

#[cargo_test]
fn dynamic_library_with_debug() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [lib]
            crate-type = ["cdylib"]
        "#,
        )
        .file(
            "src/lib.rs",
            r#"
            #[no_mangle]
            pub extern "C" fn foo() { println!("Hello, World!") }
        "#,
        )
        .build();

    p.cargo("build -Z unstable-options --out-dir out")
        .masquerade_as_nightly_cargo()
        .run();
    check_dir_contents(
        &p.root().join("out"),
        &["libfoo.so"],
        &["libfoo.dylib"],
        &["foo.dll", "foo.dll.lib"],
    );
}

#[cargo_test]
fn rlib_with_debug() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [lib]
            crate-type = ["rlib"]
        "#,
        )
        .file(
            "src/lib.rs",
            r#"
            pub fn foo() { println!("Hello, World!") }
        "#,
        )
        .build();

    p.cargo("build -Z unstable-options --out-dir out")
        .masquerade_as_nightly_cargo()
        .run();
    check_dir_contents(
        &p.root().join("out"),
        &["libfoo.rlib"],
        &["libfoo.rlib"],
        &["libfoo.rlib"],
    );
}

#[cargo_test]
fn include_only_the_binary_from_the_current_package() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [workspace]

            [dependencies]
            utils = { path = "./utils" }
        "#,
        )
        .file("src/lib.rs", "extern crate utils;")
        .file(
            "src/main.rs",
            r#"
            extern crate foo;
            extern crate utils;
            fn main() {
                println!("Hello, World!")
            }
        "#,
        )
        .file("utils/Cargo.toml", &basic_manifest("utils", "0.0.1"))
        .file("utils/src/lib.rs", "")
        .build();

    p.cargo("build -Z unstable-options --bin foo --out-dir out")
        .masquerade_as_nightly_cargo()
        .run();
    check_dir_contents(
        &p.root().join("out"),
        &["foo"],
        &["foo", "foo.dSYM"],
        &["foo.exe", "foo.pdb"],
    );
}

#[cargo_test]
fn out_dir_is_a_file() {
    let p = project()
        .file("src/main.rs", r#"fn main() { println!("Hello, World!") }"#)
        .build();
    File::create(p.root().join("out")).unwrap();

    p.cargo("build -Z unstable-options --out-dir out")
        .masquerade_as_nightly_cargo()
        .with_status(101)
        .with_stderr_contains("[ERROR] failed to create directory [..]")
        .run();
}

#[cargo_test]
fn replaces_artifacts() {
    let p = project()
        .file("src/main.rs", r#"fn main() { println!("foo") }"#)
        .build();

    p.cargo("build -Z unstable-options --out-dir out")
        .masquerade_as_nightly_cargo()
        .run();
    p.process(
        &p.root()
            .join(&format!("out/foo{}", env::consts::EXE_SUFFIX)),
    )
    .with_stdout("foo")
    .run();

    sleep_ms(1000);
    p.change_file("src/main.rs", r#"fn main() { println!("bar") }"#);

    p.cargo("build -Z unstable-options --out-dir out")
        .masquerade_as_nightly_cargo()
        .run();
    p.process(
        &p.root()
            .join(&format!("out/foo{}", env::consts::EXE_SUFFIX)),
    )
    .with_stdout("bar")
    .run();
}

#[cargo_test]
fn avoid_build_scripts() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["a", "b"]
            "#,
        )
        .file("a/Cargo.toml", &basic_manifest("a", "0.0.1"))
        .file("a/src/main.rs", "fn main() {}")
        .file("a/build.rs", r#"fn main() { println!("hello-build-a"); }"#)
        .file("b/Cargo.toml", &basic_manifest("b", "0.0.1"))
        .file("b/src/main.rs", "fn main() {}")
        .file("b/build.rs", r#"fn main() { println!("hello-build-b"); }"#)
        .build();

    p.cargo("build -Z unstable-options --out-dir out -vv")
        .masquerade_as_nightly_cargo()
        .with_stdout_contains("[a 0.0.1] hello-build-a")
        .with_stdout_contains("[b 0.0.1] hello-build-b")
        .run();
    check_dir_contents(
        &p.root().join("out"),
        &["a", "b"],
        &["a", "a.dSYM", "b", "b.dSYM"],
        &["a.exe", "a.pdb", "b.exe", "b.pdb"],
    );
}

fn check_dir_contents(
    out_dir: &Path,
    expected_linux: &[&str],
    expected_mac: &[&str],
    expected_win: &[&str],
) {
    let expected = if cfg!(target_os = "windows") {
        expected_win
    } else if cfg!(target_os = "macos") {
        expected_mac
    } else {
        expected_linux
    };

    let actual = list_dir(out_dir);
    let mut expected = expected.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    expected.sort_unstable();
    assert_eq!(actual, expected);
}

fn list_dir(dir: &Path) -> Vec<String> {
    let mut res = Vec::new();
    for entry in fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        res.push(entry.file_name().into_string().unwrap());
    }
    res.sort_unstable();
    res
}
