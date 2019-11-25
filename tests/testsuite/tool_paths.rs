//! Tests for configuration values that point to programs.

use cargo_test_support::rustc_host;
use cargo_test_support::{basic_lib_manifest, project};

#[cargo_test]
fn pathless_tools() {
    let target = rustc_host();

    let foo = project()
        .file("Cargo.toml", &basic_lib_manifest("foo"))
        .file("src/lib.rs", "")
        .file(
            ".cargo/config",
            &format!(
                r#"
            [target.{}]
            ar = "nonexistent-ar"
            linker = "nonexistent-linker"
        "#,
                target
            ),
        )
        .build();

    foo.cargo("build --verbose")
        .with_stderr(
            "\
[COMPILING] foo v0.5.0 ([CWD])
[RUNNING] `rustc [..] -C ar=nonexistent-ar -C linker=nonexistent-linker [..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cargo_test]
fn absolute_tools() {
    let target = rustc_host();

    // Escaped as they appear within a TOML config file
    let config = if cfg!(windows) {
        (
            r#"C:\\bogus\\nonexistent-ar"#,
            r#"C:\\bogus\\nonexistent-linker"#,
        )
    } else {
        (r#"/bogus/nonexistent-ar"#, r#"/bogus/nonexistent-linker"#)
    };

    let foo = project()
        .file("Cargo.toml", &basic_lib_manifest("foo"))
        .file("src/lib.rs", "")
        .file(
            ".cargo/config",
            &format!(
                r#"
            [target.{target}]
            ar = "{ar}"
            linker = "{linker}"
        "#,
                target = target,
                ar = config.0,
                linker = config.1
            ),
        )
        .build();

    foo.cargo("build --verbose")
        .with_stderr(
            "\
[COMPILING] foo v0.5.0 ([CWD])
[RUNNING] `rustc [..] -C ar=[..]bogus/nonexistent-ar -C linker=[..]bogus/nonexistent-linker [..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cargo_test]
fn relative_tools() {
    let target = rustc_host();

    // Escaped as they appear within a TOML config file
    let config = if cfg!(windows) {
        (r#".\\nonexistent-ar"#, r#".\\tools\\nonexistent-linker"#)
    } else {
        (r#"./nonexistent-ar"#, r#"./tools/nonexistent-linker"#)
    };

    // Funky directory structure to test that relative tool paths are made absolute
    // by reference to the `.cargo/..` directory and not to (for example) the CWD.
    let p = project()
        .no_manifest()
        .file("bar/Cargo.toml", &basic_lib_manifest("bar"))
        .file("bar/src/lib.rs", "")
        .file(
            ".cargo/config",
            &format!(
                r#"
            [target.{target}]
            ar = "{ar}"
            linker = "{linker}"
        "#,
                target = target,
                ar = config.0,
                linker = config.1
            ),
        )
        .build();

    let prefix = p.root().into_os_string().into_string().unwrap();

    p.cargo("build --verbose").cwd("bar").with_stderr(&format!(
            "\
[COMPILING] bar v0.5.0 ([CWD])
[RUNNING] `rustc [..] -C ar={prefix}/./nonexistent-ar -C linker={prefix}/./tools/nonexistent-linker [..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
            prefix = prefix,
        )).run();
}

#[cargo_test]
fn custom_runner() {
    let target = rustc_host();

    let p = project()
        .file("src/main.rs", "fn main() {}")
        .file("tests/test.rs", "")
        .file("benches/bench.rs", "")
        .file(
            ".cargo/config",
            &format!(
                r#"
            [target.{}]
            runner = "nonexistent-runner -r"
        "#,
                target
            ),
        )
        .build();

    p.cargo("run -- --param")
        .with_status(101)
        .with_stderr_contains(
            "\
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] `nonexistent-runner -r target/debug/foo[EXE] --param`
",
        )
        .run();

    p.cargo("test --test test --verbose -- --param")
        .with_status(101)
        .with_stderr_contains(
            "\
[COMPILING] foo v0.0.1 ([CWD])
[RUNNING] `rustc [..]`
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]
[RUNNING] `nonexistent-runner -r [..]/target/debug/deps/test-[..][EXE] --param`
",
        )
        .run();

    p.cargo("bench --bench bench --verbose -- --param")
        .with_status(101)
        .with_stderr_contains(
            "\
[COMPILING] foo v0.0.1 ([CWD])
[RUNNING] `rustc [..]`
[RUNNING] `rustc [..]`
[FINISHED] bench [optimized] target(s) in [..]
[RUNNING] `nonexistent-runner -r [..]/target/release/deps/bench-[..][EXE] --param --bench`
",
        )
        .run();
}

// can set a custom runner via `target.'cfg(..)'.runner`
#[cargo_test]
fn custom_runner_cfg() {
    let p = project()
        .file("src/main.rs", "fn main() {}")
        .file(
            ".cargo/config",
            r#"
            [target.'cfg(not(target_os = "none"))']
            runner = "nonexistent-runner -r"
            "#,
        )
        .build();

    p.cargo("run -- --param")
        .with_status(101)
        .with_stderr_contains(
            "\
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] `nonexistent-runner -r target/debug/foo[EXE] --param`
",
        )
        .run();
}

// custom runner set via `target.$triple.runner` have precende over `target.'cfg(..)'.runner`
#[cargo_test]
fn custom_runner_cfg_precedence() {
    let target = rustc_host();

    let p = project()
        .file("src/main.rs", "fn main() {}")
        .file(
            ".cargo/config",
            &format!(
                r#"
            [target.'cfg(not(target_os = "none"))']
            runner = "ignored-runner"

            [target.{}]
            runner = "nonexistent-runner -r"
        "#,
                target
            ),
        )
        .build();

    p.cargo("run -- --param")
        .with_status(101)
        .with_stderr_contains(
            "\
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] `nonexistent-runner -r target/debug/foo[EXE] --param`
",
        )
        .run();
}

#[cargo_test]
fn custom_runner_cfg_collision() {
    let p = project()
        .file("src/main.rs", "fn main() {}")
        .file(
            ".cargo/config",
            r#"
            [target.'cfg(not(target_arch = "avr"))']
            runner = "true"

            [target.'cfg(not(target_os = "none"))']
            runner = "false"
            "#,
        )
        .build();

    p.cargo("run -- --param")
        .with_status(101)
        .with_stderr_contains(
            "\
[ERROR] several matching instances of `target.'cfg(..)'.runner` in `.cargo/config`
",
        )
        .run();
}
