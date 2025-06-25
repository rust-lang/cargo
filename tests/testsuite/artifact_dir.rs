//! Tests for --artifact-dir flag.

use std::env;
use std::fs;
use std::path::Path;

use crate::prelude::*;
use cargo_test_support::sleep_ms;
use cargo_test_support::str;
use cargo_test_support::{basic_manifest, project};

#[cargo_test]
fn binary_with_debug() {
    let p = project()
        .file("src/main.rs", r#"fn main() { println!("Hello, World!") }"#)
        .build();

    p.cargo("build -Z unstable-options --artifact-dir out")
        .masquerade_as_nightly_cargo(&["artifact-dir"])
        .enable_mac_dsym()
        .run();
    check_dir_contents(
        &p.root().join("out"),
        &["foo"],
        &["foo", "foo.dSYM"],
        &["foo.exe", "foo.pdb"],
        &["foo.exe"],
    );
}

#[cargo_test]
fn static_library_with_debug() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
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

    p.cargo("build -Z unstable-options --artifact-dir out")
        .masquerade_as_nightly_cargo(&["artifact-dir"])
        .run();
    check_dir_contents(
        &p.root().join("out"),
        &["libfoo.a"],
        &["libfoo.a"],
        &["foo.lib"],
        &["libfoo.a"],
    );
}

#[cargo_test]
fn dynamic_library_with_debug() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
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

    p.cargo("build -Z unstable-options --artifact-dir out")
        .masquerade_as_nightly_cargo(&["artifact-dir"])
        .enable_mac_dsym()
        .run();
    check_dir_contents(
        &p.root().join("out"),
        &["libfoo.so"],
        &["libfoo.dylib", "libfoo.dylib.dSYM"],
        &["foo.dll", "foo.dll.exp", "foo.dll.lib", "foo.pdb"],
        &["foo.dll", "libfoo.dll.a"],
    );
}

#[cargo_test]
fn rlib_with_debug() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
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

    p.cargo("build -Z unstable-options --artifact-dir out")
        .masquerade_as_nightly_cargo(&["artifact-dir"])
        .run();
    check_dir_contents(
        &p.root().join("out"),
        &["libfoo.rlib"],
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
                [package]
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

    p.cargo("build -Z unstable-options --bin foo --artifact-dir out")
        .masquerade_as_nightly_cargo(&["artifact-dir"])
        .enable_mac_dsym()
        .run();
    check_dir_contents(
        &p.root().join("out"),
        &["foo"],
        &["foo", "foo.dSYM"],
        &["foo.exe", "foo.pdb"],
        &["foo.exe"],
    );
}

#[cargo_test]
fn artifact_dir_is_a_file() {
    let p = project()
        .file("src/main.rs", r#"fn main() { println!("Hello, World!") }"#)
        .file("out", "")
        .build();

    p.cargo("build -Z unstable-options --artifact-dir out")
        .masquerade_as_nightly_cargo(&["artifact-dir"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[ERROR] failed to create directory `[ROOT]/foo/out`

Caused by:
...

"#]])
        .run();
}

#[cargo_test]
fn replaces_artifacts() {
    let p = project()
        .file("src/main.rs", r#"fn main() { println!("foo") }"#)
        .build();

    p.cargo("build -Z unstable-options --artifact-dir out")
        .masquerade_as_nightly_cargo(&["artifact-dir"])
        .run();
    p.process(
        &p.root()
            .join(&format!("out/foo{}", env::consts::EXE_SUFFIX)),
    )
    .with_stdout_data(str![[r#"
foo

"#]])
    .run();

    sleep_ms(1000);
    p.change_file("src/main.rs", r#"fn main() { println!("bar") }"#);

    p.cargo("build -Z unstable-options --artifact-dir out")
        .masquerade_as_nightly_cargo(&["artifact-dir"])
        .run();
    p.process(
        &p.root()
            .join(&format!("out/foo{}", env::consts::EXE_SUFFIX)),
    )
    .with_stdout_data(str![[r#"
bar

"#]])
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

    p.cargo("build -Z unstable-options --artifact-dir out -vv")
        .masquerade_as_nightly_cargo(&["artifact-dir"])
        .enable_mac_dsym()
        .with_stdout_data(
            str![[r#"
[a 0.0.1] hello-build-a
[b 0.0.1] hello-build-b

"#]]
            .unordered(),
        )
        .run();
    check_dir_contents(
        &p.root().join("out"),
        &["a", "b"],
        &["a", "a.dSYM", "b", "b.dSYM"],
        &["a.exe", "a.pdb", "b.exe", "b.pdb"],
        &["a.exe", "b.exe"],
    );
}

#[cargo_test]
fn cargo_build_artifact_dir() {
    let p = project()
        .file("src/main.rs", r#"fn main() { println!("Hello, World!") }"#)
        .file(
            ".cargo/config.toml",
            r#"
            [build]
            artifact-dir = "out"
            "#,
        )
        .build();

    p.cargo("build -Z unstable-options")
        .masquerade_as_nightly_cargo(&["artifact-dir"])
        .enable_mac_dsym()
        .run();
    check_dir_contents(
        &p.root().join("out"),
        &["foo"],
        &["foo", "foo.dSYM"],
        &["foo.exe", "foo.pdb"],
        &["foo.exe"],
    );
}

#[cargo_test]
fn unsupported_short_artifact_dir_flag() {
    let p = project()
        .file("src/main.rs", r#"fn main() { println!("Hello, World!") }"#)
        .build();

    p.cargo("build -Z unstable-options -O")
        .masquerade_as_nightly_cargo(&["artifact-dir"])
        .with_stderr_data(str![[r#"
[ERROR] unexpected argument '-O' found

  tip: a similar argument exists: '--artifact-dir'

Usage: cargo[EXE] build [OPTIONS]

For more information, try '--help'.

"#]])
        .with_status(1)
        .run();
}

#[cargo_test]
fn deprecated_out_dir() {
    let p = project()
        .file("src/main.rs", r#"fn main() { println!("Hello, World!") }"#)
        .build();

    p.cargo("build -Z unstable-options --out-dir out")
        .masquerade_as_nightly_cargo(&["out-dir"])
        .enable_mac_dsym()
        .with_stderr_data(str![[r#"
[WARNING] the --out-dir flag has been changed to --artifact-dir
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    check_dir_contents(
        &p.root().join("out"),
        &["foo"],
        &["foo", "foo.dSYM"],
        &["foo.exe", "foo.pdb"],
        &["foo.exe"],
    );
}

#[cargo_test]
fn cargo_build_deprecated_out_dir() {
    let p = project()
        .file("src/main.rs", r#"fn main() { println!("Hello, World!") }"#)
        .file(
            ".cargo/config.toml",
            r#"
            [build]
            out-dir = "out"
            "#,
        )
        .build();

    p.cargo("build -Z unstable-options")
        .masquerade_as_nightly_cargo(&["out-dir"])
        .enable_mac_dsym()
        .with_stderr_data(str![[r#"
[WARNING] the out-dir config option has been changed to artifact-dir
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    check_dir_contents(
        &p.root().join("out"),
        &["foo"],
        &["foo", "foo.dSYM"],
        &["foo.exe", "foo.pdb"],
        &["foo.exe"],
    );
}

fn check_dir_contents(
    artifact_dir: &Path,
    expected_linux: &[&str],
    expected_mac: &[&str],
    expected_win_msvc: &[&str],
    expected_win_gnu: &[&str],
) {
    let expected = if cfg!(target_os = "windows") {
        if cfg!(target_env = "msvc") {
            expected_win_msvc
        } else {
            expected_win_gnu
        }
    } else if cfg!(target_os = "macos") {
        expected_mac
    } else {
        expected_linux
    };

    let actual = list_dir(artifact_dir);
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
