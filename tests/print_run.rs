extern crate cargo;
extern crate cargotest;
extern crate hamcrest;

use cargotest::support::{project, execs};
use hamcrest::{assert_that, existing_file};

#[test]
fn single_bin() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/main.rs", r#"
            fn main() { println!("hello"); }
        "#);

    assert_that(p.cargo_process("run").env("CARGO_PRINT_RUN", "1"),
                execs().with_status(0)
                       .with_stderr(&"\
[COMPILING] foo v0.0.1 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]")
                       .with_json(r#"
{
  "reason": "run-profile",
  "program": "[..][/]foo[/]target[/]debug[/]foo",
  "env": {
    "LD_LIBRARY_PATH": "[..]",
    "CARGO_PKG_VERSION_PRE": "",
    "CARGO_PKG_VERSION_PATCH": "1",
    "CARGO_PKG_VERSION_MINOR": "0",
    "CARGO_PKG_VERSION_MAJOR": "0",
    "CARGO_PKG_VERSION": "0.0.1",
    "CARGO_PKG_NAME": "foo",
    "CARGO_PKG_HOMEPAGE": "",
    "CARGO_PKG_DESCRIPTION": "",
    "CARGO_PKG_AUTHORS": "",
    "CARGO_MANIFEST_DIR": "[..]",
    "CARGO": "[..]"
  },
  "cwd": "[..]",
  "args": []
}
"#));
    assert_that(&p.bin("foo"), existing_file());
}

#[test]
fn several_tests() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/lib.rs", r#"
            /// Doctest are not executed with wrap
            /// ```
            /// assert!(false);
            /// ```
            pub fn f() {  }

            #[test] fn test_in_lib() {}
        "#)
        .file("tests/bar.rs", r#"
            #[test] fn test_bar() {}
        "#)
        .file("tests/baz.rs", r#"
            #[test] fn test_baz() {}
        "#);

    let env = r#"{
        "LD_LIBRARY_PATH": "[..]",
        "CARGO_PKG_VERSION_PRE": "",
        "CARGO_PKG_VERSION_PATCH": "1",
        "CARGO_PKG_VERSION_MINOR": "0",
        "CARGO_PKG_VERSION_MAJOR": "0",
        "CARGO_PKG_VERSION": "0.0.1",
        "CARGO_PKG_NAME": "foo",
        "CARGO_PKG_HOMEPAGE": "",
        "CARGO_PKG_DESCRIPTION": "",
        "CARGO_PKG_AUTHORS": "",
        "CARGO_MANIFEST_DIR": "[..]",
        "CARGO": "[..]"
    }"#;
    assert_that(p.cargo_process("test").env("CARGO_PRINT_RUN", "1"),
                execs().with_status(0)
                       .with_json(&format!(r#"
{{
  "reason": "run-profile",
  "program": "[..]bar-[..]",
  "env": {env},
  "cwd": "[..]",
  "args": []
}}

{{
  "reason": "run-profile",
  "program": "[..]baz-[..]",
  "env": {env},
  "cwd": "[..]",
  "args": []
}}

{{
  "reason": "run-profile",
  "program": "[..]foo-[..]",
  "env": {env},
  "cwd": "[..]",
  "args": []
}}
"#, env=env)));
}

