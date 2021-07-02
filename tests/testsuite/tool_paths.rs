//! Tests for configuration values that point to programs.

use cargo_test_support::{
    basic_lib_manifest, no_such_file_err_msg, project, rustc_host, rustc_host_env,
};

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
[RUNNING] `rustc [..] -C linker=nonexistent-linker [..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cargo_test]
fn absolute_tools() {
    let target = rustc_host();

    // Escaped as they appear within a TOML config file
    let linker = if cfg!(windows) {
        r#"C:\\bogus\\nonexistent-linker"#
    } else {
        r#"/bogus/nonexistent-linker"#
    };

    let foo = project()
        .file("Cargo.toml", &basic_lib_manifest("foo"))
        .file("src/lib.rs", "")
        .file(
            ".cargo/config",
            &format!(
                r#"
                    [target.{target}]
                    linker = "{linker}"
                "#,
                target = target,
                linker = linker
            ),
        )
        .build();

    foo.cargo("build --verbose")
        .with_stderr(
            "\
[COMPILING] foo v0.5.0 ([CWD])
[RUNNING] `rustc [..] -C linker=[..]bogus/nonexistent-linker [..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cargo_test]
fn relative_tools() {
    let target = rustc_host();

    // Escaped as they appear within a TOML config file
    let linker = if cfg!(windows) {
        r#".\\tools\\nonexistent-linker"#
    } else {
        r#"./tools/nonexistent-linker"#
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
                    linker = "{linker}"
                "#,
                target = target,
                linker = linker
            ),
        )
        .build();

    let prefix = p.root().into_os_string().into_string().unwrap();

    p.cargo("build --verbose")
        .cwd("bar")
        .with_stderr(&format!(
            "\
[COMPILING] bar v0.5.0 ([CWD])
[RUNNING] `rustc [..] -C linker={prefix}/./tools/nonexistent-linker [..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
            prefix = prefix,
        ))
        .run();
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

// custom runner set via `target.$triple.runner` have precedence over `target.'cfg(..)'.runner`
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
        .with_stderr(
            "\
[ERROR] several matching instances of `target.'cfg(..)'.runner` in `.cargo/config`
first match `cfg(not(target_arch = \"avr\"))` located in [..]/foo/.cargo/config
second match `cfg(not(target_os = \"none\"))` located in [..]/foo/.cargo/config
",
        )
        .run();
}

#[cargo_test]
fn custom_runner_env() {
    let p = project().file("src/main.rs", "fn main() {}").build();

    let key = format!("CARGO_TARGET_{}_RUNNER", rustc_host_env());

    p.cargo("run")
        .env(&key, "nonexistent-runner --foo")
        .with_status(101)
        .with_stderr(&format!(
            "\
[COMPILING] foo [..]
[FINISHED] dev [..]
[RUNNING] `nonexistent-runner --foo target/debug/foo[EXE]`
[ERROR] could not execute process `nonexistent-runner --foo target/debug/foo[EXE]` (never executed)

Caused by:
  {}
",
            no_such_file_err_msg()
        ))
        .run();
}

#[cargo_test]
fn custom_runner_env_overrides_config() {
    let target = rustc_host();
    let p = project()
        .file("src/main.rs", "fn main() {}")
        .file(
            ".cargo/config.toml",
            &format!(
                r#"
                    [target.{}]
                    runner = "should-not-run -r"
                "#,
                target
            ),
        )
        .build();

    let key = format!("CARGO_TARGET_{}_RUNNER", rustc_host_env());

    p.cargo("run")
        .env(&key, "should-run --foo")
        .with_status(101)
        .with_stderr_contains("[RUNNING] `should-run --foo target/debug/foo[EXE]`")
        .run();
}

#[cargo_test]
#[cfg(unix)] // Assumes `true` is in PATH.
fn custom_runner_env_true() {
    // Check for a bug where "true" was interpreted as a boolean instead of
    // the executable.
    let p = project().file("src/main.rs", "fn main() {}").build();

    let key = format!("CARGO_TARGET_{}_RUNNER", rustc_host_env());

    p.cargo("run")
        .env(&key, "true")
        .with_stderr_contains("[RUNNING] `true target/debug/foo[EXE]`")
        .run();
}

#[cargo_test]
fn custom_linker_env() {
    let p = project().file("src/main.rs", "fn main() {}").build();

    let key = format!("CARGO_TARGET_{}_LINKER", rustc_host_env());

    p.cargo("build -v")
        .env(&key, "nonexistent-linker")
        .with_status(101)
        .with_stderr_contains("[RUNNING] `rustc [..]-C linker=nonexistent-linker [..]")
        .run();
}

#[cargo_test]
// Temporarily disabled until https://github.com/rust-lang/rust/pull/85270 is in nightly.
#[cfg_attr(target_os = "windows", ignore)]
fn target_in_environment_contains_lower_case() {
    let p = project().file("src/main.rs", "fn main() {}").build();

    let target = rustc_host();
    let env_key = format!(
        "CARGO_TARGET_{}_LINKER",
        target.to_lowercase().replace('-', "_")
    );

    p.cargo("build -v --target")
        .arg(target)
        .env(&env_key, "nonexistent-linker")
        .with_stderr_contains(format!(
            "warning: Environment variables are expected to use uppercase \
             letters and underscores, the variable `{}` will be ignored and \
             have no effect",
            env_key
        ))
        .run();
}

#[cargo_test]
fn cfg_ignored_fields() {
    // Test for some ignored fields in [target.'cfg()'] tables.
    let p = project()
        .file(
            ".cargo/config",
            r#"
            # Try some empty tables.
            [target.'cfg(not(foo))']
            [target.'cfg(not(bar))'.somelib]

            # A bunch of unused fields.
            [target.'cfg(not(target_os = "none"))']
            linker = 'false'
            ar = 'false'
            foo = {rustc-flags = "-l foo"}
            invalid = 1
            runner = 'false'
            rustflags = ''
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check")
        .with_stderr(
            "\
[WARNING] unused key `somelib` in [target] config table `cfg(not(bar))`
[WARNING] unused key `ar` in [target] config table `cfg(not(target_os = \"none\"))`
[WARNING] unused key `foo` in [target] config table `cfg(not(target_os = \"none\"))`
[WARNING] unused key `invalid` in [target] config table `cfg(not(target_os = \"none\"))`
[WARNING] unused key `linker` in [target] config table `cfg(not(target_os = \"none\"))`
[CHECKING] foo v0.0.1 ([..])
[FINISHED] [..]
",
        )
        .run();
}
