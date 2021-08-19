//! Tests for the `cargo build` command.

use cargo::{
    core::compiler::CompileMode,
    core::{Shell, Workspace},
    ops::CompileOptions,
    Config,
};
use cargo_test_support::compare;
use cargo_test_support::paths::{root, CargoPathExt};
use cargo_test_support::registry::Package;
use cargo_test_support::tools;
use cargo_test_support::{
    basic_bin_manifest, basic_lib_manifest, basic_manifest, cargo_exe, git, is_nightly, main_file,
    paths, process, project, rustc_host, sleep_ms, symlink_supported, t, Execs, ProjectBuilder,
};
use cargo_util::paths::dylib_path_envvar;
use std::env;
use std::fs;
use std::io::Read;
use std::process::Stdio;

#[cargo_test]
fn cargo_compile_simple() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]))
        .build();

    p.cargo("build").run();
    assert!(p.bin("foo").is_file());

    p.process(&p.bin("foo")).with_stdout("i am foo\n").run();
}

#[cargo_test]
fn cargo_fail_with_no_stderr() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &String::from("refusal"))
        .build();
    p.cargo("build --message-format=json")
        .with_status(101)
        .with_stderr_does_not_contain("--- stderr")
        .run();
}

/// Checks that the `CARGO_INCREMENTAL` environment variable results in
/// `rustc` getting `-C incremental` passed to it.
#[cargo_test]
fn cargo_compile_incremental() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]))
        .build();

    p.cargo("build -v")
        .env("CARGO_INCREMENTAL", "1")
        .with_stderr_contains(
            "[RUNNING] `rustc [..] -C incremental=[..]/target/debug/incremental[..]`\n",
        )
        .run();

    p.cargo("test -v")
        .env("CARGO_INCREMENTAL", "1")
        .with_stderr_contains(
            "[RUNNING] `rustc [..] -C incremental=[..]/target/debug/incremental[..]`\n",
        )
        .run();
}

#[cargo_test]
fn incremental_profile() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                authors = []

                [profile.dev]
                incremental = false

                [profile.release]
                incremental = true
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("build -v")
        .env_remove("CARGO_INCREMENTAL")
        .with_stderr_does_not_contain("[..]C incremental=[..]")
        .run();

    p.cargo("build -v")
        .env("CARGO_INCREMENTAL", "1")
        .with_stderr_contains("[..]C incremental=[..]")
        .run();

    p.cargo("build --release -v")
        .env_remove("CARGO_INCREMENTAL")
        .with_stderr_contains("[..]C incremental=[..]")
        .run();

    p.cargo("build --release -v")
        .env("CARGO_INCREMENTAL", "0")
        .with_stderr_does_not_contain("[..]C incremental=[..]")
        .run();
}

#[cargo_test]
fn incremental_config() {
    let p = project()
        .file("src/main.rs", "fn main() {}")
        .file(
            ".cargo/config",
            r#"
                [build]
                incremental = false
            "#,
        )
        .build();

    p.cargo("build -v")
        .env_remove("CARGO_INCREMENTAL")
        .with_stderr_does_not_contain("[..]C incremental=[..]")
        .run();

    p.cargo("build -v")
        .env("CARGO_INCREMENTAL", "1")
        .with_stderr_contains("[..]C incremental=[..]")
        .run();
}

#[cargo_test]
fn cargo_compile_with_workspace_excluded() {
    let p = project().file("src/main.rs", "fn main() {}").build();

    p.cargo("build --workspace --exclude foo")
        .with_stderr_does_not_contain("[..]virtual[..]")
        .with_stderr_contains("[..]no packages to compile")
        .with_status(101)
        .run();
}

#[cargo_test]
fn cargo_compile_manifest_path() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]))
        .build();

    p.cargo("build --manifest-path foo/Cargo.toml")
        .cwd(p.root().parent().unwrap())
        .run();
    assert!(p.bin("foo").is_file());
}

#[cargo_test]
fn cargo_compile_with_invalid_manifest() {
    let p = project().file("Cargo.toml", "").build();

    p.cargo("build")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  virtual manifests must be configured with [workspace]
",
        )
        .run();
}

#[cargo_test]
fn cargo_compile_with_invalid_manifest2() {
    let p = project()
        .file(
            "Cargo.toml",
            "
                [project]
                foo = bar
            ",
        )
        .build();

    p.cargo("build")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  could not parse input as TOML

Caused by:
  invalid TOML value, did you mean to use a quoted string? at line 3 column 23
",
        )
        .run();
}

#[cargo_test]
fn cargo_compile_with_invalid_manifest3() {
    let p = project().file("src/Cargo.toml", "a = bar").build();

    p.cargo("build --manifest-path src/Cargo.toml")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  could not parse input as TOML

Caused by:
  invalid TOML value, did you mean to use a quoted string? at line 1 column 5
",
        )
        .run();
}

#[cargo_test]
fn cargo_compile_duplicate_build_targets() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [lib]
                name = "main"
                path = "src/main.rs"
                crate-type = ["dylib"]

                [dependencies]
            "#,
        )
        .file("src/main.rs", "#![allow(warnings)] fn main() {}")
        .build();

    p.cargo("build")
        .with_stderr(
            "\
warning: file found to be present in multiple build targets: [..]main.rs
[COMPILING] foo v0.0.1 ([..])
[FINISHED] [..]
",
        )
        .run();
}

#[cargo_test]
fn cargo_compile_with_invalid_version() {
    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "1.0"))
        .build();

    p.cargo("build")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  unexpected end of input while parsing minor version number for key `package.version`
",
        )
        .run();
}

#[cargo_test]
fn cargo_compile_with_empty_package_name() {
    let p = project()
        .file("Cargo.toml", &basic_manifest("", "0.0.0"))
        .build();

    p.cargo("build")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  package name cannot be an empty string
",
        )
        .run();
}

#[cargo_test]
fn cargo_compile_with_invalid_package_name() {
    let p = project()
        .file("Cargo.toml", &basic_manifest("foo::bar", "0.0.0"))
        .build();

    p.cargo("build")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  invalid character `:` in package name: `foo::bar`, [..]
",
        )
        .run();
}

#[cargo_test]
fn cargo_compile_with_invalid_bin_target_name() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                authors = []
                version = "0.0.0"

                [[bin]]
                name = ""
            "#,
        )
        .build();

    p.cargo("build")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  binary target names cannot be empty
",
        )
        .run();
}

#[cargo_test]
fn cargo_compile_with_forbidden_bin_target_name() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                authors = []
                version = "0.0.0"

                [[bin]]
                name = "build"
            "#,
        )
        .build();

    p.cargo("build")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  the binary target name `build` is forbidden, it conflicts with with cargo's build directory names
",
        )
        .run();
}

#[cargo_test]
fn cargo_compile_with_bin_and_crate_type() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                authors = []
                version = "0.0.0"

                [[bin]]
                name = "the_foo_bin"
                path = "src/foo.rs"
                crate-type = ["cdylib", "rlib"]
            "#,
        )
        .file("src/foo.rs", "fn main() {}")
        .build();

    p.cargo("build")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  the target `the_foo_bin` is a binary and can't have any crate-types set \
(currently \"cdylib, rlib\")",
        )
        .run();
}

#[cargo_test]
fn cargo_compile_api_exposes_artifact_paths() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                authors = []
                version = "0.0.0"

                [[bin]]
                name = "the_foo_bin"
                path = "src/bin.rs"

                [lib]
                name = "the_foo_lib"
                path = "src/foo.rs"
                crate-type = ["cdylib", "rlib"]
            "#,
        )
        .file("src/foo.rs", "pub fn bar() {}")
        .file("src/bin.rs", "pub fn main() {}")
        .build();

    let shell = Shell::from_write(Box::new(Vec::new()));
    let config = Config::new(shell, env::current_dir().unwrap(), paths::home());
    let ws = Workspace::new(&p.root().join("Cargo.toml"), &config).unwrap();
    let compile_options = CompileOptions::new(ws.config(), CompileMode::Build).unwrap();

    let result = cargo::ops::compile(&ws, &compile_options).unwrap();

    assert_eq!(1, result.binaries.len());
    assert!(result.binaries[0].path.exists());
    assert!(result.binaries[0]
        .path
        .to_str()
        .unwrap()
        .contains("the_foo_bin"));

    assert_eq!(1, result.cdylibs.len());
    // The exact library path varies by platform, but should certainly exist at least
    assert!(result.cdylibs[0].path.exists());
    assert!(result.cdylibs[0]
        .path
        .to_str()
        .unwrap()
        .contains("the_foo_lib"));
}

#[cargo_test]
fn cargo_compile_with_bin_and_proc() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                authors = []
                version = "0.0.0"

                [[bin]]
                name = "the_foo_bin"
                path = "src/foo.rs"
                proc-macro = true
            "#,
        )
        .file("src/foo.rs", "fn main() {}")
        .build();

    p.cargo("build")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  the target `the_foo_bin` is a binary and can't have `proc-macro` set `true`",
        )
        .run();
}

#[cargo_test]
fn cargo_compile_with_invalid_lib_target_name() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                authors = []
                version = "0.0.0"

                [lib]
                name = ""
            "#,
        )
        .build();

    p.cargo("build")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  library target names cannot be empty
",
        )
        .run();
}

#[cargo_test]
fn cargo_compile_with_invalid_non_numeric_dep_version() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"

                [dependencies]
                crossbeam = "y"
            "#,
        )
        .build();

    p.cargo("build")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] failed to parse manifest at `[CWD]/Cargo.toml`

Caused by:
  failed to parse the version requirement `y` for dependency `crossbeam`

Caused by:
  unexpected character 'y' while parsing major version number
",
        )
        .run();
}

#[cargo_test]
fn cargo_compile_without_manifest() {
    let p = project().no_manifest().build();

    p.cargo("build")
        .with_status(101)
        .with_stderr("[ERROR] could not find `Cargo.toml` in `[..]` or any parent directory")
        .run();
}

#[cargo_test]
#[cfg(target_os = "linux")]
fn cargo_compile_with_lowercase_cargo_toml() {
    let p = project()
        .no_manifest()
        .file("cargo.toml", &basic_manifest("foo", "0.1.0"))
        .file("src/lib.rs", &main_file(r#""i am foo""#, &[]))
        .build();

    p.cargo("build")
        .with_status(101)
        .with_stderr(
            "[ERROR] could not find `Cargo.toml` in `[..]` or any parent directory, \
        but found cargo.toml please try to rename it to Cargo.toml",
        )
        .run();
}

#[cargo_test]
fn cargo_compile_with_invalid_code() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", "invalid rust code!")
        .build();

    p.cargo("build")
        .with_status(101)
        .with_stderr_contains("[ERROR] could not compile `foo` due to previous error\n")
        .run();
    assert!(p.root().join("Cargo.lock").is_file());
}

#[cargo_test]
fn cargo_compile_with_invalid_code_in_deps() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies.bar]
                path = "../bar"
                [dependencies.baz]
                path = "../baz"
            "#,
        )
        .file("src/main.rs", "invalid rust code!")
        .build();
    let _bar = project()
        .at("bar")
        .file("Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("src/lib.rs", "invalid rust code!")
        .build();
    let _baz = project()
        .at("baz")
        .file("Cargo.toml", &basic_manifest("baz", "0.1.0"))
        .file("src/lib.rs", "invalid rust code!")
        .build();
    p.cargo("build")
        .with_status(101)
        .with_stderr_contains("[..]invalid rust code[..]")
        .with_stderr_contains("[ERROR] could not compile [..]")
        .run();
}

#[cargo_test]
fn cargo_compile_with_warnings_in_the_root_package() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", "fn main() {} fn dead() {}")
        .build();

    p.cargo("build")
        .with_stderr_contains("[..]function is never used: `dead`[..]")
        .run();
}

#[cargo_test]
fn cargo_compile_with_warnings_in_a_dep_package() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]

                name = "foo"
                version = "0.5.0"
                authors = ["wycats@example.com"]

                [dependencies.bar]
                path = "bar"

                [[bin]]

                name = "foo"
            "#,
        )
        .file("src/foo.rs", &main_file(r#""{}", bar::gimme()"#, &["bar"]))
        .file("bar/Cargo.toml", &basic_lib_manifest("bar"))
        .file(
            "bar/src/bar.rs",
            r#"
                pub fn gimme() -> &'static str {
                    "test passed"
                }

                fn dead() {}
            "#,
        )
        .build();

    p.cargo("build")
        .with_stderr_contains("[..]function is never used: `dead`[..]")
        .run();

    assert!(p.bin("foo").is_file());

    p.process(&p.bin("foo")).with_stdout("test passed\n").run();
}

#[cargo_test]
fn cargo_compile_with_nested_deps_inferred() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]

                name = "foo"
                version = "0.5.0"
                authors = ["wycats@example.com"]

                [dependencies.bar]
                path = 'bar'

                [[bin]]
                name = "foo"
            "#,
        )
        .file("src/foo.rs", &main_file(r#""{}", bar::gimme()"#, &["bar"]))
        .file(
            "bar/Cargo.toml",
            r#"
                [project]

                name = "bar"
                version = "0.5.0"
                authors = ["wycats@example.com"]

                [dependencies.baz]
                path = "../baz"
            "#,
        )
        .file(
            "bar/src/lib.rs",
            r#"
                extern crate baz;

                pub fn gimme() -> String {
                    baz::gimme()
                }
            "#,
        )
        .file("baz/Cargo.toml", &basic_manifest("baz", "0.5.0"))
        .file(
            "baz/src/lib.rs",
            r#"
                pub fn gimme() -> String {
                    "test passed".to_string()
                }
            "#,
        )
        .build();

    p.cargo("build").run();

    assert!(p.bin("foo").is_file());
    assert!(!p.bin("libbar.rlib").is_file());
    assert!(!p.bin("libbaz.rlib").is_file());

    p.process(&p.bin("foo")).with_stdout("test passed\n").run();
}

#[cargo_test]
fn cargo_compile_with_nested_deps_correct_bin() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]

                name = "foo"
                version = "0.5.0"
                authors = ["wycats@example.com"]

                [dependencies.bar]
                path = "bar"

                [[bin]]
                name = "foo"
            "#,
        )
        .file("src/main.rs", &main_file(r#""{}", bar::gimme()"#, &["bar"]))
        .file(
            "bar/Cargo.toml",
            r#"
                [project]

                name = "bar"
                version = "0.5.0"
                authors = ["wycats@example.com"]

                [dependencies.baz]
                path = "../baz"
            "#,
        )
        .file(
            "bar/src/lib.rs",
            r#"
                extern crate baz;

                pub fn gimme() -> String {
                    baz::gimme()
                }
            "#,
        )
        .file("baz/Cargo.toml", &basic_manifest("baz", "0.5.0"))
        .file(
            "baz/src/lib.rs",
            r#"
                pub fn gimme() -> String {
                    "test passed".to_string()
                }
            "#,
        )
        .build();

    p.cargo("build").run();

    assert!(p.bin("foo").is_file());
    assert!(!p.bin("libbar.rlib").is_file());
    assert!(!p.bin("libbaz.rlib").is_file());

    p.process(&p.bin("foo")).with_stdout("test passed\n").run();
}

#[cargo_test]
fn cargo_compile_with_nested_deps_shorthand() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]

                name = "foo"
                version = "0.5.0"
                authors = ["wycats@example.com"]

                [dependencies.bar]
                path = "bar"
            "#,
        )
        .file("src/main.rs", &main_file(r#""{}", bar::gimme()"#, &["bar"]))
        .file(
            "bar/Cargo.toml",
            r#"
                [project]

                name = "bar"
                version = "0.5.0"
                authors = ["wycats@example.com"]

                [dependencies.baz]
                path = "../baz"

                [lib]

                name = "bar"
            "#,
        )
        .file(
            "bar/src/bar.rs",
            r#"
                extern crate baz;

                pub fn gimme() -> String {
                    baz::gimme()
                }
            "#,
        )
        .file("baz/Cargo.toml", &basic_lib_manifest("baz"))
        .file(
            "baz/src/baz.rs",
            r#"
                pub fn gimme() -> String {
                    "test passed".to_string()
                }
            "#,
        )
        .build();

    p.cargo("build").run();

    assert!(p.bin("foo").is_file());
    assert!(!p.bin("libbar.rlib").is_file());
    assert!(!p.bin("libbaz.rlib").is_file());

    p.process(&p.bin("foo")).with_stdout("test passed\n").run();
}

#[cargo_test]
fn cargo_compile_with_nested_deps_longhand() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]

                name = "foo"
                version = "0.5.0"
                authors = ["wycats@example.com"]

                [dependencies.bar]
                path = "bar"
                version = "0.5.0"

                [[bin]]

                name = "foo"
            "#,
        )
        .file("src/foo.rs", &main_file(r#""{}", bar::gimme()"#, &["bar"]))
        .file(
            "bar/Cargo.toml",
            r#"
                [project]

                name = "bar"
                version = "0.5.0"
                authors = ["wycats@example.com"]

                [dependencies.baz]
                path = "../baz"
                version = "0.5.0"

                [lib]

                name = "bar"
            "#,
        )
        .file(
            "bar/src/bar.rs",
            r#"
                extern crate baz;

                pub fn gimme() -> String {
                    baz::gimme()
                }
            "#,
        )
        .file("baz/Cargo.toml", &basic_lib_manifest("baz"))
        .file(
            "baz/src/baz.rs",
            r#"
                pub fn gimme() -> String {
                    "test passed".to_string()
                }
            "#,
        )
        .build();

    p.cargo("build").run();

    assert!(p.bin("foo").is_file());
    assert!(!p.bin("libbar.rlib").is_file());
    assert!(!p.bin("libbaz.rlib").is_file());

    p.process(&p.bin("foo")).with_stdout("test passed\n").run();
}

// Check that Cargo gives a sensible error if a dependency can't be found
// because of a name mismatch.
#[cargo_test]
fn cargo_compile_with_dep_name_mismatch() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]

                name = "foo"
                version = "0.0.1"
                authors = ["wycats@example.com"]

                [[bin]]

                name = "foo"

                [dependencies.notquitebar]

                path = "bar"
            "#,
        )
        .file("src/bin/foo.rs", &main_file(r#""i am foo""#, &["bar"]))
        .file("bar/Cargo.toml", &basic_bin_manifest("bar"))
        .file("bar/src/bar.rs", &main_file(r#""i am bar""#, &[]))
        .build();

    p.cargo("build")
        .with_status(101)
        .with_stderr(
            "\
error: no matching package named `notquitebar` found
location searched: [CWD]/bar
required by package `foo v0.0.1 ([CWD])`
",
        )
        .run();
}

// Ensure that renamed deps have a valid name
#[cargo_test]
fn cargo_compile_with_invalid_dep_rename() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "buggin"
                version = "0.1.0"

                [dependencies]
                "haha this isn't a valid name 🐛" = { package = "libc", version = "0.1" }
            "#,
        )
        .file("src/main.rs", &main_file(r#""What's good?""#, &[]))
        .build();

    p.cargo("build")
        .with_status(101)
        .with_stderr(
            "\
error: failed to parse manifest at `[..]`

Caused by:
  invalid character ` ` in dependency name: `haha this isn't a valid name 🐛`, characters must be Unicode XID characters (numbers, `-`, `_`, or most letters)
",
        )
        .run();
}

#[cargo_test]
fn cargo_compile_with_filename() {
    let p = project()
        .file("src/lib.rs", "")
        .file(
            "src/bin/a.rs",
            r#"
                extern crate foo;
                fn main() { println!("hello a.rs"); }
            "#,
        )
        .file("examples/a.rs", r#"fn main() { println!("example"); }"#)
        .build();

    p.cargo("build --bin bin.rs")
        .with_status(101)
        .with_stderr("[ERROR] no bin target named `bin.rs`")
        .run();

    p.cargo("build --bin a.rs")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] no bin target named `a.rs`

<tab>Did you mean `a`?",
        )
        .run();

    p.cargo("build --example example.rs")
        .with_status(101)
        .with_stderr("[ERROR] no example target named `example.rs`")
        .run();

    p.cargo("build --example a.rs")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] no example target named `a.rs`

<tab>Did you mean `a`?",
        )
        .run();
}

#[cargo_test]
fn incompatible_dependencies() {
    Package::new("bad", "0.1.0").publish();
    Package::new("bad", "1.0.0").publish();
    Package::new("bad", "1.0.1").publish();
    Package::new("bad", "1.0.2").publish();
    Package::new("bar", "0.1.0").dep("bad", "0.1.0").publish();
    Package::new("baz", "0.1.1").dep("bad", "=1.0.0").publish();
    Package::new("baz", "0.1.0").dep("bad", "=1.0.0").publish();
    Package::new("qux", "0.1.2").dep("bad", ">=1.0.1").publish();
    Package::new("qux", "0.1.1").dep("bad", ">=1.0.1").publish();
    Package::new("qux", "0.1.0").dep("bad", ">=1.0.1").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"

                [dependencies]
                bar = "0.1.0"
                baz = "0.1.0"
                qux = "0.1.0"
            "#,
        )
        .file("src/main.rs", "fn main(){}")
        .build();

    p.cargo("build")
        .with_status(101)
        .with_stderr_contains(
            "\
error: failed to select a version for `bad`.
    ... required by package `qux v0.1.0`
    ... which is depended on by `foo v0.0.1 ([..])`
versions that meet the requirements `>=1.0.1` are: 1.0.2, 1.0.1

all possible versions conflict with previously selected packages.

  previously selected package `bad v1.0.0`
    ... which is depended on by `baz v0.1.0`
    ... which is depended on by `foo v0.0.1 ([..])`

failed to select a version for `bad` which could resolve this conflict",
        )
        .run();
}

#[cargo_test]
fn incompatible_dependencies_with_multi_semver() {
    Package::new("bad", "1.0.0").publish();
    Package::new("bad", "1.0.1").publish();
    Package::new("bad", "2.0.0").publish();
    Package::new("bad", "2.0.1").publish();
    Package::new("bar", "0.1.0").dep("bad", "=1.0.0").publish();
    Package::new("baz", "0.1.0").dep("bad", ">=2.0.1").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"

                [dependencies]
                bar = "0.1.0"
                baz = "0.1.0"
                bad = ">=1.0.1, <=2.0.0"
            "#,
        )
        .file("src/main.rs", "fn main(){}")
        .build();

    p.cargo("build")
        .with_status(101)
        .with_stderr_contains(
            "\
error: failed to select a version for `bad`.
    ... required by package `foo v0.0.1 ([..])`
versions that meet the requirements `>=1.0.1, <=2.0.0` are: 2.0.0, 1.0.1

all possible versions conflict with previously selected packages.

  previously selected package `bad v2.0.1`
    ... which is depended on by `baz v0.1.0`
    ... which is depended on by `foo v0.0.1 ([..])`

  previously selected package `bad v1.0.0`
    ... which is depended on by `bar v0.1.0`
    ... which is depended on by `foo v0.0.1 ([..])`

failed to select a version for `bad` which could resolve this conflict",
        )
        .run();
}

#[cargo_test]
fn compile_path_dep_then_change_version() {
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
        .file("src/lib.rs", "")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file("bar/src/lib.rs", "")
        .build();

    p.cargo("build").run();

    p.change_file("bar/Cargo.toml", &basic_manifest("bar", "0.0.2"));

    p.cargo("build").run();
}

#[cargo_test]
fn ignores_carriage_return_in_lockfile() {
    let p = project()
        .file("src/main.rs", "mod a; fn main() {}")
        .file("src/a.rs", "")
        .build();

    p.cargo("build").run();

    let lock = p.read_lockfile();
    p.change_file("Cargo.lock", &lock.replace("\n", "\r\n"));
    p.cargo("build").run();
}

#[cargo_test]
fn cargo_default_env_metadata_env_var() {
    // Ensure that path dep + dylib + env_var get metadata
    // (even though path_dep + dylib should not)
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
        .file("src/lib.rs", "// hi")
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.0.1"
                authors = []

                [lib]
                name = "bar"
                crate_type = ["dylib"]
            "#,
        )
        .file("bar/src/lib.rs", "// hello")
        .build();

    // No metadata on libbar since it's a dylib path dependency
    p.cargo("build -v")
        .with_stderr(&format!(
            "\
[COMPILING] bar v0.0.1 ([CWD]/bar)
[RUNNING] `rustc --crate-name bar bar/src/lib.rs [..]--crate-type dylib \
        --emit=[..]link \
        -C prefer-dynamic[..]-C debuginfo=2 \
        -C metadata=[..] \
        --out-dir [..] \
        -L dependency=[CWD]/target/debug/deps`
[COMPILING] foo v0.0.1 ([CWD])
[RUNNING] `rustc --crate-name foo src/lib.rs [..]--crate-type lib \
        --emit=[..]link[..]-C debuginfo=2 \
        -C metadata=[..] \
        -C extra-filename=[..] \
        --out-dir [..] \
        -L dependency=[CWD]/target/debug/deps \
        --extern bar=[CWD]/target/debug/deps/{prefix}bar{suffix}`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]",
            prefix = env::consts::DLL_PREFIX,
            suffix = env::consts::DLL_SUFFIX,
        ))
        .run();

    p.cargo("clean").run();

    // If you set the env-var, then we expect metadata on libbar
    p.cargo("build -v")
        .env("__CARGO_DEFAULT_LIB_METADATA", "stable")
        .with_stderr(&format!(
            "\
[COMPILING] bar v0.0.1 ([CWD]/bar)
[RUNNING] `rustc --crate-name bar bar/src/lib.rs [..]--crate-type dylib \
        --emit=[..]link \
        -C prefer-dynamic[..]-C debuginfo=2 \
        -C metadata=[..] \
        --out-dir [..] \
        -L dependency=[CWD]/target/debug/deps`
[COMPILING] foo v0.0.1 ([CWD])
[RUNNING] `rustc --crate-name foo src/lib.rs [..]--crate-type lib \
        --emit=[..]link[..]-C debuginfo=2 \
        -C metadata=[..] \
        -C extra-filename=[..] \
        --out-dir [..] \
        -L dependency=[CWD]/target/debug/deps \
        --extern bar=[CWD]/target/debug/deps/{prefix}bar-[..]{suffix}`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
            prefix = env::consts::DLL_PREFIX,
            suffix = env::consts::DLL_SUFFIX,
        ))
        .run();
}

#[cargo_test]
fn crate_env_vars() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.5.1-alpha.1"
            description = "This is foo"
            homepage = "https://example.com"
            repository = "https://example.com/repo.git"
            authors = ["wycats@example.com"]
            license = "MIT OR Apache-2.0"
            license_file = "license.txt"

            [[bin]]
            name = "foo-bar"
            path = "src/main.rs"
            "#,
        )
        .file(
            "src/main.rs",
            r#"
                extern crate foo;


                static VERSION_MAJOR: &'static str = env!("CARGO_PKG_VERSION_MAJOR");
                static VERSION_MINOR: &'static str = env!("CARGO_PKG_VERSION_MINOR");
                static VERSION_PATCH: &'static str = env!("CARGO_PKG_VERSION_PATCH");
                static VERSION_PRE: &'static str = env!("CARGO_PKG_VERSION_PRE");
                static VERSION: &'static str = env!("CARGO_PKG_VERSION");
                static CARGO_MANIFEST_DIR: &'static str = env!("CARGO_MANIFEST_DIR");
                static PKG_NAME: &'static str = env!("CARGO_PKG_NAME");
                static HOMEPAGE: &'static str = env!("CARGO_PKG_HOMEPAGE");
                static REPOSITORY: &'static str = env!("CARGO_PKG_REPOSITORY");
                static LICENSE: &'static str = env!("CARGO_PKG_LICENSE");
                static LICENSE_FILE: &'static str = env!("CARGO_PKG_LICENSE_FILE");
                static DESCRIPTION: &'static str = env!("CARGO_PKG_DESCRIPTION");
                static BIN_NAME: &'static str = env!("CARGO_BIN_NAME");
                static CRATE_NAME: &'static str = env!("CARGO_CRATE_NAME");


                fn main() {
                    let s = format!("{}-{}-{} @ {} in {}", VERSION_MAJOR,
                                    VERSION_MINOR, VERSION_PATCH, VERSION_PRE,
                                    CARGO_MANIFEST_DIR);
                     assert_eq!(s, foo::version());
                     println!("{}", s);
                     assert_eq!("foo", PKG_NAME);
                     assert_eq!("foo-bar", BIN_NAME);
                     assert_eq!("foo_bar", CRATE_NAME);
                     assert_eq!("https://example.com", HOMEPAGE);
                     assert_eq!("https://example.com/repo.git", REPOSITORY);
                     assert_eq!("MIT OR Apache-2.0", LICENSE);
                     assert_eq!("This is foo", DESCRIPTION);
                    let s = format!("{}.{}.{}-{}", VERSION_MAJOR,
                                    VERSION_MINOR, VERSION_PATCH, VERSION_PRE);
                    assert_eq!(s, VERSION);

                    // Verify CARGO_TARGET_TMPDIR isn't set for bins
                    assert!(option_env!("CARGO_TARGET_TMPDIR").is_none());
                }
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
                use std::env;
                use std::path::PathBuf;

                pub fn version() -> String {
                    format!("{}-{}-{} @ {} in {}",
                            env!("CARGO_PKG_VERSION_MAJOR"),
                            env!("CARGO_PKG_VERSION_MINOR"),
                            env!("CARGO_PKG_VERSION_PATCH"),
                            env!("CARGO_PKG_VERSION_PRE"),
                            env!("CARGO_MANIFEST_DIR"))
                }

                pub fn check_no_int_test_env() {
                    env::var("CARGO_TARGET_DIR").unwrap_err();
                }

                pub fn check_tmpdir(tmp: Option<&'static str>) {
                    let tmpdir: PathBuf = tmp.unwrap().into();

                    let exe: PathBuf = env::current_exe().unwrap().into();
                    let mut expected: PathBuf = exe.parent().unwrap().parent().unwrap().into();
                    expected.push("tmp");
                    assert_eq!(tmpdir, expected);

                    // Check that CARGO_TARGET_TMPDIR isn't set for lib code
                    assert!(option_env!("CARGO_TARGET_TMPDIR").is_none());
                    env::var("CARGO_TARGET_TMPDIR").unwrap_err();
                }

                #[test]
                fn env() {
                    // Check that CARGO_TARGET_TMPDIR isn't set for unit tests
                    assert!(option_env!("CARGO_TARGET_TMPDIR").is_none());
                    env::var("CARGO_TARGET_TMPDIR").unwrap_err();
                }
            "#,
        )
        .file(
            "tests/env.rs",
            r#"
                #[test]
                fn env() {
                    foo::check_tmpdir(option_env!("CARGO_TARGET_TMPDIR"));
                }
            "#,
        );

    let p = if is_nightly() {
        p.file(
            "benches/env.rs",
            r#"
                #![feature(test)]
                extern crate test;
                use test::Bencher;

                #[bench]
                fn env(_: &mut Bencher) {
                    foo::check_tmpdir(option_env!("CARGO_TARGET_TMPDIR"));
                }
            "#,
        )
        .build()
    } else {
        p.build()
    };

    println!("build");
    p.cargo("build -v").run();

    println!("bin");
    p.process(&p.bin("foo-bar"))
        .with_stdout("0-5-1 @ alpha.1 in [CWD]")
        .run();

    println!("test");
    p.cargo("test -v").run();

    if is_nightly() {
        println!("bench");
        p.cargo("bench -v").run();
    }
}

#[cargo_test]
fn crate_authors_env_vars() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.5.1-alpha.1"
                authors = ["wycats@example.com", "neikos@example.com"]
            "#,
        )
        .file(
            "src/main.rs",
            r#"
                extern crate foo;

                static AUTHORS: &'static str = env!("CARGO_PKG_AUTHORS");

                fn main() {
                    let s = "wycats@example.com:neikos@example.com";
                    assert_eq!(AUTHORS, foo::authors());
                    println!("{}", AUTHORS);
                    assert_eq!(s, AUTHORS);
                }
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
                pub fn authors() -> String {
                    format!("{}", env!("CARGO_PKG_AUTHORS"))
                }
            "#,
        )
        .build();

    println!("build");
    p.cargo("build -v").run();

    println!("bin");
    p.process(&p.bin("foo"))
        .with_stdout("wycats@example.com:neikos@example.com")
        .run();

    println!("test");
    p.cargo("test -v").run();
}

#[cargo_test]
fn vv_prints_rustc_env_vars() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
                authors = ["escape='\"@example.com"]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    let mut b = p.cargo("build -vv");

    if cfg!(windows) {
        b.with_stderr_contains(
            "[RUNNING] `[..]set CARGO_PKG_NAME=foo&& [..]rustc [..]`"
        ).with_stderr_contains(
            r#"[RUNNING] `[..]set CARGO_PKG_AUTHORS="escape='\"@example.com"&& [..]rustc [..]`"#
        )
    } else {
        b.with_stderr_contains("[RUNNING] `[..]CARGO_PKG_NAME=foo [..]rustc [..]`")
            .with_stderr_contains(
                r#"[RUNNING] `[..]CARGO_PKG_AUTHORS='escape='\''"@example.com' [..]rustc [..]`"#,
            )
    };

    b.run();
}

// The tester may already have LD_LIBRARY_PATH=::/foo/bar which leads to a false positive error
fn setenv_for_removing_empty_component(mut execs: Execs) -> Execs {
    let v = dylib_path_envvar();
    if let Ok(search_path) = env::var(v) {
        let new_search_path =
            env::join_paths(env::split_paths(&search_path).filter(|e| !e.as_os_str().is_empty()))
                .expect("join_paths");
        execs.env(v, new_search_path); // build_command() will override LD_LIBRARY_PATH accordingly
    }
    execs
}

// Regression test for #4277
#[cargo_test]
fn crate_library_path_env_var() {
    let p = project()
        .file(
            "src/main.rs",
            &format!(
                r#"
                    fn main() {{
                        let search_path = env!("{}");
                        let paths = std::env::split_paths(&search_path).collect::<Vec<_>>();
                        assert!(!paths.contains(&"".into()));
                    }}
                "#,
                dylib_path_envvar()
            ),
        )
        .build();

    setenv_for_removing_empty_component(p.cargo("run")).run();
}

// Regression test for #4277
#[cargo_test]
fn build_with_fake_libc_not_loading() {
    let p = project()
        .file("src/main.rs", "fn main() {}")
        .file("src/lib.rs", r#" "#)
        .file("libc.so.6", r#""#)
        .build();

    setenv_for_removing_empty_component(p.cargo("build")).run();
}

// this is testing that src/<pkg-name>.rs still works (for now)
#[cargo_test]
fn many_crate_types_old_style_lib_location() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]

                name = "foo"
                version = "0.5.0"
                authors = ["wycats@example.com"]

                [lib]

                name = "foo"
                crate_type = ["rlib", "dylib"]
            "#,
        )
        .file("src/foo.rs", "pub fn foo() {}")
        .build();
    p.cargo("build")
        .with_stderr_contains(
            "\
[WARNING] path `[..]src/foo.rs` was erroneously implicitly accepted for library `foo`,
please rename the file to `src/lib.rs` or set lib.path in Cargo.toml",
        )
        .run();

    assert!(p.root().join("target/debug/libfoo.rlib").is_file());
    let fname = format!("{}foo{}", env::consts::DLL_PREFIX, env::consts::DLL_SUFFIX);
    assert!(p.root().join("target/debug").join(&fname).is_file());
}

#[cargo_test]
fn many_crate_types_correct() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]

                name = "foo"
                version = "0.5.0"
                authors = ["wycats@example.com"]

                [lib]

                name = "foo"
                crate_type = ["rlib", "dylib"]
            "#,
        )
        .file("src/lib.rs", "pub fn foo() {}")
        .build();
    p.cargo("build").run();

    assert!(p.root().join("target/debug/libfoo.rlib").is_file());
    let fname = format!("{}foo{}", env::consts::DLL_PREFIX, env::consts::DLL_SUFFIX);
    assert!(p.root().join("target/debug").join(&fname).is_file());
}

#[cargo_test]
fn self_dependency() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]

                name = "test"
                version = "0.0.0"
                authors = []

                [dependencies.test]

                path = "."

                [lib]
                name = "test"
                path = "src/test.rs"
            "#,
        )
        .file("src/test.rs", "fn main() {}")
        .build();
    p.cargo("build")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] cyclic package dependency: package `test v0.0.0 ([CWD])` depends on itself. Cycle:
package `test v0.0.0 ([CWD])`
    ... which is depended on by `test v0.0.0 ([..])`",
        )
        .run();
}

#[cargo_test]
/// Make sure broken symlinks don't break the build
///
/// This test requires you to be able to make symlinks.
/// For windows, this may require you to enable developer mode.
fn ignore_broken_symlinks() {
    if !symlink_supported() {
        return;
    }

    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]))
        .symlink("Notafile", "bar")
        .build();

    p.cargo("build").run();
    assert!(p.bin("foo").is_file());

    p.process(&p.bin("foo")).with_stdout("i am foo\n").run();
}

#[cargo_test]
fn missing_lib_and_bin() {
    let p = project().build();
    p.cargo("build")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] failed to parse manifest at `[..]Cargo.toml`

Caused by:
  no targets specified in the manifest
  either src/lib.rs, src/main.rs, a [lib] section, or [[bin]] section must be present\n",
        )
        .run();
}

#[cargo_test]
fn lto_build() {
    // FIXME: currently this hits a linker bug on 32-bit MSVC
    if cfg!(all(target_env = "msvc", target_pointer_width = "32")) {
        return;
    }

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]

                name = "test"
                version = "0.0.0"
                authors = []

                [profile.release]
                lto = true
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();
    p.cargo("build -v --release")
        .with_stderr(
            "\
[COMPILING] test v0.0.0 ([CWD])
[RUNNING] `rustc --crate-name test src/main.rs [..]--crate-type bin \
        --emit=[..]link \
        -C opt-level=3 \
        -C lto \
        [..]
[FINISHED] release [optimized] target(s) in [..]
",
        )
        .run();
}

#[cargo_test]
fn verbose_build() {
    let p = project().file("src/lib.rs", "").build();
    p.cargo("build -v")
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([CWD])
[RUNNING] `rustc --crate-name foo src/lib.rs [..]--crate-type lib \
        --emit=[..]link[..]-C debuginfo=2 \
        -C metadata=[..] \
        --out-dir [..] \
        -L dependency=[CWD]/target/debug/deps`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cargo_test]
fn verbose_release_build() {
    let p = project().file("src/lib.rs", "").build();
    p.cargo("build -v --release")
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([CWD])
[RUNNING] `rustc --crate-name foo src/lib.rs [..]--crate-type lib \
        --emit=[..]link[..]\
        -C opt-level=3[..]\
        -C metadata=[..] \
        --out-dir [..] \
        -L dependency=[CWD]/target/release/deps`
[FINISHED] release [optimized] target(s) in [..]
",
        )
        .run();
}

#[cargo_test]
fn verbose_release_build_deps() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]

                name = "test"
                version = "0.0.0"
                authors = []

                [dependencies.foo]
                path = "foo"
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "foo/Cargo.toml",
            r#"
                [package]

                name = "foo"
                version = "0.0.0"
                authors = []

                [lib]
                name = "foo"
                crate_type = ["dylib", "rlib"]
            "#,
        )
        .file("foo/src/lib.rs", "")
        .build();
    p.cargo("build -v --release")
        .with_stderr(&format!(
            "\
[COMPILING] foo v0.0.0 ([CWD]/foo)
[RUNNING] `rustc --crate-name foo foo/src/lib.rs [..]\
        --crate-type dylib --crate-type rlib \
        --emit=[..]link \
        -C prefer-dynamic[..]\
        -C opt-level=3[..]\
        -C metadata=[..] \
        --out-dir [..] \
        -L dependency=[CWD]/target/release/deps`
[COMPILING] test v0.0.0 ([CWD])
[RUNNING] `rustc --crate-name test src/lib.rs [..]--crate-type lib \
        --emit=[..]link[..]\
        -C opt-level=3[..]\
        -C metadata=[..] \
        --out-dir [..] \
        -L dependency=[CWD]/target/release/deps \
        --extern foo=[CWD]/target/release/deps/{prefix}foo{suffix} \
        --extern foo=[CWD]/target/release/deps/libfoo.rlib`
[FINISHED] release [optimized] target(s) in [..]
",
            prefix = env::consts::DLL_PREFIX,
            suffix = env::consts::DLL_SUFFIX
        ))
        .run();
}

#[cargo_test]
fn explicit_examples() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "1.0.0"
                authors = []

                [lib]
                name = "foo"
                path = "src/lib.rs"

                [[example]]
                name = "hello"
                path = "examples/ex-hello.rs"

                [[example]]
                name = "goodbye"
                path = "examples/ex-goodbye.rs"
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
                pub fn get_hello() -> &'static str { "Hello" }
                pub fn get_goodbye() -> &'static str { "Goodbye" }
                pub fn get_world() -> &'static str { "World" }
            "#,
        )
        .file(
            "examples/ex-hello.rs",
            r#"
                extern crate foo;
                fn main() { println!("{}, {}!", foo::get_hello(), foo::get_world()); }
            "#,
        )
        .file(
            "examples/ex-goodbye.rs",
            r#"
                extern crate foo;
                fn main() { println!("{}, {}!", foo::get_goodbye(), foo::get_world()); }
            "#,
        )
        .build();

    p.cargo("build --examples").run();
    p.process(&p.bin("examples/hello"))
        .with_stdout("Hello, World!\n")
        .run();
    p.process(&p.bin("examples/goodbye"))
        .with_stdout("Goodbye, World!\n")
        .run();
}

#[cargo_test]
fn non_existing_example() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "1.0.0"
                authors = []

                [lib]
                name = "foo"
                path = "src/lib.rs"

                [[example]]
                name = "hello"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("test -v")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  can't find `hello` example, specify example.path",
        )
        .run();
}

#[cargo_test]
fn non_existing_binary() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/lib.rs", "")
        .file("src/bin/ehlo.rs", "")
        .build();

    p.cargo("build -v")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  can't find `foo` bin, specify bin.path",
        )
        .run();
}

#[cargo_test]
fn legacy_binary_paths_warnings() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "1.0.0"
                authors = []

                [[bin]]
                name = "bar"
            "#,
        )
        .file("src/lib.rs", "")
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("build -v")
        .with_stderr_contains(
            "\
[WARNING] path `[..]src/main.rs` was erroneously implicitly accepted for binary `bar`,
please set bin.path in Cargo.toml",
        )
        .run();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "1.0.0"
                authors = []

                [[bin]]
                name = "bar"
            "#,
        )
        .file("src/lib.rs", "")
        .file("src/bin/main.rs", "fn main() {}")
        .build();

    p.cargo("build -v")
        .with_stderr_contains(
            "\
[WARNING] path `[..]src/bin/main.rs` was erroneously implicitly accepted for binary `bar`,
please set bin.path in Cargo.toml",
        )
        .run();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "1.0.0"
                authors = []

                [[bin]]
                name = "bar"
            "#,
        )
        .file("src/bar.rs", "fn main() {}")
        .build();

    p.cargo("build -v")
        .with_stderr_contains(
            "\
[WARNING] path `[..]src/bar.rs` was erroneously implicitly accepted for binary `bar`,
please set bin.path in Cargo.toml",
        )
        .run();
}

#[cargo_test]
fn implicit_examples() {
    let p = project()
        .file(
            "src/lib.rs",
            r#"
                pub fn get_hello() -> &'static str { "Hello" }
                pub fn get_goodbye() -> &'static str { "Goodbye" }
                pub fn get_world() -> &'static str { "World" }
            "#,
        )
        .file(
            "examples/hello.rs",
            r#"
                extern crate foo;
                fn main() {
                    println!("{}, {}!", foo::get_hello(), foo::get_world());
                }
            "#,
        )
        .file(
            "examples/goodbye.rs",
            r#"
                extern crate foo;
                fn main() {
                    println!("{}, {}!", foo::get_goodbye(), foo::get_world());
                }
            "#,
        )
        .build();

    p.cargo("build --examples").run();
    p.process(&p.bin("examples/hello"))
        .with_stdout("Hello, World!\n")
        .run();
    p.process(&p.bin("examples/goodbye"))
        .with_stdout("Goodbye, World!\n")
        .run();
}

#[cargo_test]
fn standard_build_no_ndebug() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file(
            "src/foo.rs",
            r#"
                fn main() {
                    if cfg!(debug_assertions) {
                        println!("slow")
                    } else {
                        println!("fast")
                    }
                }
            "#,
        )
        .build();

    p.cargo("build").run();
    p.process(&p.bin("foo")).with_stdout("slow\n").run();
}

#[cargo_test]
fn release_build_ndebug() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file(
            "src/foo.rs",
            r#"
                fn main() {
                    if cfg!(debug_assertions) {
                        println!("slow")
                    } else {
                        println!("fast")
                    }
                }
            "#,
        )
        .build();

    p.cargo("build --release").run();
    p.process(&p.release_bin("foo")).with_stdout("fast\n").run();
}

#[cargo_test]
fn inferred_main_bin() {
    let p = project().file("src/main.rs", "fn main() {}").build();

    p.cargo("build").run();
    p.process(&p.bin("foo")).run();
}

#[cargo_test]
fn deletion_causes_failure() {
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
        .file("src/main.rs", "extern crate bar; fn main() {}")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file("bar/src/lib.rs", "")
        .build();

    p.cargo("build").run();
    p.change_file("Cargo.toml", &basic_manifest("foo", "0.0.1"));
    p.cargo("build")
        .with_status(101)
        .with_stderr_contains("[..]can't find crate for `bar`")
        .run();
}

#[cargo_test]
fn bad_cargo_toml_in_target_dir() {
    let p = project()
        .file("src/main.rs", "fn main() {}")
        .file("target/Cargo.toml", "bad-toml")
        .build();

    p.cargo("build").run();
    p.process(&p.bin("foo")).run();
}

#[cargo_test]
fn lib_with_standard_name() {
    let p = project()
        .file("Cargo.toml", &basic_manifest("syntax", "0.0.1"))
        .file("src/lib.rs", "pub fn foo() {}")
        .file(
            "src/main.rs",
            "extern crate syntax; fn main() { syntax::foo() }",
        )
        .build();

    p.cargo("build")
        .with_stderr(
            "\
[COMPILING] syntax v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cargo_test]
fn simple_staticlib() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                  [package]
                  name = "foo"
                  authors = []
                  version = "0.0.1"

                  [lib]
                  name = "foo"
                  crate-type = ["staticlib"]
            "#,
        )
        .file("src/lib.rs", "pub fn foo() {}")
        .build();

    // env var is a test for #1381
    p.cargo("build").env("CARGO_LOG", "nekoneko=trace").run();
}

#[cargo_test]
fn staticlib_rlib_and_bin() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                  [package]
                  name = "foo"
                  authors = []
                  version = "0.0.1"

                  [lib]
                  name = "foo"
                  crate-type = ["staticlib", "rlib"]
            "#,
        )
        .file("src/lib.rs", "pub fn foo() {}")
        .file("src/main.rs", "extern crate foo; fn main() { foo::foo(); }")
        .build();

    p.cargo("build -v").run();
}

#[cargo_test]
fn opt_out_of_bin() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                  bin = []

                  [package]
                  name = "foo"
                  authors = []
                  version = "0.0.1"
            "#,
        )
        .file("src/lib.rs", "")
        .file("src/main.rs", "bad syntax")
        .build();
    p.cargo("build").run();
}

#[cargo_test]
fn single_lib() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                  [package]
                  name = "foo"
                  authors = []
                  version = "0.0.1"

                  [lib]
                  name = "foo"
                  path = "src/bar.rs"
            "#,
        )
        .file("src/bar.rs", "")
        .build();
    p.cargo("build").run();
}

#[cargo_test]
fn freshness_ignores_excluded() {
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.0"
                authors = []
                build = "build.rs"
                exclude = ["src/b*.rs"]
            "#,
        )
        .file("build.rs", "fn main() {}")
        .file("src/lib.rs", "pub fn bar() -> i32 { 1 }")
        .build();
    foo.root().move_into_the_past();

    foo.cargo("build")
        .with_stderr(
            "\
[COMPILING] foo v0.0.0 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();

    // Smoke test to make sure it doesn't compile again
    println!("first pass");
    foo.cargo("build").with_stdout("").run();

    // Modify an ignored file and make sure we don't rebuild
    println!("second pass");
    foo.change_file("src/bar.rs", "");
    foo.cargo("build").with_stdout("").run();
}

#[cargo_test]
fn rebuild_preserves_out_dir() {
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.0"
                authors = []
                build = 'build.rs'
            "#,
        )
        .file(
            "build.rs",
            r#"
                use std::env;
                use std::fs::File;
                use std::path::Path;

                fn main() {
                    let path = Path::new(&env::var("OUT_DIR").unwrap()).join("foo");
                    if env::var_os("FIRST").is_some() {
                        File::create(&path).unwrap();
                    } else {
                        File::create(&path).unwrap();
                    }
                }
            "#,
        )
        .file("src/lib.rs", "pub fn bar() -> i32 { 1 }")
        .build();
    foo.root().move_into_the_past();

    foo.cargo("build")
        .env("FIRST", "1")
        .with_stderr(
            "\
[COMPILING] foo v0.0.0 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();

    foo.change_file("src/bar.rs", "");
    foo.cargo("build")
        .with_stderr(
            "\
[COMPILING] foo v0.0.0 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cargo_test]
fn dep_no_libs() {
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.0"
                authors = []

                [dependencies.bar]
                path = "bar"
            "#,
        )
        .file("src/lib.rs", "pub fn bar() -> i32 { 1 }")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.0.0"))
        .file("bar/src/main.rs", "")
        .build();
    foo.cargo("build").run();
}

#[cargo_test]
fn recompile_space_in_name() {
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.0"
                authors = []

                [lib]
                name = "foo"
                path = "src/my lib.rs"
            "#,
        )
        .file("src/my lib.rs", "")
        .build();
    foo.cargo("build").run();
    foo.root().move_into_the_past();
    foo.cargo("build").with_stdout("").run();
}

#[cfg(unix)]
#[cargo_test]
fn credentials_is_unreadable() {
    use cargo_test_support::paths::home;
    use std::os::unix::prelude::*;
    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "0.1.0"))
        .file("src/lib.rs", "")
        .build();

    let credentials = home().join(".cargo/credentials");
    t!(fs::create_dir_all(credentials.parent().unwrap()));
    t!(fs::write(
        &credentials,
        r#"
            [registry]
            token = "api-token"
        "#
    ));
    let stat = fs::metadata(credentials.as_path()).unwrap();
    let mut perms = stat.permissions();
    perms.set_mode(0o000);
    fs::set_permissions(credentials, perms).unwrap();

    p.cargo("build").run();
}

#[cfg(unix)]
#[cargo_test]
fn ignore_bad_directories() {
    use std::os::unix::prelude::*;
    let foo = project()
        .file("Cargo.toml", &basic_manifest("foo", "0.0.0"))
        .file("src/lib.rs", "")
        .build();
    let dir = foo.root().join("tmp");
    fs::create_dir(&dir).unwrap();
    let stat = fs::metadata(&dir).unwrap();
    let mut perms = stat.permissions();
    perms.set_mode(0o644);
    fs::set_permissions(&dir, perms.clone()).unwrap();
    foo.cargo("build").run();
    perms.set_mode(0o755);
    fs::set_permissions(&dir, perms).unwrap();
}

#[cargo_test]
fn bad_cargo_config() {
    let foo = project()
        .file("Cargo.toml", &basic_manifest("foo", "0.0.0"))
        .file("src/lib.rs", "")
        .file(".cargo/config", "this is not valid toml")
        .build();
    foo.cargo("build -v")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] could not load Cargo configuration

Caused by:
  could not parse TOML configuration in `[..]`

Caused by:
  could not parse input as TOML

Caused by:
  expected an equals, found an identifier at line 1 column 6
",
        )
        .run();
}

#[cargo_test]
fn cargo_platform_specific_dependency() {
    let host = rustc_host();
    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [project]
                    name = "foo"
                    version = "0.5.0"
                    authors = ["wycats@example.com"]
                    build = "build.rs"

                    [target.{host}.dependencies]
                    dep = {{ path = "dep" }}
                    [target.{host}.build-dependencies]
                    build = {{ path = "build" }}
                    [target.{host}.dev-dependencies]
                    dev = {{ path = "dev" }}
                "#,
                host = host
            ),
        )
        .file("src/main.rs", "extern crate dep; fn main() { dep::dep() }")
        .file(
            "tests/foo.rs",
            "extern crate dev; #[test] fn foo() { dev::dev() }",
        )
        .file(
            "build.rs",
            "extern crate build; fn main() { build::build(); }",
        )
        .file("dep/Cargo.toml", &basic_manifest("dep", "0.5.0"))
        .file("dep/src/lib.rs", "pub fn dep() {}")
        .file("build/Cargo.toml", &basic_manifest("build", "0.5.0"))
        .file("build/src/lib.rs", "pub fn build() {}")
        .file("dev/Cargo.toml", &basic_manifest("dev", "0.5.0"))
        .file("dev/src/lib.rs", "pub fn dev() {}")
        .build();

    p.cargo("build").run();

    assert!(p.bin("foo").is_file());
    p.cargo("test").run();
}

#[cargo_test]
fn bad_platform_specific_dependency() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]

                name = "foo"
                version = "0.5.0"
                authors = ["wycats@example.com"]

                [target.wrong-target.dependencies.bar]
                path = "bar"
            "#,
        )
        .file("src/main.rs", &main_file(r#""{}", bar::gimme()"#, &["bar"]))
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.5.0"))
        .file(
            "bar/src/lib.rs",
            r#"pub fn gimme() -> String { format!("") }"#,
        )
        .build();

    p.cargo("build")
        .with_status(101)
        .with_stderr_contains("[..]can't find crate for `bar`")
        .run();
}

#[cargo_test]
fn cargo_platform_specific_dependency_wrong_platform() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]

                name = "foo"
                version = "0.5.0"
                authors = ["wycats@example.com"]

                [target.non-existing-triplet.dependencies.bar]
                path = "bar"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.5.0"))
        .file(
            "bar/src/lib.rs",
            "invalid rust file, should not be compiled",
        )
        .build();

    p.cargo("build").run();

    assert!(p.bin("foo").is_file());
    p.process(&p.bin("foo")).run();

    let lockfile = p.read_lockfile();
    assert!(lockfile.contains("bar"));
}

#[cargo_test]
fn example_as_lib() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [[example]]
                name = "ex"
                crate-type = ["lib"]
            "#,
        )
        .file("src/lib.rs", "")
        .file("examples/ex.rs", "")
        .build();

    p.cargo("build --example=ex").run();
    assert!(p.example_lib("ex", "lib").is_file());
}

#[cargo_test]
fn example_as_rlib() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [[example]]
                name = "ex"
                crate-type = ["rlib"]
            "#,
        )
        .file("src/lib.rs", "")
        .file("examples/ex.rs", "")
        .build();

    p.cargo("build --example=ex").run();
    assert!(p.example_lib("ex", "rlib").is_file());
}

#[cargo_test]
fn example_as_dylib() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [[example]]
                name = "ex"
                crate-type = ["dylib"]
            "#,
        )
        .file("src/lib.rs", "")
        .file("examples/ex.rs", "")
        .build();

    p.cargo("build --example=ex").run();
    assert!(p.example_lib("ex", "dylib").is_file());
}

#[cargo_test]
fn example_as_proc_macro() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [[example]]
                name = "ex"
                crate-type = ["proc-macro"]
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "examples/ex.rs",
            r#"
            extern crate proc_macro;
            use proc_macro::TokenStream;

            #[proc_macro]
            pub fn eat(_item: TokenStream) -> TokenStream {
                "".parse().unwrap()
            }
            "#,
        )
        .build();

    p.cargo("build --example=ex").run();
    assert!(p.example_lib("ex", "proc-macro").is_file());
}

#[cargo_test]
fn example_bin_same_name() {
    let p = project()
        .file("src/main.rs", "fn main() {}")
        .file("examples/foo.rs", "fn main() {}")
        .build();

    p.cargo("build --examples").run();

    assert!(!p.bin("foo").is_file());
    // We expect a file of the form bin/foo-{metadata_hash}
    assert!(p.bin("examples/foo").is_file());

    p.cargo("build --examples").run();

    assert!(!p.bin("foo").is_file());
    // We expect a file of the form bin/foo-{metadata_hash}
    assert!(p.bin("examples/foo").is_file());
}

#[cargo_test]
fn compile_then_delete() {
    let p = project().file("src/main.rs", "fn main() {}").build();

    p.cargo("run -v").run();
    assert!(p.bin("foo").is_file());
    if cfg!(windows) {
        // On windows unlinking immediately after running often fails, so sleep
        sleep_ms(100);
    }
    fs::remove_file(&p.bin("foo")).unwrap();
    p.cargo("run -v").run();
}

#[cargo_test]
fn transitive_dependencies_not_available() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies.aaaaa]
                path = "a"
            "#,
        )
        .file(
            "src/main.rs",
            "extern crate bbbbb; extern crate aaaaa; fn main() {}",
        )
        .file(
            "a/Cargo.toml",
            r#"
                [package]
                name = "aaaaa"
                version = "0.0.1"
                authors = []

                [dependencies.bbbbb]
                path = "../b"
            "#,
        )
        .file("a/src/lib.rs", "extern crate bbbbb;")
        .file("b/Cargo.toml", &basic_manifest("bbbbb", "0.0.1"))
        .file("b/src/lib.rs", "")
        .build();

    p.cargo("build -v")
        .with_status(101)
        .with_stderr_contains("[..] can't find crate for `bbbbb`[..]")
        .run();
}

#[cargo_test]
fn cyclic_deps_rejected() {
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
        .file("src/lib.rs", "")
        .file(
            "a/Cargo.toml",
            r#"
                [package]
                name = "a"
                version = "0.0.1"
                authors = []

                [dependencies.foo]
                path = ".."
            "#,
        )
        .file("a/src/lib.rs", "")
        .build();

    p.cargo("build -v")
        .with_status(101)
        .with_stderr(
"[ERROR] cyclic package dependency: package `a v0.0.1 ([CWD]/a)` depends on itself. Cycle:
package `a v0.0.1 ([CWD]/a)`
    ... which is depended on by `foo v0.0.1 ([CWD])`
    ... which is depended on by `a v0.0.1 ([..])`",
        ).run();
}

#[cargo_test]
fn predictable_filenames() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [lib]
                name = "foo"
                crate-type = ["dylib", "rlib"]
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build -v").run();
    assert!(p.root().join("target/debug/libfoo.rlib").is_file());
    let dylib_name = format!("{}foo{}", env::consts::DLL_PREFIX, env::consts::DLL_SUFFIX);
    assert!(p.root().join("target/debug").join(dylib_name).is_file());
}

#[cargo_test]
fn dashes_to_underscores() {
    let p = project()
        .file("Cargo.toml", &basic_manifest("foo-bar", "0.0.1"))
        .file("src/lib.rs", "")
        .file("src/main.rs", "extern crate foo_bar; fn main() {}")
        .build();

    p.cargo("build -v").run();
    assert!(p.bin("foo-bar").is_file());
}

#[cargo_test]
fn dashes_in_crate_name_bad() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [lib]
                name = "foo-bar"
            "#,
        )
        .file("src/lib.rs", "")
        .file("src/main.rs", "extern crate foo_bar; fn main() {}")
        .build();

    p.cargo("build -v")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] failed to parse manifest at `[..]/foo/Cargo.toml`

Caused by:
  library target names cannot contain hyphens: foo-bar
",
        )
        .run();
}

#[cargo_test]
fn rustc_env_var() {
    let p = project().file("src/lib.rs", "").build();

    p.cargo("build -v")
        .env("RUSTC", "rustc-that-does-not-exist")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] could not execute process `rustc-that-does-not-exist -vV` ([..])

Caused by:
[..]
",
        )
        .run();
    assert!(!p.bin("a").is_file());
}

#[cargo_test]
fn filtering() {
    let p = project()
        .file("src/lib.rs", "")
        .file("src/bin/a.rs", "fn main() {}")
        .file("src/bin/b.rs", "fn main() {}")
        .file("examples/a.rs", "fn main() {}")
        .file("examples/b.rs", "fn main() {}")
        .build();

    p.cargo("build --lib").run();
    assert!(!p.bin("a").is_file());

    p.cargo("build --bin=a --example=a").run();
    assert!(p.bin("a").is_file());
    assert!(!p.bin("b").is_file());
    assert!(p.bin("examples/a").is_file());
    assert!(!p.bin("examples/b").is_file());
}

#[cargo_test]
fn filtering_implicit_bins() {
    let p = project()
        .file("src/lib.rs", "")
        .file("src/bin/a.rs", "fn main() {}")
        .file("src/bin/b.rs", "fn main() {}")
        .file("examples/a.rs", "fn main() {}")
        .file("examples/b.rs", "fn main() {}")
        .build();

    p.cargo("build --bins").run();
    assert!(p.bin("a").is_file());
    assert!(p.bin("b").is_file());
    assert!(!p.bin("examples/a").is_file());
    assert!(!p.bin("examples/b").is_file());
}

#[cargo_test]
fn filtering_implicit_examples() {
    let p = project()
        .file("src/lib.rs", "")
        .file("src/bin/a.rs", "fn main() {}")
        .file("src/bin/b.rs", "fn main() {}")
        .file("examples/a.rs", "fn main() {}")
        .file("examples/b.rs", "fn main() {}")
        .build();

    p.cargo("build --examples").run();
    assert!(!p.bin("a").is_file());
    assert!(!p.bin("b").is_file());
    assert!(p.bin("examples/a").is_file());
    assert!(p.bin("examples/b").is_file());
}

#[cargo_test]
fn ignore_dotfile() {
    let p = project()
        .file("src/bin/.a.rs", "")
        .file("src/bin/a.rs", "fn main() {}")
        .build();

    p.cargo("build").run();
}

#[cargo_test]
fn ignore_dotdirs() {
    let p = project()
        .file("src/bin/a.rs", "fn main() {}")
        .file(".git/Cargo.toml", "")
        .file(".pc/dummy-fix.patch/Cargo.toml", "")
        .build();

    p.cargo("build").run();
}

#[cargo_test]
fn dotdir_root() {
    let p = ProjectBuilder::new(root().join(".foo"))
        .file("src/bin/a.rs", "fn main() {}")
        .build();
    p.cargo("build").run();
}

#[cargo_test]
fn custom_target_dir_env() {
    let p = project().file("src/main.rs", "fn main() {}").build();

    let exe_name = format!("foo{}", env::consts::EXE_SUFFIX);

    p.cargo("build").env("CARGO_TARGET_DIR", "foo/target").run();
    assert!(p.root().join("foo/target/debug").join(&exe_name).is_file());
    assert!(!p.root().join("target/debug").join(&exe_name).is_file());

    p.cargo("build").run();
    assert!(p.root().join("foo/target/debug").join(&exe_name).is_file());
    assert!(p.root().join("target/debug").join(&exe_name).is_file());

    p.cargo("build")
        .env("CARGO_BUILD_TARGET_DIR", "foo2/target")
        .run();
    assert!(p.root().join("foo2/target/debug").join(&exe_name).is_file());

    p.change_file(
        ".cargo/config",
        r#"
            [build]
            target-dir = "foo/target"
        "#,
    );
    p.cargo("build").env("CARGO_TARGET_DIR", "bar/target").run();
    assert!(p.root().join("bar/target/debug").join(&exe_name).is_file());
    assert!(p.root().join("foo/target/debug").join(&exe_name).is_file());
    assert!(p.root().join("target/debug").join(&exe_name).is_file());
}

#[cargo_test]
fn custom_target_dir_line_parameter() {
    let p = project().file("src/main.rs", "fn main() {}").build();

    let exe_name = format!("foo{}", env::consts::EXE_SUFFIX);

    p.cargo("build --target-dir foo/target").run();
    assert!(p.root().join("foo/target/debug").join(&exe_name).is_file());
    assert!(!p.root().join("target/debug").join(&exe_name).is_file());

    p.cargo("build").run();
    assert!(p.root().join("foo/target/debug").join(&exe_name).is_file());
    assert!(p.root().join("target/debug").join(&exe_name).is_file());

    p.change_file(
        ".cargo/config",
        r#"
            [build]
            target-dir = "foo/target"
        "#,
    );
    p.cargo("build --target-dir bar/target").run();
    assert!(p.root().join("bar/target/debug").join(&exe_name).is_file());
    assert!(p.root().join("foo/target/debug").join(&exe_name).is_file());
    assert!(p.root().join("target/debug").join(&exe_name).is_file());

    p.cargo("build --target-dir foobar/target")
        .env("CARGO_TARGET_DIR", "bar/target")
        .run();
    assert!(p
        .root()
        .join("foobar/target/debug")
        .join(&exe_name)
        .is_file());
    assert!(p.root().join("bar/target/debug").join(&exe_name).is_file());
    assert!(p.root().join("foo/target/debug").join(&exe_name).is_file());
    assert!(p.root().join("target/debug").join(&exe_name).is_file());
}

#[cargo_test]
fn build_multiple_packages() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies.d1]
                    path = "d1"
                [dependencies.d2]
                    path = "d2"

                [[bin]]
                    name = "foo"
            "#,
        )
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]))
        .file("d1/Cargo.toml", &basic_bin_manifest("d1"))
        .file("d1/src/lib.rs", "")
        .file("d1/src/main.rs", "fn main() { println!(\"d1\"); }")
        .file(
            "d2/Cargo.toml",
            r#"
                [package]
                name = "d2"
                version = "0.0.1"
                authors = []

                [[bin]]
                    name = "d2"
                    doctest = false
            "#,
        )
        .file("d2/src/main.rs", "fn main() { println!(\"d2\"); }")
        .build();

    p.cargo("build -p d1 -p d2 -p foo").run();

    assert!(p.bin("foo").is_file());
    p.process(&p.bin("foo")).with_stdout("i am foo\n").run();

    let d1_path = &p
        .build_dir()
        .join("debug")
        .join(format!("d1{}", env::consts::EXE_SUFFIX));
    let d2_path = &p
        .build_dir()
        .join("debug")
        .join(format!("d2{}", env::consts::EXE_SUFFIX));

    assert!(d1_path.is_file());
    p.process(d1_path).with_stdout("d1").run();

    assert!(d2_path.is_file());
    p.process(d2_path).with_stdout("d2").run();
}

#[cargo_test]
fn invalid_spec() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies.d1]
                    path = "d1"

                [[bin]]
                    name = "foo"
            "#,
        )
        .file("src/bin/foo.rs", &main_file(r#""i am foo""#, &[]))
        .file("d1/Cargo.toml", &basic_bin_manifest("d1"))
        .file("d1/src/lib.rs", "")
        .file("d1/src/main.rs", "fn main() { println!(\"d1\"); }")
        .build();

    p.cargo("build -p notAValidDep")
        .with_status(101)
        .with_stderr("[ERROR] package ID specification `notAValidDep` did not match any packages")
        .run();

    p.cargo("build -p d1 -p notAValidDep")
        .with_status(101)
        .with_stderr("[ERROR] package ID specification `notAValidDep` did not match any packages")
        .run();
}

#[cargo_test]
fn manifest_with_bom_is_ok() {
    let p = project()
        .file(
            "Cargo.toml",
            "\u{FEFF}
            [package]
            name = \"foo\"
            version = \"0.0.1\"
            authors = []
        ",
        )
        .file("src/lib.rs", "")
        .build();
    p.cargo("build -v").run();
}

#[cargo_test]
fn panic_abort_compiles_with_panic_abort() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [profile.dev]
                panic = 'abort'
            "#,
        )
        .file("src/lib.rs", "")
        .build();
    p.cargo("build -v")
        .with_stderr_contains("[..] -C panic=abort [..]")
        .run();
}

#[cargo_test]
fn compiler_json_error_format() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]

                name = "foo"
                version = "0.5.0"
                authors = ["wycats@example.com"]

                [dependencies.bar]
                path = "bar"
            "#,
        )
        .file(
            "build.rs",
            "fn main() { println!(\"cargo:rustc-cfg=xyz\") }",
        )
        .file("src/main.rs", "fn main() { let unused = 92; }")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.5.0"))
        .file("bar/src/lib.rs", r#"fn dead() {}"#)
        .build();

    let output = |fresh| {
        r#"
            {
                "reason":"compiler-artifact",
                "package_id":"foo 0.5.0 ([..])",
                "manifest_path": "[..]",
                "target":{
                    "kind":["custom-build"],
                    "crate_types":["bin"],
                    "doc": false,
                    "doctest": false,
                    "edition": "2015",
                    "name":"build-script-build",
                    "src_path":"[..]build.rs",
                    "test": false
                },
                "profile": {
                    "debug_assertions": true,
                    "debuginfo": 2,
                    "opt_level": "0",
                    "overflow_checks": true,
                    "test": false
                },
                "executable": null,
                "features": [],
                "filenames": "{...}",
                "fresh": $FRESH
            }

            {
                "reason":"compiler-message",
                "package_id":"bar 0.5.0 ([..])",
                "manifest_path": "[..]",
                "target":{
                    "kind":["lib"],
                    "crate_types":["lib"],
                    "doc": true,
                    "doctest": true,
                    "edition": "2015",
                    "name":"bar",
                    "src_path":"[..]lib.rs",
                    "test": true
                },
                "message":"{...}"
            }

            {
                "reason":"compiler-artifact",
                "profile": {
                    "debug_assertions": true,
                    "debuginfo": 2,
                    "opt_level": "0",
                    "overflow_checks": true,
                    "test": false
                },
                "executable": null,
                "features": [],
                "package_id":"bar 0.5.0 ([..])",
                "manifest_path": "[..]",
                "target":{
                    "kind":["lib"],
                    "crate_types":["lib"],
                    "doc": true,
                    "doctest": true,
                    "edition": "2015",
                    "name":"bar",
                    "src_path":"[..]lib.rs",
                    "test": true
                },
                "filenames":[
                    "[..].rlib",
                    "[..].rmeta"
                ],
                "fresh": $FRESH
            }

            {
                "reason":"build-script-executed",
                "package_id":"foo 0.5.0 ([..])",
                "linked_libs":[],
                "linked_paths":[],
                "env":[],
                "cfgs":["xyz"],
                "out_dir": "[..]target/debug/build/foo-[..]/out"
            }

            {
                "reason":"compiler-message",
                "package_id":"foo 0.5.0 ([..])",
                "manifest_path": "[..]",
                "target":{
                    "kind":["bin"],
                    "crate_types":["bin"],
                    "doc": true,
                    "doctest": false,
                    "edition": "2015",
                    "name":"foo",
                    "src_path":"[..]main.rs",
                    "test": true
                },
                "message":"{...}"
            }

            {
                "reason":"compiler-artifact",
                "package_id":"foo 0.5.0 ([..])",
                "manifest_path": "[..]",
                "target":{
                    "kind":["bin"],
                    "crate_types":["bin"],
                    "doc": true,
                    "doctest": false,
                    "edition": "2015",
                    "name":"foo",
                    "src_path":"[..]main.rs",
                    "test": true
                },
                "profile": {
                    "debug_assertions": true,
                    "debuginfo": 2,
                    "opt_level": "0",
                    "overflow_checks": true,
                    "test": false
                },
                "executable": "[..]/foo/target/debug/foo[EXE]",
                "features": [],
                "filenames": "{...}",
                "fresh": $FRESH
            }

            {"reason": "build-finished", "success": true}
        "#
        .replace("$FRESH", fresh)
    };

    // Use `jobs=1` to ensure that the order of messages is consistent.
    p.cargo("build -v --message-format=json --jobs=1")
        .with_json_contains_unordered(&output("false"))
        .run();

    // With fresh build, we should repeat the artifacts,
    // and replay the cached compiler warnings.
    p.cargo("build -v --message-format=json --jobs=1")
        .with_json_contains_unordered(&output("true"))
        .run();
}

#[cargo_test]
fn wrong_message_format_option() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("build --message-format XML")
        .with_status(101)
        .with_stderr_contains(
            "\
error: invalid message format specifier: `xml`
",
        )
        .run();
}

#[cargo_test]
fn message_format_json_forward_stderr() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", "fn main() { let unused = 0; }")
        .build();

    p.cargo("rustc --release --bin foo --message-format JSON")
        .with_json_contains_unordered(
            r#"
                {
                    "reason":"compiler-message",
                    "package_id":"foo 0.5.0 ([..])",
                    "manifest_path": "[..]",
                    "target":{
                        "kind":["bin"],
                        "crate_types":["bin"],
                        "doc": true,
                        "doctest": false,
                        "edition": "2015",
                        "name":"foo",
                        "src_path":"[..]",
                        "test": true
                    },
                    "message":"{...}"
                }

                {
                    "reason":"compiler-artifact",
                    "package_id":"foo 0.5.0 ([..])",
                    "manifest_path": "[..]",
                    "target":{
                        "kind":["bin"],
                        "crate_types":["bin"],
                        "doc": true,
                        "doctest": false,
                        "edition": "2015",
                        "name":"foo",
                        "src_path":"[..]",
                        "test": true
                    },
                    "profile":{
                        "debug_assertions":false,
                        "debuginfo":null,
                        "opt_level":"3",
                        "overflow_checks": false,
                        "test":false
                    },
                    "executable": "{...}",
                    "features":[],
                    "filenames": "{...}",
                    "fresh": false
                }

                {"reason": "build-finished", "success": true}
            "#,
        )
        .run();
}

#[cargo_test]
fn no_warn_about_package_metadata() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [package.metadata]
                foo = "bar"
                a = true
                b = 3

                [package.metadata.another]
                bar = 3
            "#,
        )
        .file("src/lib.rs", "")
        .build();
    p.cargo("build")
        .with_stderr(
            "[..] foo v0.0.1 ([..])\n\
             [FINISHED] dev [unoptimized + debuginfo] target(s) in [..]\n",
        )
        .run();
}

#[cargo_test]
fn no_warn_about_workspace_metadata() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["foo"]

            [workspace.metadata]
            something = "something_else"
            x = 1
            y = 2

            [workspace.metadata.another]
            bar = 12
            "#,
        )
        .file(
            "foo/Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            "#,
        )
        .file("foo/src/lib.rs", "")
        .build();

    p.cargo("build")
        .with_stderr(
            "[..] foo v0.0.1 ([..])\n\
             [FINISHED] dev [unoptimized + debuginfo] target(s) in [..]\n",
        )
        .run();
}

#[cargo_test]
fn cargo_build_empty_target() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("build --target")
        .arg("")
        .with_status(101)
        .with_stderr_contains("[..] target was empty")
        .run();
}

#[cargo_test]
fn build_all_workspace() {
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

    p.cargo("build --workspace")
        .with_stderr(
            "\
[COMPILING] bar v0.1.0 ([..])
[COMPILING] foo v0.1.0 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cargo_test]
fn build_all_exclude() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.1.0"

                [workspace]
                members = ["bar", "baz"]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "pub fn bar() {}")
        .file("baz/Cargo.toml", &basic_manifest("baz", "0.1.0"))
        .file("baz/src/lib.rs", "pub fn baz() { break_the_build(); }")
        .build();

    p.cargo("build --workspace --exclude baz")
        .with_stderr_does_not_contain("[COMPILING] baz v0.1.0 [..]")
        .with_stderr_unordered(
            "\
[COMPILING] foo v0.1.0 ([..])
[COMPILING] bar v0.1.0 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cargo_test]
fn build_all_exclude_not_found() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.1.0"

                [workspace]
                members = ["bar"]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "pub fn bar() {}")
        .build();

    p.cargo("build --workspace --exclude baz")
        .with_stderr_does_not_contain("[COMPILING] baz v0.1.0 [..]")
        .with_stderr_unordered(
            "\
[WARNING] excluded package(s) `baz` not found in workspace [..]
[COMPILING] foo v0.1.0 ([..])
[COMPILING] bar v0.1.0 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cargo_test]
fn build_all_exclude_glob() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.1.0"

                [workspace]
                members = ["bar", "baz"]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "pub fn bar() {}")
        .file("baz/Cargo.toml", &basic_manifest("baz", "0.1.0"))
        .file("baz/src/lib.rs", "pub fn baz() { break_the_build(); }")
        .build();

    p.cargo("build --workspace --exclude '*z'")
        .with_stderr_does_not_contain("[COMPILING] baz v0.1.0 [..]")
        .with_stderr_unordered(
            "\
[COMPILING] foo v0.1.0 ([..])
[COMPILING] bar v0.1.0 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cargo_test]
fn build_all_exclude_glob_not_found() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.1.0"

                [workspace]
                members = ["bar"]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "pub fn bar() {}")
        .build();

    p.cargo("build --workspace --exclude '*z'")
        .with_stderr_does_not_contain("[COMPILING] baz v0.1.0 [..]")
        .with_stderr(
            "\
[WARNING] excluded package pattern(s) `*z` not found in workspace [..]
[COMPILING] [..] v0.1.0 ([..])
[COMPILING] [..] v0.1.0 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cargo_test]
fn build_all_exclude_broken_glob() {
    let p = project().file("src/main.rs", "fn main() {}").build();

    p.cargo("build --workspace --exclude '[*z'")
        .with_status(101)
        .with_stderr_contains("[ERROR] cannot build glob pattern from `[*z`")
        .run();
}

#[cargo_test]
fn build_all_workspace_implicit_examples() {
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
        .file("src/lib.rs", "")
        .file("src/bin/a.rs", "fn main() {}")
        .file("src/bin/b.rs", "fn main() {}")
        .file("examples/c.rs", "fn main() {}")
        .file("examples/d.rs", "fn main() {}")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "")
        .file("bar/src/bin/e.rs", "fn main() {}")
        .file("bar/src/bin/f.rs", "fn main() {}")
        .file("bar/examples/g.rs", "fn main() {}")
        .file("bar/examples/h.rs", "fn main() {}")
        .build();

    p.cargo("build --workspace --examples")
        .with_stderr(
            "[..] Compiling bar v0.1.0 ([..])\n\
             [..] Compiling foo v0.1.0 ([..])\n\
             [..] Finished dev [unoptimized + debuginfo] target(s) in [..]\n",
        )
        .run();
    assert!(!p.bin("a").is_file());
    assert!(!p.bin("b").is_file());
    assert!(p.bin("examples/c").is_file());
    assert!(p.bin("examples/d").is_file());
    assert!(!p.bin("e").is_file());
    assert!(!p.bin("f").is_file());
    assert!(p.bin("examples/g").is_file());
    assert!(p.bin("examples/h").is_file());
}

#[cargo_test]
fn build_all_virtual_manifest() {
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

    // The order in which bar and baz are built is not guaranteed
    p.cargo("build --workspace")
        .with_stderr_unordered(
            "\
[COMPILING] baz v0.1.0 ([..])
[COMPILING] bar v0.1.0 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cargo_test]
fn build_virtual_manifest_all_implied() {
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

    // The order in which `bar` and `baz` are built is not guaranteed.
    p.cargo("build")
        .with_stderr_unordered(
            "\
[COMPILING] baz v0.1.0 ([..])
[COMPILING] bar v0.1.0 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cargo_test]
fn build_virtual_manifest_one_project() {
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

    p.cargo("build -p bar")
        .with_stderr_does_not_contain("[..]baz[..]")
        .with_stderr(
            "\
[COMPILING] bar v0.1.0 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cargo_test]
fn build_virtual_manifest_glob() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["bar", "baz"]
            "#,
        )
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "pub fn bar() { break_the_build(); }")
        .file("baz/Cargo.toml", &basic_manifest("baz", "0.1.0"))
        .file("baz/src/lib.rs", "pub fn baz() {}")
        .build();

    p.cargo("build -p '*z'")
        .with_stderr_does_not_contain("[..]bar[..]")
        .with_stderr(
            "\
[COMPILING] baz v0.1.0 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cargo_test]
fn build_virtual_manifest_glob_not_found() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["bar"]
            "#,
        )
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "pub fn bar() {}")
        .build();

    p.cargo("build -p bar -p '*z'")
        .with_status(101)
        .with_stderr("[ERROR] package pattern(s) `*z` not found in workspace [..]")
        .run();
}

#[cargo_test]
fn build_virtual_manifest_broken_glob() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["bar"]
            "#,
        )
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "pub fn bar() {}")
        .build();

    p.cargo("build -p '[*z'")
        .with_status(101)
        .with_stderr_contains("[ERROR] cannot build glob pattern from `[*z`")
        .run();
}

#[cargo_test]
fn build_all_virtual_manifest_implicit_examples() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["bar", "baz"]
            "#,
        )
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "")
        .file("bar/src/bin/a.rs", "fn main() {}")
        .file("bar/src/bin/b.rs", "fn main() {}")
        .file("bar/examples/c.rs", "fn main() {}")
        .file("bar/examples/d.rs", "fn main() {}")
        .file("baz/Cargo.toml", &basic_manifest("baz", "0.1.0"))
        .file("baz/src/lib.rs", "")
        .file("baz/src/bin/e.rs", "fn main() {}")
        .file("baz/src/bin/f.rs", "fn main() {}")
        .file("baz/examples/g.rs", "fn main() {}")
        .file("baz/examples/h.rs", "fn main() {}")
        .build();

    // The order in which bar and baz are built is not guaranteed
    p.cargo("build --workspace --examples")
        .with_stderr_unordered(
            "\
[COMPILING] baz v0.1.0 ([..])
[COMPILING] bar v0.1.0 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
    assert!(!p.bin("a").is_file());
    assert!(!p.bin("b").is_file());
    assert!(p.bin("examples/c").is_file());
    assert!(p.bin("examples/d").is_file());
    assert!(!p.bin("e").is_file());
    assert!(!p.bin("f").is_file());
    assert!(p.bin("examples/g").is_file());
    assert!(p.bin("examples/h").is_file());
}

#[cargo_test]
fn build_all_member_dependency_same_name() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["a"]
            "#,
        )
        .file(
            "a/Cargo.toml",
            r#"
                [project]
                name = "a"
                version = "0.1.0"

                [dependencies]
                a = "0.1.0"
            "#,
        )
        .file("a/src/lib.rs", "pub fn a() {}")
        .build();

    Package::new("a", "0.1.0").publish();

    p.cargo("build --workspace")
        .with_stderr(
            "[UPDATING] `[..]` index\n\
             [DOWNLOADING] crates ...\n\
             [DOWNLOADED] a v0.1.0 ([..])\n\
             [COMPILING] a v0.1.0\n\
             [COMPILING] a v0.1.0 ([..])\n\
             [FINISHED] dev [unoptimized + debuginfo] target(s) in [..]\n",
        )
        .run();
}

#[cargo_test]
fn run_proper_binary() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                authors = []
                version = "0.0.0"
                [[bin]]
                name = "main"
                [[bin]]
                name = "other"
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "src/bin/main.rs",
            r#"fn main() { panic!("This should never be run."); }"#,
        )
        .file("src/bin/other.rs", "fn main() {}")
        .build();

    p.cargo("run --bin other").run();
}

#[cargo_test]
fn run_proper_binary_main_rs() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/lib.rs", "")
        .file("src/bin/main.rs", "fn main() {}")
        .build();

    p.cargo("run --bin foo").run();
}

#[cargo_test]
fn run_proper_alias_binary_from_src() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                authors = []
                version = "0.0.0"
                [[bin]]
                name = "foo"
                [[bin]]
                name = "bar"
            "#,
        )
        .file("src/foo.rs", r#"fn main() { println!("foo"); }"#)
        .file("src/bar.rs", r#"fn main() { println!("bar"); }"#)
        .build();

    p.cargo("build --workspace").run();
    p.process(&p.bin("foo")).with_stdout("foo\n").run();
    p.process(&p.bin("bar")).with_stdout("bar\n").run();
}

#[cargo_test]
fn run_proper_alias_binary_main_rs() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                authors = []
                version = "0.0.0"
                [[bin]]
                name = "foo"
                [[bin]]
                name = "bar"
            "#,
        )
        .file("src/main.rs", r#"fn main() { println!("main"); }"#)
        .build();

    p.cargo("build --workspace").run();
    p.process(&p.bin("foo")).with_stdout("main\n").run();
    p.process(&p.bin("bar")).with_stdout("main\n").run();
}

#[cargo_test]
fn run_proper_binary_main_rs_as_foo() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file(
            "src/foo.rs",
            r#" fn main() { panic!("This should never be run."); }"#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("run --bin foo").run();
}

#[cargo_test]
fn rustc_wrapper() {
    let p = project().file("src/lib.rs", "").build();
    let wrapper = tools::echo_wrapper();
    let running = format!(
        "[RUNNING] `{} rustc --crate-name foo [..]",
        wrapper.display()
    );
    p.cargo("build -v")
        .env("RUSTC_WRAPPER", &wrapper)
        .with_stderr_contains(&running)
        .run();
    p.build_dir().rm_rf();
    p.cargo("build -v")
        .env("RUSTC_WORKSPACE_WRAPPER", &wrapper)
        .with_stderr_contains(&running)
        .run();
}

#[cargo_test]
fn rustc_wrapper_relative() {
    Package::new("bar", "1.0.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [dependencies]
                bar = "1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .build();
    let wrapper = tools::echo_wrapper();
    let exe_name = wrapper.file_name().unwrap().to_str().unwrap();
    let relative_path = format!("./{}", exe_name);
    fs::hard_link(&wrapper, p.root().join(exe_name)).unwrap();
    let running = format!("[RUNNING] `[ROOT]/foo/./{} rustc[..]", exe_name);
    p.cargo("build -v")
        .env("RUSTC_WRAPPER", &relative_path)
        .with_stderr_contains(&running)
        .run();
    p.build_dir().rm_rf();
    p.cargo("build -v")
        .env("RUSTC_WORKSPACE_WRAPPER", &relative_path)
        .with_stderr_contains(&running)
        .run();
    p.build_dir().rm_rf();
    p.change_file(
        ".cargo/config.toml",
        &format!(
            r#"
                build.rustc-wrapper = "./{}"
            "#,
            exe_name
        ),
    );
    p.cargo("build -v").with_stderr_contains(&running).run();
}

#[cargo_test]
fn rustc_wrapper_from_path() {
    let p = project().file("src/lib.rs", "").build();
    p.cargo("build -v")
        .env("RUSTC_WRAPPER", "wannabe_sccache")
        .with_status(101)
        .with_stderr_contains("[..]`wannabe_sccache rustc [..]")
        .run();
    p.build_dir().rm_rf();
    p.cargo("build -v")
        .env("RUSTC_WORKSPACE_WRAPPER", "wannabe_sccache")
        .with_status(101)
        .with_stderr_contains("[..]`wannabe_sccache rustc [..]")
        .run();
}

#[cargo_test]
fn cdylib_not_lifted() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                authors = []
                version = "0.1.0"

                [lib]
                crate-type = ["cdylib"]
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build").run();

    let files = if cfg!(windows) {
        if cfg!(target_env = "msvc") {
            vec!["foo.dll.lib", "foo.dll.exp", "foo.dll"]
        } else {
            vec!["libfoo.dll.a", "foo.dll"]
        }
    } else if cfg!(target_os = "macos") {
        vec!["libfoo.dylib"]
    } else {
        vec!["libfoo.so"]
    };

    for file in files {
        println!("checking: {}", file);
        assert!(p.root().join("target/debug/deps").join(&file).is_file());
    }
}

#[cargo_test]
fn cdylib_final_outputs() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo-bar"
                authors = []
                version = "0.1.0"

                [lib]
                crate-type = ["cdylib"]
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build").run();

    let files = if cfg!(windows) {
        if cfg!(target_env = "msvc") {
            vec!["foo_bar.dll.lib", "foo_bar.dll"]
        } else {
            vec!["foo_bar.dll", "libfoo_bar.dll.a"]
        }
    } else if cfg!(target_os = "macos") {
        vec!["libfoo_bar.dylib"]
    } else {
        vec!["libfoo_bar.so"]
    };

    for file in files {
        println!("checking: {}", file);
        assert!(p.root().join("target/debug").join(&file).is_file());
    }
}

#[cargo_test]
fn deterministic_cfg_flags() {
    // This bug is non-deterministic.

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.1.0"
                authors = []
                build = "build.rs"

                [features]
                default = ["f_a", "f_b", "f_c", "f_d"]
                f_a = []
                f_b = []
                f_c = []
                f_d = []
            "#,
        )
        .file(
            "build.rs",
            r#"
                fn main() {
                    println!("cargo:rustc-cfg=cfg_a");
                    println!("cargo:rustc-cfg=cfg_b");
                    println!("cargo:rustc-cfg=cfg_c");
                    println!("cargo:rustc-cfg=cfg_d");
                    println!("cargo:rustc-cfg=cfg_e");
                }
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("build -v")
        .with_stderr(
            "\
[COMPILING] foo v0.1.0 [..]
[RUNNING] [..]
[RUNNING] [..]
[RUNNING] `rustc --crate-name foo [..] \
--cfg[..]default[..]--cfg[..]f_a[..]--cfg[..]f_b[..]\
--cfg[..]f_c[..]--cfg[..]f_d[..] \
--cfg cfg_a --cfg cfg_b --cfg cfg_c --cfg cfg_d --cfg cfg_e`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]",
        )
        .run();
}

#[cargo_test]
fn explicit_bins_without_paths() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                authors = []

                [[bin]]
                name = "foo"

                [[bin]]
                name = "bar"
            "#,
        )
        .file("src/lib.rs", "")
        .file("src/main.rs", "fn main() {}")
        .file("src/bin/bar.rs", "fn main() {}")
        .build();

    p.cargo("build").run();
}

#[cargo_test]
fn no_bin_in_src_with_lib() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/lib.rs", "")
        .file("src/foo.rs", "fn main() {}")
        .build();

    p.cargo("build")
        .with_status(101)
        .with_stderr_contains(
            "\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  can't find `foo` bin, specify bin.path",
        )
        .run();
}

#[cargo_test]
fn inferred_bins() {
    let p = project()
        .file("src/main.rs", "fn main() {}")
        .file("src/bin/bar.rs", "fn main() {}")
        .file("src/bin/baz/main.rs", "fn main() {}")
        .build();

    p.cargo("build").run();
    assert!(p.bin("foo").is_file());
    assert!(p.bin("bar").is_file());
    assert!(p.bin("baz").is_file());
}

#[cargo_test]
fn inferred_bins_duplicate_name() {
    // this should fail, because we have two binaries with the same name
    let p = project()
        .file("src/main.rs", "fn main() {}")
        .file("src/bin/bar.rs", "fn main() {}")
        .file("src/bin/bar/main.rs", "fn main() {}")
        .build();

    p.cargo("build").with_status(101).with_stderr_contains(
            "[..]found duplicate binary name bar, but all binary targets must have a unique name[..]",
        )
        .run();
}

#[cargo_test]
fn inferred_bin_path() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []

            [[bin]]
            name = "bar"
            # Note, no `path` key!
            "#,
        )
        .file("src/bin/bar/main.rs", "fn main() {}")
        .build();

    p.cargo("build").run();
    assert!(p.bin("bar").is_file());
}

#[cargo_test]
fn inferred_examples() {
    let p = project()
        .file("src/lib.rs", "fn main() {}")
        .file("examples/bar.rs", "fn main() {}")
        .file("examples/baz/main.rs", "fn main() {}")
        .build();

    p.cargo("build --examples").run();
    assert!(p.bin("examples/bar").is_file());
    assert!(p.bin("examples/baz").is_file());
}

#[cargo_test]
fn inferred_tests() {
    let p = project()
        .file("src/lib.rs", "fn main() {}")
        .file("tests/bar.rs", "fn main() {}")
        .file("tests/baz/main.rs", "fn main() {}")
        .build();

    p.cargo("test --test=bar --test=baz").run();
}

#[cargo_test]
fn inferred_benchmarks() {
    let p = project()
        .file("src/lib.rs", "fn main() {}")
        .file("benches/bar.rs", "fn main() {}")
        .file("benches/baz/main.rs", "fn main() {}")
        .build();

    p.cargo("bench --bench=bar --bench=baz").run();
}

#[cargo_test]
fn target_edition() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"

                [lib]
                edition = "2018"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build -v")
        .with_stderr_contains(
            "\
[COMPILING] foo v0.0.1 ([..])
[RUNNING] `rustc [..]--edition=2018 [..]
",
        )
        .run();
}

#[cargo_test]
fn target_edition_override() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []
                edition = "2018"

                [lib]
                edition = "2015"
            "#,
        )
        .file(
            "src/lib.rs",
            "
                pub fn async() {}
                pub fn try() {}
                pub fn await() {}
            ",
        )
        .build();

    p.cargo("build -v").run();
}

#[cargo_test]
fn same_metadata_different_directory() {
    // A top-level crate built in two different workspaces should have the
    // same metadata hash.
    let p = project()
        .at("foo1")
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]))
        .build();
    let output = t!(String::from_utf8(
        t!(p.cargo("build -v").exec_with_output()).stderr,
    ));
    let metadata = output
        .split_whitespace()
        .find(|arg| arg.starts_with("metadata="))
        .unwrap();

    let p = project()
        .at("foo2")
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]))
        .build();

    p.cargo("build -v")
        .with_stderr_contains(format!("[..]{}[..]", metadata))
        .run();
}

#[cargo_test]
fn building_a_dependent_crate_witout_bin_should_fail() {
    Package::new("testless", "0.1.0")
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "testless"
                version = "0.1.0"

                [[bin]]
                name = "a_bin"
            "#,
        )
        .file("src/lib.rs", "")
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.1.0"

                [dependencies]
                testless = "0.1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build")
        .with_status(101)
        .with_stderr_contains("[..]can't find `a_bin` bin, specify bin.path")
        .run();
}

#[cargo_test]
#[cfg(any(target_os = "macos", target_os = "ios"))]
fn uplift_dsym_of_bin_on_mac() {
    use cargo_test_support::paths::is_symlink;
    let p = project()
        .file("src/main.rs", "fn main() { panic!(); }")
        .file("src/bin/b.rs", "fn main() { panic!(); }")
        .file("examples/c.rs", "fn main() { panic!(); }")
        .file("tests/d.rs", "fn main() { panic!(); }")
        .build();

    p.cargo("build --bins --examples --tests")
        .enable_mac_dsym()
        .run();
    assert!(p.target_debug_dir().join("foo.dSYM").is_dir());
    assert!(p.target_debug_dir().join("b.dSYM").is_dir());
    assert!(is_symlink(&p.target_debug_dir().join("b.dSYM")));
    assert!(p.target_debug_dir().join("examples/c.dSYM").is_dir());
    assert!(!p.target_debug_dir().join("c.dSYM").exists());
    assert!(!p.target_debug_dir().join("d.dSYM").exists());
}

#[cargo_test]
#[cfg(any(target_os = "macos", target_os = "ios"))]
fn uplift_dsym_of_bin_on_mac_when_broken_link_exists() {
    use cargo_test_support::paths::is_symlink;
    let p = project()
        .file("src/main.rs", "fn main() { panic!(); }")
        .build();
    let dsym = p.target_debug_dir().join("foo.dSYM");

    p.cargo("build").enable_mac_dsym().run();
    assert!(dsym.is_dir());

    // Simulate the situation where the underlying dSYM bundle goes missing
    // but the uplifted symlink to it remains. This would previously cause
    // builds to permanently fail until the bad symlink was manually removed.
    dsym.rm_rf();
    p.symlink(
        p.target_debug_dir()
            .join("deps")
            .join("foo-baaaaaadbaaaaaad.dSYM"),
        &dsym,
    );
    assert!(is_symlink(&dsym));
    assert!(!dsym.exists());

    p.cargo("build").enable_mac_dsym().run();
    assert!(dsym.is_dir());
}

#[cargo_test]
#[cfg(all(target_os = "windows", target_env = "msvc"))]
fn uplift_pdb_of_bin_on_windows() {
    let p = project()
        .file("src/main.rs", "fn main() { panic!(); }")
        .file("src/bin/b.rs", "fn main() { panic!(); }")
        .file("src/bin/foo-bar.rs", "fn main() { panic!(); }")
        .file("examples/c.rs", "fn main() { panic!(); }")
        .file("tests/d.rs", "fn main() { panic!(); }")
        .build();

    p.cargo("build --bins --examples --tests").run();
    assert!(p.target_debug_dir().join("foo.pdb").is_file());
    assert!(p.target_debug_dir().join("b.pdb").is_file());
    assert!(p.target_debug_dir().join("examples/c.pdb").exists());
    assert!(p.target_debug_dir().join("foo-bar.exe").is_file());
    assert!(p.target_debug_dir().join("foo_bar.pdb").is_file());
    assert!(!p.target_debug_dir().join("c.pdb").exists());
    assert!(!p.target_debug_dir().join("d.pdb").exists());
}

// Ensure that `cargo build` chooses the correct profile for building
// targets based on filters (assuming `--profile` is not specified).
#[cargo_test]
fn build_filter_infer_profile() {
    let p = project()
        .file("src/lib.rs", "")
        .file("src/main.rs", "fn main() {}")
        .file("tests/t1.rs", "")
        .file("benches/b1.rs", "")
        .file("examples/ex1.rs", "fn main() {}")
        .build();

    p.cargo("build -v")
        .with_stderr_contains(
            "[RUNNING] `rustc --crate-name foo src/lib.rs [..]--crate-type lib \
             --emit=[..]link[..]",
        )
        .with_stderr_contains(
            "[RUNNING] `rustc --crate-name foo src/main.rs [..]--crate-type bin \
             --emit=[..]link[..]",
        )
        .run();

    p.root().join("target").rm_rf();
    p.cargo("build -v --test=t1")
        .with_stderr_contains(
            "[RUNNING] `rustc --crate-name foo src/lib.rs [..]--crate-type lib \
             --emit=[..]link[..]-C debuginfo=2 [..]",
        )
        .with_stderr_contains(
            "[RUNNING] `rustc --crate-name t1 tests/t1.rs [..]--emit=[..]link[..]\
             -C debuginfo=2 [..]",
        )
        .with_stderr_contains(
            "[RUNNING] `rustc --crate-name foo src/main.rs [..]--crate-type bin \
             --emit=[..]link[..]-C debuginfo=2 [..]",
        )
        .run();

    p.root().join("target").rm_rf();
    // Bench uses test profile without `--release`.
    p.cargo("build -v --bench=b1")
        .with_stderr_contains(
            "[RUNNING] `rustc --crate-name foo src/lib.rs [..]--crate-type lib \
             --emit=[..]link[..]-C debuginfo=2 [..]",
        )
        .with_stderr_contains(
            "[RUNNING] `rustc --crate-name b1 benches/b1.rs [..]--emit=[..]link[..]\
             -C debuginfo=2 [..]",
        )
        .with_stderr_does_not_contain("opt-level")
        .with_stderr_contains(
            "[RUNNING] `rustc --crate-name foo src/main.rs [..]--crate-type bin \
             --emit=[..]link[..]-C debuginfo=2 [..]",
        )
        .run();
}

#[cargo_test]
fn targets_selected_default() {
    let p = project().file("src/main.rs", "fn main() {}").build();
    p.cargo("build -v")
        // Binaries.
        .with_stderr_contains(
            "[RUNNING] `rustc --crate-name foo src/main.rs [..]--crate-type bin \
             --emit=[..]link[..]",
        )
        // Benchmarks.
        .with_stderr_does_not_contain(
            "[RUNNING] `rustc --crate-name foo src/main.rs [..]--emit=[..]link \
             -C opt-level=3 --test [..]",
        )
        // Unit tests.
        .with_stderr_does_not_contain(
            "[RUNNING] `rustc --crate-name foo src/main.rs [..]--emit=[..]link[..]\
             -C debuginfo=2 --test [..]",
        )
        .run();
}

#[cargo_test]
fn targets_selected_all() {
    let p = project().file("src/main.rs", "fn main() {}").build();
    p.cargo("build -v --all-targets")
        // Binaries.
        .with_stderr_contains(
            "[RUNNING] `rustc --crate-name foo src/main.rs [..]--crate-type bin \
             --emit=[..]link[..]",
        )
        // Unit tests.
        .with_stderr_contains(
            "[RUNNING] `rustc --crate-name foo src/main.rs [..]--emit=[..]link[..]\
             -C debuginfo=2 --test [..]",
        )
        .run();
}

#[cargo_test]
fn all_targets_no_lib() {
    let p = project().file("src/main.rs", "fn main() {}").build();
    p.cargo("build -v --all-targets")
        // Binaries.
        .with_stderr_contains(
            "[RUNNING] `rustc --crate-name foo src/main.rs [..]--crate-type bin \
             --emit=[..]link[..]",
        )
        // Unit tests.
        .with_stderr_contains(
            "[RUNNING] `rustc --crate-name foo src/main.rs [..]--emit=[..]link[..]\
             -C debuginfo=2 --test [..]",
        )
        .run();
}

#[cargo_test]
fn no_linkable_target() {
    // Issue 3169: this is currently not an error as per discussion in PR #4797.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                authors = []
                [dependencies]
                the_lib = { path = "the_lib" }
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "the_lib/Cargo.toml",
            r#"
                [package]
                name = "the_lib"
                version = "0.1.0"
                [lib]
                name = "the_lib"
                crate-type = ["staticlib"]
            "#,
        )
        .file("the_lib/src/lib.rs", "pub fn foo() {}")
        .build();
    p.cargo("build")
        .with_stderr_contains(
            "[WARNING] The package `the_lib` provides no linkable [..] \
             while compiling `foo`. [..] in `the_lib`'s Cargo.toml. [..]",
        )
        .run();
}

#[cargo_test]
fn avoid_dev_deps() {
    Package::new("foo", "1.0.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.1.0"
                authors = []

                [dev-dependencies]
                baz = "1.0.0"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("build")
        .with_status(101)
        .with_stderr(
            "\
[UPDATING] [..]
[ERROR] no matching package named `baz` found
location searched: registry `crates-io`
required by package `bar v0.1.0 ([..]/foo)`
",
        )
        .run();
    p.cargo("build -Zavoid-dev-deps")
        .masquerade_as_nightly_cargo()
        .run();
}

#[cargo_test]
fn default_cargo_config_jobs() {
    let p = project()
        .file("src/lib.rs", "")
        .file(
            ".cargo/config",
            r#"
                [build]
                jobs = 1
            "#,
        )
        .build();
    p.cargo("build -v").run();
}

#[cargo_test]
fn good_cargo_config_jobs() {
    let p = project()
        .file("src/lib.rs", "")
        .file(
            ".cargo/config",
            r#"
                [build]
                jobs = 4
            "#,
        )
        .build();
    p.cargo("build -v").run();
}

#[cargo_test]
fn invalid_cargo_config_jobs() {
    let p = project()
        .file("src/lib.rs", "")
        .file(
            ".cargo/config",
            r#"
                [build]
                jobs = 0
            "#,
        )
        .build();
    p.cargo("build -v")
        .with_status(101)
        .with_stderr_contains("error: jobs may not be 0")
        .run();
}

#[cargo_test]
fn invalid_jobs() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]))
        .build();

    p.cargo("build --jobs -1")
        .with_status(1)
        .with_stderr_contains(
            "error: Found argument '-1' which wasn't expected, or isn't valid in this context",
        )
        .run();

    p.cargo("build --jobs over9000")
        .with_status(1)
        .with_stderr("error: Invalid value: could not parse `over9000` as a number")
        .run();
}

#[cargo_test]
fn target_filters_workspace() {
    let ws = project()
        .at("ws")
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["a", "b"]
            "#,
        )
        .file("a/Cargo.toml", &basic_lib_manifest("a"))
        .file("a/src/lib.rs", "")
        .file("a/examples/ex1.rs", "fn main() {}")
        .file("b/Cargo.toml", &basic_bin_manifest("b"))
        .file("b/src/lib.rs", "")
        .file("b/src/main.rs", "fn main() {}")
        .build();

    ws.cargo("build -v --example ex")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] no example target named `ex`

<tab>Did you mean `ex1`?",
        )
        .run();

    ws.cargo("build -v --example 'ex??'")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] no example target matches pattern `ex??`

<tab>Did you mean `ex1`?",
        )
        .run();

    ws.cargo("build -v --lib")
        .with_stderr_contains("[RUNNING] `rustc [..]a/src/lib.rs[..]")
        .with_stderr_contains("[RUNNING] `rustc [..]b/src/lib.rs[..]")
        .run();

    ws.cargo("build -v --example ex1")
        .with_stderr_contains("[RUNNING] `rustc [..]a/examples/ex1.rs[..]")
        .run();
}

#[cargo_test]
fn target_filters_workspace_not_found() {
    let ws = project()
        .at("ws")
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["a", "b"]
            "#,
        )
        .file("a/Cargo.toml", &basic_bin_manifest("a"))
        .file("a/src/main.rs", "fn main() {}")
        .file("b/Cargo.toml", &basic_bin_manifest("b"))
        .file("b/src/main.rs", "fn main() {}")
        .build();

    ws.cargo("build -v --lib")
        .with_status(101)
        .with_stderr("[ERROR] no library targets found in packages: a, b")
        .run();
}

#[cfg(unix)]
#[cargo_test]
fn signal_display() {
    // Cause the compiler to crash with a signal.
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                [dependencies]
                pm = { path = "pm" }
            "#,
        )
        .file(
            "src/lib.rs",
            r#"
                #[macro_use]
                extern crate pm;

                #[derive(Foo)]
                pub struct S;
            "#,
        )
        .file(
            "pm/Cargo.toml",
            r#"
                [package]
                name = "pm"
                version = "0.1.0"
                [lib]
                proc-macro = true
            "#,
        )
        .file(
            "pm/src/lib.rs",
            r#"
                extern crate proc_macro;
                use proc_macro::TokenStream;

                #[proc_macro_derive(Foo)]
                pub fn derive(_input: TokenStream) -> TokenStream {
                    std::process::abort()
                }
            "#,
        )
        .build();

    foo.cargo("build")
        .with_stderr(
            "\
[COMPILING] pm [..]
[COMPILING] foo [..]
[ERROR] could not compile `foo`

Caused by:
  process didn't exit successfully: `rustc [..]` (signal: 6, SIGABRT: process abort signal)
",
        )
        .with_status(101)
        .run();
}

#[cargo_test]
fn tricky_pipelining() {
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                [dependencies]
                bar = { path = "bar" }
            "#,
        )
        .file("src/lib.rs", "extern crate bar;")
        .file("bar/Cargo.toml", &basic_lib_manifest("bar"))
        .file("bar/src/lib.rs", "")
        .build();

    foo.cargo("build -p bar")
        .env("CARGO_BUILD_PIPELINING", "true")
        .run();
    foo.cargo("build -p foo")
        .env("CARGO_BUILD_PIPELINING", "true")
        .run();
}

#[cargo_test]
fn pipelining_works() {
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                [dependencies]
                bar = { path = "bar" }
            "#,
        )
        .file("src/lib.rs", "extern crate bar;")
        .file("bar/Cargo.toml", &basic_lib_manifest("bar"))
        .file("bar/src/lib.rs", "")
        .build();

    foo.cargo("build")
        .env("CARGO_BUILD_PIPELINING", "true")
        .with_stdout("")
        .with_stderr(
            "\
[COMPILING] [..]
[COMPILING] [..]
[FINISHED] [..]
",
        )
        .run();
}

#[cargo_test]
fn pipelining_big_graph() {
    // Create a crate graph of the form {a,b}{0..29}, where {a,b}(n) depend on {a,b}(n+1)
    // Then have `foo`, a binary crate, depend on the whole thing.
    let mut project = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                [dependencies]
                a1 = { path = "a1" }
                b1 = { path = "b1" }
            "#,
        )
        .file("src/main.rs", "fn main(){}");

    for n in 0..30 {
        for x in &["a", "b"] {
            project = project
                .file(
                    &format!("{x}{n}/Cargo.toml", x = x, n = n),
                    &format!(
                        r#"
                            [package]
                            name = "{x}{n}"
                            version = "0.1.0"
                            [dependencies]
                            a{np1} = {{ path = "../a{np1}" }}
                            b{np1} = {{ path = "../b{np1}" }}
                        "#,
                        x = x,
                        n = n,
                        np1 = n + 1
                    ),
                )
                .file(&format!("{x}{n}/src/lib.rs", x = x, n = n), "");
        }
    }

    let foo = project
        .file("a30/Cargo.toml", &basic_lib_manifest("a30"))
        .file(
            "a30/src/lib.rs",
            r#"compile_error!("don't actually build me");"#,
        )
        .file("b30/Cargo.toml", &basic_lib_manifest("b30"))
        .file("b30/src/lib.rs", "")
        .build();
    foo.cargo("build -p foo")
        .env("CARGO_BUILD_PIPELINING", "true")
        .with_status(101)
        .with_stderr_contains("[ERROR] could not compile `a30`[..]")
        .run();
}

#[cargo_test]
fn forward_rustc_output() {
    let foo = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = '2018'
                [dependencies]
                bar = { path = "bar" }
            "#,
        )
        .file("src/lib.rs", "bar::foo!();")
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.1.0"
                [lib]
                proc-macro = true
            "#,
        )
        .file(
            "bar/src/lib.rs",
            r#"
                extern crate proc_macro;
                use proc_macro::*;

                #[proc_macro]
                pub fn foo(input: TokenStream) -> TokenStream {
                    println!("a");
                    println!("b");
                    println!("{{}}");
                    eprintln!("c");
                    eprintln!("d");
                    eprintln!("{{a"); // "malformed json"
                    input
                }
            "#,
        )
        .build();

    foo.cargo("build")
        .with_stdout("a\nb\n{}")
        .with_stderr(
            "\
[COMPILING] [..]
[COMPILING] [..]
c
d
{a
[FINISHED] [..]
",
        )
        .run();
}

#[cargo_test]
fn build_lib_only() {
    let p = project()
        .file("src/main.rs", "fn main() {}")
        .file("src/lib.rs", r#" "#)
        .build();

    p.cargo("build --lib -v")
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([CWD])
[RUNNING] `rustc --crate-name foo src/lib.rs [..]--crate-type lib \
        --emit=[..]link[..]-C debuginfo=2 \
        -C metadata=[..] \
        --out-dir [..] \
        -L dependency=[CWD]/target/debug/deps`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]",
        )
        .run();
}

#[cargo_test]
fn build_with_no_lib() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("build --lib")
        .with_status(101)
        .with_stderr("[ERROR] no library targets found in package `foo`")
        .run();
}

#[cargo_test]
fn build_with_relative_cargo_home_path() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]

                name = "foo"
                version = "0.0.1"
                authors = ["wycats@example.com"]

                [dependencies]

                "test-dependency" = { path = "src/test_dependency" }
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("src/test_dependency/src/lib.rs", r#" "#)
        .file(
            "src/test_dependency/Cargo.toml",
            &basic_manifest("test-dependency", "0.0.1"),
        )
        .build();

    p.cargo("build").env("CARGO_HOME", "./cargo_home/").run();
}

#[cargo_test]
fn user_specific_cfgs_are_filtered_out() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", r#"fn main() {}"#)
        .file(
            "build.rs",
            r#"
            fn main() {
                assert!(std::env::var_os("CARGO_CFG_PROC_MACRO").is_none());
                assert!(std::env::var_os("CARGO_CFG_DEBUG_ASSERTIONS").is_none());
            }
            "#,
        )
        .build();

    p.cargo("rustc -- --cfg debug_assertions --cfg proc_macro")
        .run();
    p.process(&p.bin("foo")).run();
}

#[cargo_test]
fn close_output() {
    // What happens when stdout or stderr is closed during a build.

    // Server to know when rustc has spawned.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2018"

                [lib]
                proc-macro = true

                [[bin]]
                name = "foobar"
            "#,
        )
        .file(
            "src/lib.rs",
            &r#"
                use proc_macro::TokenStream;
                use std::io::Read;

                #[proc_macro]
                pub fn repro(_input: TokenStream) -> TokenStream {
                    println!("hello stdout!");
                    eprintln!("hello stderr!");
                    // Tell the test we have started.
                    let mut socket = std::net::TcpStream::connect("__ADDR__").unwrap();
                    // Wait for the test to tell us to start printing.
                    let mut buf = [0];
                    drop(socket.read_exact(&mut buf));
                    let use_stderr = std::env::var("__CARGO_REPRO_STDERR").is_ok();
                    // Emit at least 1MB of data.
                    // Linux pipes can buffer up to 64KB.
                    // This test seems to be sensitive to having other threads
                    // calling fork. My hypothesis is that the stdout/stderr
                    // file descriptors are duplicated into the child process,
                    // and during the short window between fork and exec, the
                    // file descriptor is kept alive long enough for the
                    // build to finish. It's a half-baked theory, but this
                    // seems to prevent the spurious errors in CI.
                    // An alternative solution is to run this test in
                    // a single-threaded environment.
                    for i in 0..100000 {
                        if use_stderr {
                            eprintln!("0123456789{}", i);
                        } else {
                            println!("0123456789{}", i);
                        }
                    }
                    TokenStream::new()
                }
            "#
            .replace("__ADDR__", &addr.to_string()),
        )
        .file(
            "src/bin/foobar.rs",
            r#"
                foo::repro!();

                fn main() {}
            "#,
        )
        .build();

    // The `stderr` flag here indicates if this should forcefully close stderr or stdout.
    let spawn = |stderr: bool| {
        let mut cmd = p.cargo("build").build_command();
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
        if stderr {
            cmd.env("__CARGO_REPRO_STDERR", "1");
        }
        let mut child = cmd.spawn().unwrap();
        // Wait for proc macro to start.
        let pm_conn = listener.accept().unwrap().0;
        // Close stderr or stdout.
        if stderr {
            drop(child.stderr.take());
        } else {
            drop(child.stdout.take());
        }
        // Tell the proc-macro to continue;
        drop(pm_conn);
        // Read the output from the other channel.
        let out: &mut dyn Read = if stderr {
            child.stdout.as_mut().unwrap()
        } else {
            child.stderr.as_mut().unwrap()
        };
        let mut result = String::new();
        out.read_to_string(&mut result).unwrap();
        let status = child.wait().unwrap();
        assert!(!status.success());
        result
    };

    let stderr = spawn(false);
    compare::match_unordered(
        "\
[COMPILING] foo [..]
hello stderr!
[ERROR] [..]
[WARNING] build failed, waiting for other jobs to finish...
[ERROR] [..]
",
        &stderr,
        None,
    )
    .unwrap();

    // Try again with stderr.
    p.build_dir().rm_rf();
    let stdout = spawn(true);
    assert_eq!(stdout, "hello stdout!\n");
}

#[cargo_test]
fn close_output_during_drain() {
    // Test to close the output during the build phase (drain_the_queue).
    // There was a bug where it would hang.

    // Server to know when rustc has spawned.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();

    // Create a wrapper so the test can know when compiling has started.
    let rustc_wrapper = {
        let p = project()
            .at("compiler")
            .file("Cargo.toml", &basic_manifest("compiler", "1.0.0"))
            .file(
                "src/main.rs",
                &r#"
                    use std::process::Command;
                    use std::env;
                    use std::io::Read;

                    fn main() {
                        // Only wait on the first dependency.
                        if matches!(env::var("CARGO_PKG_NAME").as_deref(), Ok("dep")) {
                            let mut socket = std::net::TcpStream::connect("__ADDR__").unwrap();
                            // Wait for the test to tell us to start printing.
                            let mut buf = [0];
                            drop(socket.read_exact(&mut buf));
                        }
                        let mut cmd = Command::new("rustc");
                        for arg in env::args_os().skip(1) {
                            cmd.arg(arg);
                        }
                        std::process::exit(cmd.status().unwrap().code().unwrap());
                    }
                "#
                .replace("__ADDR__", &addr.to_string()),
            )
            .build();
        p.cargo("build").run();
        p.bin("compiler")
    };

    Package::new("dep", "1.0.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [dependencies]
                dep = "1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    // Spawn cargo, wait for the first rustc to start, and then close stderr.
    let mut cmd = process(&cargo_exe())
        .arg("check")
        .cwd(p.root())
        .env("RUSTC", rustc_wrapper)
        .build_command();
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn().expect("cargo should spawn");
    // Wait for the rustc wrapper to start.
    let rustc_conn = listener.accept().unwrap().0;
    // Close stderr to force an error.
    drop(child.stderr.take());
    // Tell the wrapper to continue.
    drop(rustc_conn);
    match child.wait() {
        Ok(status) => assert!(!status.success()),
        Err(e) => panic!("child wait failed: {}", e),
    }
}

use cargo_test_support::registry::Dependency;

#[cargo_test]
fn reduced_reproduction_8249() {
    // https://github.com/rust-lang/cargo/issues/8249
    Package::new("a-src", "0.1.0").links("a").publish();
    Package::new("a-src", "0.2.0").links("a").publish();

    Package::new("b", "0.1.0")
        .add_dep(Dependency::new("a-src", "0.1").optional(true))
        .publish();
    Package::new("b", "0.2.0")
        .add_dep(Dependency::new("a-src", "0.2").optional(true))
        .publish();

    Package::new("c", "1.0.0")
        .add_dep(&Dependency::new("b", "0.1.0"))
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [dependencies]
                b = { version = "*", features = ["a-src"] }
                a-src = "*"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("generate-lockfile").run();
    cargo_util::paths::append(&p.root().join("Cargo.toml"), b"c = \"*\"").unwrap();
    p.cargo("check").run();
    p.cargo("check").run();
}

#[cargo_test]
fn target_directory_backup_exclusion() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]))
        .build();

    // Newly created target/ should have CACHEDIR.TAG inside...
    p.cargo("build").run();
    let cachedir_tag = p.build_dir().join("CACHEDIR.TAG");
    assert!(cachedir_tag.is_file());
    assert!(fs::read_to_string(&cachedir_tag)
        .unwrap()
        .starts_with("Signature: 8a477f597d28d172789f06886806bc55"));
    // ...but if target/ already exists CACHEDIR.TAG should not be created in it.
    fs::remove_file(&cachedir_tag).unwrap();
    p.cargo("build").run();
    assert!(!&cachedir_tag.is_file());
}

#[cargo_test]
fn simple_terminal_width() {
    if !is_nightly() {
        // --terminal-width is unstable
        return;
    }
    let p = project()
        .file(
            "src/lib.rs",
            r#"
                fn main() {
                    let _: () = 42;
                }
            "#,
        )
        .build();

    p.cargo("build -Zterminal-width=20")
        .masquerade_as_nightly_cargo()
        .with_status(101)
        .with_stderr_contains("3 | ..._: () = 42;")
        .run();
}

#[cargo_test]
fn build_script_o0_default() {
    let p = project()
        .file("src/lib.rs", "")
        .file("build.rs", "fn main() {}")
        .build();

    p.cargo("build -v --release")
        .with_stderr_does_not_contain("[..]build_script_build[..]opt-level[..]")
        .run();
}

#[cargo_test]
fn build_script_o0_default_even_with_release() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [profile.release]
                opt-level = 1
            "#,
        )
        .file("src/lib.rs", "")
        .file("build.rs", "fn main() {}")
        .build();

    p.cargo("build -v --release")
        .with_stderr_does_not_contain("[..]build_script_build[..]opt-level[..]")
        .run();
}

#[cargo_test]
fn primary_package_env_var() {
    // Test that CARGO_PRIMARY_PACKAGE is enabled only for "foo" and not for any dependency.

    let is_primary_package = r#"
        pub fn is_primary_package() -> bool {{
            option_env!("CARGO_PRIMARY_PACKAGE").is_some()
        }}
    "#;

    Package::new("qux", "0.1.0")
        .file("src/lib.rs", is_primary_package)
        .publish();

    let baz = git::new("baz", |project| {
        project
            .file("Cargo.toml", &basic_manifest("baz", "0.1.0"))
            .file("src/lib.rs", is_primary_package)
    });

    let foo = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"
                    version = "0.1.0"

                    [dependencies]
                    bar = {{ path = "bar" }}
                    baz = {{ git = '{}' }}
                    qux = "0.1"
                "#,
                baz.url()
            ),
        )
        .file(
            "src/lib.rs",
            &format!(
                r#"
                    extern crate bar;
                    extern crate baz;
                    extern crate qux;

                    {}

                    #[test]
                    fn verify_primary_package() {{
                        assert!(!bar::is_primary_package());
                        assert!(!baz::is_primary_package());
                        assert!(!qux::is_primary_package());
                        assert!(is_primary_package());
                    }}
                "#,
                is_primary_package
            ),
        )
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", is_primary_package)
        .build();

    foo.cargo("test").run();
}
