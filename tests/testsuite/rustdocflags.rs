//! Tests for setting custom rustdoc flags.

use crate::prelude::*;
use cargo_test_support::project;
use cargo_test_support::rustc_host;
use cargo_test_support::rustc_host_env;
use cargo_test_support::str;

#[cargo_test]
fn parses_env() {
    let p = project().file("src/lib.rs", "").build();

    p.cargo("doc -v")
        .env("RUSTDOCFLAGS", "--cfg=foo")
        .with_stderr_data(str![[r#"
...
[RUNNING] `rustdoc [..] --cfg=foo[..]`
...
"#]])
        .run();
}

#[cargo_test]
fn parses_config() {
    let p = project()
        .file("src/lib.rs", "")
        .file(
            ".cargo/config.toml",
            r#"
                [build]
                rustdocflags = ["--cfg", "foo"]
            "#,
        )
        .build();

    p.cargo("doc -v")
        .with_stderr_data(str![[r#"
...
[RUNNING] `rustdoc [..] --cfg foo [..]`
...
"#]])
        .run();
}

#[cargo_test]
fn bad_flags() {
    let p = project().file("src/lib.rs", "").build();

    p.cargo("doc")
        .env("RUSTDOCFLAGS", "--bogus")
        .with_status(101)
        .with_stderr_data(str![[r#"
...
[ERROR] Unrecognized option: 'bogus'
...
"#]])
        .run();
}

#[cargo_test]
fn rerun() {
    let p = project().file("src/lib.rs", "").build();

    p.cargo("doc").env("RUSTDOCFLAGS", "--cfg=foo").run();
    p.cargo("doc")
        .env("RUSTDOCFLAGS", "--cfg=foo")
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[GENERATED] [ROOT]/foo/target/doc/foo/index.html

"#]])
        .run();
    p.cargo("doc")
        .env("RUSTDOCFLAGS", "--cfg=bar")
        .with_stderr_data(str![[r#"
[DOCUMENTING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[GENERATED] [ROOT]/foo/target/doc/foo/index.html

"#]])
        .run();
}

#[cargo_test]
fn rustdocflags_passed_to_rustdoc_through_cargo_test() {
    let p = project()
        .file(
            "src/lib.rs",
            r#"
                //! ```
                //! assert!(cfg!(do_not_choke));
                //! ```
            "#,
        )
        .build();

    p.cargo("test --doc")
        .env("RUSTDOCFLAGS", "--cfg do_not_choke")
        .run();
}

#[cargo_test]
fn rustdocflags_passed_to_rustdoc_through_cargo_test_only_once() {
    let p = project().file("src/lib.rs", "").build();

    p.cargo("test --doc")
        .env("RUSTDOCFLAGS", "--markdown-no-toc")
        .run();
}

#[cargo_test]
fn rustdocflags_misspelled() {
    let p = project().file("src/main.rs", "fn main() { }").build();

    p.cargo("doc")
        .env("RUSTDOC_FLAGS", "foo")
        .with_stderr_data(str![[r#"
[WARNING] ignoring environment variable `RUSTDOC_FLAGS`
  |
  = [HELP] rustdoc flags are passed via `RUSTDOCFLAGS`
...
"#]])
        .run();
}

#[cargo_test]
fn whitespace() {
    // Checks behavior of different whitespace characters.
    let p = project().file("src/lib.rs", "").build();

    // "too many operands"
    p.cargo("doc")
        .env("RUSTDOCFLAGS", "--crate-version this has spaces")
        .with_stderr_data(str![[r#"
...
[ERROR] could not document `foo`
...
"#]])
        .with_status(101)
        .run();

    p.cargo("doc")
        .env_remove("__CARGO_TEST_FORCE_ARGFILE") // Not applicable for argfile.
        .env(
            "RUSTDOCFLAGS",
            "--crate-version 1111\n2222\t3333\u{00a0}4444",
        )
        .run();

    let contents = p.read_file("target/doc/foo/index.html");
    assert!(contents.contains("1111"));
    assert!(contents.contains("2222"));
    assert!(contents.contains("3333"));
    assert!(contents.contains("4444"));
}

#[cargo_test]
fn not_affected_by_target_rustflags() {
    let cfg = if cfg!(windows) { "windows" } else { "unix" };
    let p = project()
        .file("src/lib.rs", "")
        .file(
            ".cargo/config.toml",
            &format!(
                r#"
                    [target.'cfg({cfg})']
                    rustflags = ["-D", "missing-docs"]

                    [build]
                    rustdocflags = ["--cfg", "foo"]
                "#,
            ),
        )
        .build();

    // `cargo build` should fail due to missing docs.
    p.cargo("build -v")
        .with_status(101)
        .with_stderr_data(str![[r#"
...
[RUNNING] `rustc [..] -D missing-docs`
...
"#]])
        .run();

    // `cargo doc` shouldn't fail.
    p.cargo("doc -v")
        .with_stderr_data(str![[r#"
...
[RUNNING] `rustdoc [..] --cfg foo[..]`
...
"#]])
        .run();
}

#[cargo_test]
fn target_triple_rustdocflags_works() {
    let host = rustc_host();
    let host_env = rustc_host_env();
    let p = project().file("src/lib.rs", "").build();

    // target.triple.rustdocflags in env works
    p.cargo("doc -v")
        .env(
            &format!("CARGO_TARGET_{host_env}_RUSTDOCFLAGS"),
            "--cfg=foo",
        )
        .with_stderr_data(str![[r#"
...
[RUNNING] `rustdoc[..]--cfg[..]foo[..]`
...
"#]])
        .run();

    // target.triple.rustdocflags in config works
    p.cargo("doc -v")
        .arg("--config")
        .arg(format!("target.{host}.rustdocflags=['--cfg', 'foo']"))
        .with_stderr_data(str![[r#"
...
[RUNNING] `rustdoc [..] --cfg foo [..]`
...
"#]])
        .run();
}

#[cargo_test]
fn target_triple_rustdocflags_works_through_cargo_test() {
    let host = rustc_host();
    let host_env = rustc_host_env();
    let p = project()
        .file(
            "src/lib.rs",
            r#"
                //! ```
                //! assert!(cfg!(foo));
                //! ```
            "#,
        )
        .build();

    // target.triple.rustdocflags in env works
    p.cargo("test --doc -v")
        .env(
            &format!("CARGO_TARGET_{host_env}_RUSTDOCFLAGS"),
            "--cfg=foo",
        )
        .with_stderr_data(str![[r#"
...
[RUNNING] `rustdoc[..]--test[..]--cfg[..]foo[..]`

"#]])
        .with_stdout_data(str![[r#"

running 1 test
test src/lib.rs - (line 2) ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in [ELAPSED]s


"#]])
        .run();

    // target.triple.rustdocflags in config works
    p.cargo("test --doc -v")
        .arg("--config")
        .arg(format!("target.{host}.rustdocflags=['--cfg', 'foo']"))
        .with_stderr_data(str![[r#"
...
[RUNNING] `rustdoc[..]--test[..]--cfg[..]foo[..]`

"#]])
        .with_stdout_data(str![[r#"

running 1 test
test src/lib.rs - (line 2) ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in [ELAPSED]s


"#]])
        .run();
}
