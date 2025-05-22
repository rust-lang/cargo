//! Tests for configuration values that point to programs.

use cargo_test_support::prelude::*;
use cargo_test_support::{basic_lib_manifest, project, rustc_host, rustc_host_env, str};

#[cargo_test]
fn pathless_tools() {
    let target = rustc_host();

    let foo = project()
        .file("Cargo.toml", &basic_lib_manifest("foo"))
        .file("src/lib.rs", "")
        .file(
            ".cargo/config.toml",
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
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.5.0 ([ROOT]/foo)
[RUNNING] `rustc [..]-C linker=nonexistent-linker [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

// can set a custom linker via `target.'cfg(..)'.linker`
#[cargo_test]
fn custom_linker_cfg() {
    let foo = project()
        .file("Cargo.toml", &basic_lib_manifest("foo"))
        .file("src/lib.rs", "")
        .file(
            ".cargo/config.toml",
            r#"
            [target.'cfg(not(target_os = "none"))']
            linker = "nonexistent-linker"
            "#,
        )
        .build();

    foo.cargo("build --verbose")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.5.0 ([ROOT]/foo)
[RUNNING] `rustc [..]-C linker=nonexistent-linker [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

// custom linker set via `target.$triple.linker` have precede over `target.'cfg(..)'.linker`
#[cargo_test]
fn custom_linker_cfg_precedence() {
    let target = rustc_host();

    let foo = project()
        .file("Cargo.toml", &basic_lib_manifest("foo"))
        .file("src/lib.rs", "")
        .file(
            ".cargo/config.toml",
            &format!(
                r#"
                    [target.'cfg(not(target_os = "none"))']
                    linker = "ignored-linker"
                    [target.{}]
                    linker = "nonexistent-linker"
                "#,
                target
            ),
        )
        .build();

    foo.cargo("build --verbose")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.5.0 ([ROOT]/foo)
[RUNNING] `rustc [..]-C linker=nonexistent-linker [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn custom_linker_cfg_collision() {
    let foo = project()
        .file("Cargo.toml", &basic_lib_manifest("foo"))
        .file("src/lib.rs", "")
        .file(
            ".cargo/config.toml",
            r#"
            [target.'cfg(not(target_arch = "avr"))']
            linker = "nonexistent-linker1"
            [target.'cfg(not(target_os = "none"))']
            linker = "nonexistent-linker2"
            "#,
        )
        .build();

    foo.cargo("build --verbose")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] several matching instances of `target.'cfg(..)'.linker` in configurations
first match `cfg(not(target_arch = "avr"))` located in [ROOT]/foo/.cargo/config.toml
second match `cfg(not(target_os = "none"))` located in [ROOT]/foo/.cargo/config.toml

"#]])
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
            ".cargo/config.toml",
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
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.5.0 ([ROOT]/foo)
[RUNNING] `rustc [..]-C linker=[..]/bogus/nonexistent-linker [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
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
            ".cargo/config.toml",
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

    p.cargo("build --verbose")
        .cwd("bar")
        .with_stderr_data(str![[r#"
[COMPILING] bar v0.5.0 ([ROOT]/foo/bar)
[RUNNING] `rustc [..]-C linker=[ROOT]/foo/./tools/nonexistent-linker [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
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
            ".cargo/config.toml",
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
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `nonexistent-runner -r target/debug/foo[EXE] --param`
...
"#]])
        .run();

    p.cargo("test --test test --verbose -- --param")
        .with_status(101)
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc [..]`
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `nonexistent-runner -r [ROOT]/foo/target/debug/deps/test-[HASH][EXE] --param`
...
"#]])
        .run();

    p.cargo("bench --bench bench --verbose -- --param")
        .with_status(101)
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc [..]`
[RUNNING] `rustc [..]`
[FINISHED] `bench` profile [optimized] target(s) in [ELAPSED]s
[RUNNING] `nonexistent-runner -r [ROOT]/foo/target/release/deps/bench-[HASH][EXE] --param --bench`
...
"#]])
        .run();
}

// can set a custom runner via `target.'cfg(..)'.runner`
#[cargo_test]
fn custom_runner_cfg() {
    let p = project()
        .file("src/main.rs", "fn main() {}")
        .file(
            ".cargo/config.toml",
            r#"
            [target.'cfg(not(target_os = "none"))']
            runner = "nonexistent-runner -r"
            "#,
        )
        .build();

    p.cargo("run -- --param")
        .with_status(101)
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `nonexistent-runner -r target/debug/foo[EXE] --param`
...
"#]])
        .run();
}

// custom runner set via `target.$triple.runner` have precedence over `target.'cfg(..)'.runner`
#[cargo_test]
fn custom_runner_cfg_precedence() {
    let target = rustc_host();

    let p = project()
        .file("src/main.rs", "fn main() {}")
        .file(
            ".cargo/config.toml",
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
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `nonexistent-runner -r target/debug/foo[EXE] --param`
...
"#]])
        .run();
}

#[cargo_test]
fn custom_runner_cfg_collision() {
    let p = project()
        .file("src/main.rs", "fn main() {}")
        .file(
            ".cargo/config.toml",
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
        .with_stderr_data(str![[r#"
[ERROR] several matching instances of `target.'cfg(..)'.runner` in configurations
first match `cfg(not(target_arch = "avr"))` located in [ROOT]/foo/.cargo/config.toml
second match `cfg(not(target_os = "none"))` located in [ROOT]/foo/.cargo/config.toml

"#]])
        .run();
}

#[cargo_test]
fn custom_runner_env() {
    let p = project().file("src/main.rs", "fn main() {}").build();

    let key = format!("CARGO_TARGET_{}_RUNNER", rustc_host_env());

    p.cargo("run")
        .env(&key, "nonexistent-runner --foo")
        .with_status(101)
        // FIXME: Update "Caused by" error message once rust/pull/87704 is merged.
        // On Windows, changing to a custom executable resolver has changed the
        // error messages.
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `nonexistent-runner --foo target/debug/foo[EXE]`
[ERROR] could not execute process `nonexistent-runner --foo target/debug/foo[EXE]` (never executed)

Caused by:
  [NOT_FOUND]

"#]])
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
        .with_stderr_data(str![[r#"
...
[RUNNING] `should-run --foo target/debug/foo[EXE]`
...
"#]])
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
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `true target/debug/foo[EXE]`

"#]])
        .run();
}

#[cargo_test]
fn custom_linker_env() {
    let p = project().file("src/main.rs", "fn main() {}").build();

    let key = format!("CARGO_TARGET_{}_LINKER", rustc_host_env());

    p.cargo("build -v")
        .env(&key, "nonexistent-linker")
        .with_status(101)
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc [..]-C linker=nonexistent-linker [..]`
...
"#]])
        .run();
}

#[cargo_test]
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
        .with_stderr_data(format!("\
[WARNING] environment variables are expected to use uppercase letters and underscores, the variable `{env_key}` will be ignored and have no effect
[WARNING] environment variables are expected to use uppercase letters and underscores, the variable `{env_key}` will be ignored and have no effect
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo --edition=2015 src/main.rs [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
"
        ))
        .run();
}

#[cargo_test]
fn cfg_ignored_fields() {
    // Test for some ignored fields in [target.'cfg()'] tables.
    let p = project()
        .file(
            ".cargo/config.toml",
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
        .with_stderr_data(str![[r#"
[WARNING] unused key `somelib` in [target] config table `cfg(not(bar))`
[WARNING] unused key `ar` in [target] config table `cfg(not(target_os = "none"))`
[WARNING] unused key `foo` in [target] config table `cfg(not(target_os = "none"))`
[WARNING] unused key `invalid` in [target] config table `cfg(not(target_os = "none"))`
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}
