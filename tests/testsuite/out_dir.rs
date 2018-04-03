use cargotest::support::{execs, project};
use hamcrest::assert_that;
use std::path::Path;
use std::fs;

#[test]
fn binary_with_debug() {
    let p = project("foo")
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#,
        )
        .file("src/main.rs", r#"fn main() { println!("Hello, World!") }"#)
        .build();

    assert_that(p.cargo("build --out-dir out"), execs().with_status(0));
    check_dir_contents(
        &p.root().join("out"),
        &["foo"],
        &["foo"],
        &["foo.exe", "foo.pdb"],
    );
}

#[test]
fn static_library_with_debug() {
    let p = project("foo")
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

    assert_that(p.cargo("build --out-dir out"), execs().with_status(0));
    check_dir_contents(
        &p.root().join("out"),
        &["libfoo.a"],
        &["libfoo.a"],
        &["foo.lib"],
    );
}

#[test]
fn dynamic_library_with_debug() {
    let p = project("foo")
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

    assert_that(p.cargo("build --out-dir out"), execs().with_status(0));
    check_dir_contents(
        &p.root().join("out"),
        &["libfoo.so"],
        &["libfoo.so"],
        &["foo.dll", "foo.dll.lib"],
    );
}

#[test]
fn rlib_with_debug() {
    let p = project("foo")
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

    assert_that(p.cargo("build --out-dir out"), execs().with_status(0));
    check_dir_contents(
        &p.root().join("out"),
        &["libfoo.rlib"],
        &["libfoo.rlib"],
        &["libfoo.rlib"],
    );
}

#[test]
fn include_only_the_binary_from_the_current_package() {
    let p = project("foo")
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
        .file(
            "utils/Cargo.toml",
            r#"
            [project]
            name = "utils"
            version = "0.0.1"
            authors = []
        "#,
        )
        .file("utils/src/lib.rs", "")
        .build();

    assert_that(
        p.cargo("build --bin foo --out-dir out"),
        execs().with_status(0),
    );
    check_dir_contents(
        &p.root().join("out"),
        &["foo"],
        &["foo"],
        &["foo.exe", "foo.pdb"],
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
    expected.sort();
    assert_eq!(actual, expected);
}

fn list_dir(dir: &Path) -> Vec<String> {
    let mut res = Vec::new();
    for entry in fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        res.push(entry.file_name().into_string().unwrap());
    }
    res.sort();
    res
}
