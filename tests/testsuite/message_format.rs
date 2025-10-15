//! Tests for --message-format flag.

use crate::prelude::*;
use cargo_test_support::{basic_lib_manifest, basic_manifest, project, str};

#[cargo_test]
fn cannot_specify_two() {
    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "0.1.0"))
        .file("src/main.rs", "fn main() {}")
        .build();

    let formats = ["human", "json", "short"];

    for a in formats.iter() {
        for b in formats.iter() {
            p.cargo(&format!("build --message-format {},{}", a, b))
                .with_status(101)
                .with_stderr_data(str![[r#"
[ERROR] cannot specify two kinds of `message-format` arguments

"#]])
                .run();
        }
    }
}

#[cargo_test]
fn double_json_works() {
    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "0.1.0"))
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("check --message-format json,json-render-diagnostics")
        .run();
    p.cargo("check --message-format json,json-diagnostic-short")
        .run();
    p.cargo("check --message-format json,json-diagnostic-rendered-ansi")
        .run();
    p.cargo("check --message-format json --message-format json-diagnostic-rendered-ansi")
        .run();
    p.cargo("check --message-format json-diagnostic-rendered-ansi")
        .run();
    p.cargo("check --message-format json-diagnostic-short,json-diagnostic-rendered-ansi")
        .run();
}

#[cargo_test]
fn cargo_renders() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = 'foo'
                version = '0.1.0'

                [dependencies]
                bar = { path = 'bar' }
            "#,
        )
        .file("src/main.rs", "")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "")
        .build();

    p.cargo("check --message-format json-render-diagnostics")
        .with_status(101)
        .with_stdout_data(
            str![[r#"
[
  {
    "reason": "compiler-artifact",
    "...": "{...}"
  },
  {
    "reason": "build-finished",
    "success": false
  }
]
"#]]
            .is_json()
            .against_jsonlines(),
        )
        .with_stderr_contains(
            "\
[CHECKING] bar [..]
[CHECKING] foo [..]
error[..]`main`[..]
",
        )
        .run();
}

#[cargo_test]
fn cargo_renders_short() {
    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "0.1.0"))
        .file("src/main.rs", "")
        .build();

    p.cargo("check --message-format json-render-diagnostics,json-diagnostic-short")
        .with_status(101)
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.1.0 ([ROOT]/foo)
error[E0601]: `main` function not found in crate `foo`
[ERROR] could not compile `foo` (bin "foo") due to 1 previous error

"#]])
        .run();
}

#[cargo_test]
fn cargo_renders_ansi() {
    let p = project()
        .file("Cargo.toml", &basic_manifest("foo", "0.1.0"))
        .file("src/main.rs", "")
        .build();

    p.cargo("check --message-format json-diagnostic-rendered-ansi")
        .with_status(101)
        // Because 1b is the start of an ANSI escape sequence, checking for it
        // allows us to verify that ANSI colors are being emitted without
        // looking for specific color codes, that may change over time.
        .with_stdout_contains("[..]\\u001b[..]")
        .run();
}

#[cargo_test]
fn cargo_renders_doctests() {
    let p = project()
        .file("Cargo.toml", &basic_lib_manifest("foo"))
        .file(
            "src/lib.rs",
            "\
            /// ```rust
            /// bar()
            /// ```
            pub fn bar() {}
            ",
        )
        .build();

    p.cargo("test --doc --message-format short")
        .with_status(101)
        .with_stdout_data(str![[r#"

running 1 test
test src/lib.rs - bar (line 1) ... FAILED

failures:

---- src/lib.rs - bar (line 1) stdout ----
src/lib.rs:2:1: error[E0425]: cannot find function `bar`[..]
[ERROR] aborting due to 1 previous error
Couldn't compile the test.

failures:
    src/lib.rs - bar (line 1)

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in [ELAPSED]s


"#]])
        .run();
}
