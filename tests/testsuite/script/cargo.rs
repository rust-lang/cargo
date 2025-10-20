use std::fs;

use crate::prelude::*;
use cargo_test_support::basic_manifest;
use cargo_test_support::paths::cargo_home;
use cargo_test_support::registry::Package;
use cargo_test_support::str;

const ECHO_SCRIPT: &str = r#"#!/usr/bin/env cargo

fn main() {
    let current_exe = std::env::current_exe().unwrap().to_str().unwrap().to_owned();
    let mut args = std::env::args_os();
    let arg0 = args.next().unwrap().to_str().unwrap().to_owned();
    let args = args.collect::<Vec<_>>();
    println!("current_exe: {current_exe}");
    println!("arg0: {arg0}");
    println!("args: {args:?}");
}

#[test]
fn test () {}
"#;

#[cfg(unix)]
fn path() -> Vec<std::path::PathBuf> {
    std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()).collect()
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
fn basic_rs() {
    let p = cargo_test_support::project()
        .file("echo.rs", ECHO_SCRIPT)
        .build();

    p.cargo("-Zscript -v echo.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout_data(str![[r#"
current_exe: [ROOT]/home/.cargo/build/[HASH]/target/debug/echo[EXE]
arg0: [..]
args: []

"#]])
        .with_stderr_data(str![[r#"
[WARNING] `package.edition` is unspecified, defaulting to `2024`
[COMPILING] echo v0.0.0 ([ROOT]/foo/echo.rs)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `[ROOT]/home/.cargo/build/[HASH]/target/debug/echo[EXE]`

"#]])
        .run();
}

#[cfg(unix)]
#[cargo_test(nightly, reason = "-Zscript is unstable")]
fn arg0() {
    let p = cargo_test_support::project()
        .file("echo.rs", ECHO_SCRIPT)
        .build();

    p.cargo("-Zscript -v echo.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout_data(str![[r#"
current_exe: [ROOT]/home/.cargo/build/[HASH]/target/debug/echo[EXE]
arg0: [ROOT]/foo/echo.rs
args: []

"#]])
        .with_stderr_data(str![[r#"
[WARNING] `package.edition` is unspecified, defaulting to `2024`
[COMPILING] echo v0.0.0 ([ROOT]/foo/echo.rs)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `[ROOT]/home/.cargo/build/[HASH]/target/debug/echo[EXE]`

"#]])
        .run();
}

#[cfg(windows)]
#[cargo_test(nightly, reason = "-Zscript is unstable")]
fn arg0() {
    let p = cargo_test_support::project()
        .file("echo.rs", ECHO_SCRIPT)
        .build();

    p.cargo("-Zscript -v echo.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout_data(str![[r#"
current_exe: [ROOT]/home/.cargo/build/[HASH]/target/debug/echo[EXE]
arg0: [ROOT]/home/.cargo/build/[HASH]/target/debug/echo[EXE]
args: []

"#]])
        .with_stderr_data(str![[r#"
[WARNING] `package.edition` is unspecified, defaulting to `2024`
[COMPILING] echo v0.0.0 ([ROOT]/foo/echo.rs)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `[ROOT]/home/.cargo/build/[HASH]/target/debug/echo[EXE]`

"#]])
        .run();
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
fn basic_path() {
    let p = cargo_test_support::project()
        .file("echo", ECHO_SCRIPT)
        .build();

    p.cargo("-Zscript -v ./echo")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout_data(str![[r#"
current_exe: [ROOT]/home/.cargo/build/[HASH]/target/debug/echo[EXE]
arg0: [..]
args: []

"#]])
        .with_stderr_data(str![[r#"
[WARNING] `package.edition` is unspecified, defaulting to `2024`
[COMPILING] echo v0.0.0 ([ROOT]/foo/echo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `[ROOT]/home/.cargo/build/[HASH]/target/debug/echo[EXE]`

"#]])
        .run();
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
fn path_required() {
    let p = cargo_test_support::project()
        .file("echo", ECHO_SCRIPT)
        .build();

    p.cargo("-Zscript -v echo")
        .masquerade_as_nightly_cargo(&["script"])
        .with_status(101)
        .with_stdout_data("")
        .with_stderr_data(str![[r#"
[ERROR] no such command: `echo`

[HELP] a command with a similar name exists: `bench`

[HELP] view all installed commands with `cargo --list`
[HELP] find a package to install `echo` with `cargo search cargo-echo`
[HELP] To run the file `echo`, provide a relative path like `./echo`

"#]])
        .run();
}

#[cfg(unix)]
#[cargo_test(nightly, reason = "-Zscript is unstable")]
fn manifest_precedence_over_plugins() {
    let p = cargo_test_support::project()
        .file("echo.rs", ECHO_SCRIPT)
        .executable(std::path::Path::new("path-test").join("cargo-echo.rs"), "")
        .build();

    // With path - fmt is there with known description
    let mut path = path();
    path.push(p.root().join("path-test"));
    let path = std::env::join_paths(path.iter()).unwrap();

    p.cargo("-Zscript -v echo.rs")
        .env("PATH", &path)
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout_data(str![[r#"
current_exe: [ROOT]/home/.cargo/build/[HASH]/target/debug/echo[EXE]
arg0: [..]
args: []

"#]])
        .with_stderr_data(str![[r#"
[WARNING] `package.edition` is unspecified, defaulting to `2024`
[COMPILING] echo v0.0.0 ([ROOT]/foo/echo.rs)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `[ROOT]/home/.cargo/build/[HASH]/target/debug/echo[EXE]`

"#]])
        .run();
}

#[cfg(unix)]
#[cargo_test(nightly, reason = "-Zscript is unstable")]
fn warn_when_plugin_masks_manifest_on_stable() {
    let p = cargo_test_support::project()
        .file("echo.rs", ECHO_SCRIPT)
        .executable(std::path::Path::new("path-test").join("cargo-echo.rs"), "")
        .build();

    let mut path = path();
    path.push(p.root().join("path-test"));
    let path = std::env::join_paths(path.iter()).unwrap();

    p.cargo("-v echo.rs")
        .env("PATH", &path)
        .with_stdout_data("")
        .with_stderr_data(str![[r#"
[WARNING] external subcommand `echo.rs` has the appearance of a manifest-command
  |
  = [NOTE] this was previously accepted but will be phased out when `-Zscript` is stabilized; see <https://github.com/rust-lang/cargo/issues/12207>

"#]])
        .run();
}

#[cargo_test]
fn requires_nightly() {
    let p = cargo_test_support::project()
        .file("echo.rs", ECHO_SCRIPT)
        .build();

    p.cargo("-v echo.rs")
        .with_status(101)
        .with_stdout_data("")
        .with_stderr_data(str![[r#"
[ERROR] running the file `echo.rs` requires `-Zscript`

"#]])
        .run();
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
fn requires_z_flag() {
    let p = cargo_test_support::project()
        .file("echo.rs", ECHO_SCRIPT)
        .build();

    p.cargo("-v echo.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_status(101)
        .with_stdout_data("")
        .with_stderr_data(str![[r#"
[ERROR] running the file `echo.rs` requires `-Zscript`

"#]])
        .run();
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
fn manifest_parse_error() {
    // Exagerate the newlines to make it more obvious if the error's line number is off
    let script = r#"#!/usr/bin/env cargo





---
[dependencies]
bar = 3
---

fn main() {
    println!("Hello world!");
}"#;
    let p = cargo_test_support::project()
        .file("script.rs", script)
        .build();

    p.cargo("-Zscript -v script.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_status(101)
        .with_stdout_data(str![""])
        .with_stderr_data(str![[r#"
[ERROR] invalid type: integer `3`, expected a version string like "0.9.8" or a detailed dependency like { version = "0.9.8" }
 --> script.rs:9:7
  |
9 | bar = 3
  |       ^

"#]])
        .run();
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
fn clean_output_with_edition() {
    let script = r#"#!/usr/bin/env cargo
---
[package]
edition = "2018"
---

fn main() {
    println!("Hello world!");
}"#;
    let p = cargo_test_support::project()
        .file("script.rs", script)
        .build();

    p.cargo("-Zscript -v script.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout_data(str![[r#"
Hello world!

"#]])
        .with_stderr_data(str![[r#"
[COMPILING] script v0.0.0 ([ROOT]/foo/script.rs)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `[ROOT]/home/.cargo/build/[HASH]/target/debug/script[EXE]`

"#]])
        .run();
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
fn warning_without_edition() {
    let script = r#"#!/usr/bin/env cargo
---
[package]
---

fn main() {
    println!("Hello world!");
}"#;
    let p = cargo_test_support::project()
        .file("script.rs", script)
        .build();

    p.cargo("-Zscript -v script.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout_data(str![[r#"
Hello world!

"#]])
        .with_stderr_data(str![[r#"
[WARNING] `package.edition` is unspecified, defaulting to `2024`
[COMPILING] script v0.0.0 ([ROOT]/foo/script.rs)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `[ROOT]/home/.cargo/build/[HASH]/target/debug/script[EXE]`

"#]])
        .run();
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
fn rebuild() {
    let script = r#"#!/usr/bin/env cargo-eval

fn main() {
    let msg = option_env!("_MESSAGE").unwrap_or("undefined");
    println!("msg = {}", msg);
}"#;
    let p = cargo_test_support::project()
        .file("script.rs", script)
        .build();

    p.cargo("-Zscript -v script.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout_data(str![[r#"
msg = undefined

"#]])
        .with_stderr_data(str![[r#"
[WARNING] `package.edition` is unspecified, defaulting to `2024`
[COMPILING] script v0.0.0 ([ROOT]/foo/script.rs)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `[ROOT]/home/.cargo/build/[HASH]/target/debug/script[EXE]`

"#]])
        .run();

    // Verify we don't rebuild
    p.cargo("-Zscript -v script.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout_data(str![[r#"
msg = undefined

"#]])
        .with_stderr_data(str![[r#"
[WARNING] `package.edition` is unspecified, defaulting to `2024`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `[ROOT]/home/.cargo/build/[HASH]/target/debug/script[EXE]`

"#]])
        .run();

    // Verify we do rebuild
    p.cargo("-Zscript -v script.rs")
        .env("_MESSAGE", "hello")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout_data(str![[r#"
msg = hello

"#]])
        .with_stderr_data(str![[r#"
[WARNING] `package.edition` is unspecified, defaulting to `2024`
[COMPILING] script v0.0.0 ([ROOT]/foo/script.rs)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `[ROOT]/home/.cargo/build/[HASH]/target/debug/script[EXE]`

"#]])
        .run();
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
fn use_cargo_home_config() {
    let script = ECHO_SCRIPT;
    let _ = cargo_test_support::project()
        .at("script")
        .file("script.rs", script)
        .build();

    let p = cargo_test_support::project()
        .file(
            ".cargo/config.toml",
            r#"
[build]
rustc = "non-existent-rustc"
"#,
        )
        .file("script.rs", script)
        .build();

    // Verify that the config from the current directory is used
    p.cargo("-Zscript script.rs -NotAnArg")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout_data(str![[r#"
current_exe: [ROOT]/home/.cargo/build/[HASH]/target/debug/script[EXE]
arg0: [..]
args: ["-NotAnArg"]

"#]])
        .run();

    // Verify that the config from the parent directory is not used
    p.cargo("-Zscript ../script/script.rs -NotAnArg")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout_data(str![[r#"
current_exe: [ROOT]/home/.cargo/build/[HASH]/target/debug/script[EXE]
arg0: [..]
args: ["-NotAnArg"]

"#]])
        .run();

    // Write a global config.toml in the cargo home directory
    let cargo_home = cargo_home();
    fs::write(
        &cargo_home.join("config.toml"),
        r#"
[build]
rustc = "non-existent-rustc"
"#,
    )
    .unwrap();

    // Verify the global config is used
    p.cargo("-Zscript script.rs -NotAnArg")
        .masquerade_as_nightly_cargo(&["script"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] could not execute process `non-existent-rustc -vV` (never executed)

Caused by:
  [NOT_FOUND]

"#]])
        .run();
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
fn default_programmatic_verbosity() {
    let script = ECHO_SCRIPT;
    let p = cargo_test_support::project()
        .file("script.rs", script)
        .build();

    p.cargo("-Zscript script.rs -NotAnArg")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout_data(str![[r#"
current_exe: [ROOT]/home/.cargo/build/[HASH]/target/debug/script[EXE]
arg0: [..]
args: ["-NotAnArg"]

"#]])
        .with_stderr_data("")
        .run();
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
fn quiet() {
    let script = ECHO_SCRIPT;
    let p = cargo_test_support::project()
        .file("script.rs", script)
        .build();

    p.cargo("-Zscript -q script.rs -NotAnArg")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout_data(str![[r#"
current_exe: [ROOT]/home/.cargo/build/[HASH]/target/debug/script[EXE]
arg0: [..]
args: ["-NotAnArg"]

"#]])
        .with_stderr_data("")
        .run();
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
fn test_line_numbering_preserved() {
    let script = r#"#!/usr/bin/env cargo

fn main() {
    println!("line: {}", line!());
}
"#;
    let p = cargo_test_support::project()
        .file("script.rs", script)
        .build();

    p.cargo("-Zscript -v script.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout_data(str![[r#"
line: 4

"#]])
        .with_stderr_data(str![[r#"
[WARNING] `package.edition` is unspecified, defaulting to `2024`
[COMPILING] script v0.0.0 ([ROOT]/foo/script.rs)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `[ROOT]/home/.cargo/build/[HASH]/target/debug/script[EXE]`

"#]])
        .run();
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
fn test_escaped_hyphen_arg() {
    let script = ECHO_SCRIPT;
    let p = cargo_test_support::project()
        .file("script.rs", script)
        .build();

    p.cargo("-Zscript -v -- script.rs -NotAnArg")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout_data(str![[r#"
current_exe: [ROOT]/home/.cargo/build/[HASH]/target/debug/script[EXE]
arg0: [..]
args: ["-NotAnArg"]

"#]])
        .with_stderr_data(str![[r#"
[WARNING] `package.edition` is unspecified, defaulting to `2024`
[COMPILING] script v0.0.0 ([ROOT]/foo/script.rs)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `[ROOT]/home/.cargo/build/[HASH]/target/debug/script[EXE] -NotAnArg`

"#]])
        .run();
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
fn test_unescaped_hyphen_arg() {
    let script = ECHO_SCRIPT;
    let p = cargo_test_support::project()
        .file("script.rs", script)
        .build();

    p.cargo("-Zscript -v script.rs -NotAnArg")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout_data(str![[r#"
current_exe: [ROOT]/home/.cargo/build/[HASH]/target/debug/script[EXE]
arg0: [..]
args: ["-NotAnArg"]

"#]])
        .with_stderr_data(str![[r#"
[WARNING] `package.edition` is unspecified, defaulting to `2024`
[COMPILING] script v0.0.0 ([ROOT]/foo/script.rs)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `[ROOT]/home/.cargo/build/[HASH]/target/debug/script[EXE] -NotAnArg`

"#]])
        .run();
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
fn test_same_flags() {
    let script = ECHO_SCRIPT;
    let p = cargo_test_support::project()
        .file("script.rs", script)
        .build();

    p.cargo("-Zscript -v script.rs --help")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout_data(str![[r#"
current_exe: [ROOT]/home/.cargo/build/[HASH]/target/debug/script[EXE]
arg0: [..]
args: ["--help"]

"#]])
        .with_stderr_data(str![[r#"
[WARNING] `package.edition` is unspecified, defaulting to `2024`
[COMPILING] script v0.0.0 ([ROOT]/foo/script.rs)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `[ROOT]/home/.cargo/build/[HASH]/target/debug/script[EXE] --help`

"#]])
        .run();
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
fn test_name_has_weird_chars() {
    let script = ECHO_SCRIPT;
    let p = cargo_test_support::project()
        .file("s-h.w§c!.rs", script)
        .build();

    p.cargo("-Zscript -v s-h.w§c!.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout_data(str![[r#"
current_exe: [ROOT]/home/.cargo/build/[HASH]/target/debug/s-h-w-c-[EXE]
arg0: [..]
args: []

"#]])
        .with_stderr_data(str![[r#"
[WARNING] `package.edition` is unspecified, defaulting to `2024`
[COMPILING] s-h-w-c- v0.0.0 ([ROOT]/foo/s-h.w§c!.rs)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `[ROOT]/home/.cargo/build/[HASH]/target/debug/s-h-w-c-[EXE]`

"#]])
        .run();
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
fn test_name_has_leading_number() {
    let script = ECHO_SCRIPT;
    let p = cargo_test_support::project()
        .file("42answer.rs", script)
        .build();

    p.cargo("-Zscript -v 42answer.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout_data(str![[r#"
current_exe: [ROOT]/home/.cargo/build/[HASH]/target/debug/answer[EXE]
arg0: [..]
args: []

"#]])
        .with_stderr_data(str![[r#"
[WARNING] `package.edition` is unspecified, defaulting to `2024`
[COMPILING] answer v0.0.0 ([ROOT]/foo/42answer.rs)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `[ROOT]/home/.cargo/build/[HASH]/target/debug/answer[EXE]`

"#]])
        .run();
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
fn test_name_is_number() {
    let script = ECHO_SCRIPT;
    let p = cargo_test_support::project().file("42.rs", script).build();

    p.cargo("-Zscript -v 42.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout_data(str![[r#"
current_exe: [ROOT]/home/.cargo/build/[HASH]/target/debug/package[EXE]
arg0: [..]
args: []

"#]])
        .with_stderr_data(str![[r#"
[WARNING] `package.edition` is unspecified, defaulting to `2024`
[COMPILING] package v0.0.0 ([ROOT]/foo/42.rs)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `[ROOT]/home/.cargo/build/[HASH]/target/debug/package[EXE]`

"#]])
        .run();
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
#[cfg(not(windows))]
fn test_name_is_windows_reserved_name() {
    let script = ECHO_SCRIPT;
    let p = cargo_test_support::project().file("con", script).build();

    p.cargo("-Zscript -v ./con")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout_data(str![[r#"
current_exe: [ROOT]/home/.cargo/build/[HASH]/target/debug/con[EXE]
arg0: [..]
args: []

"#]])
        .with_stderr_data(str![[r#"
[WARNING] `package.edition` is unspecified, defaulting to `2024`
[COMPILING] con v0.0.0 ([ROOT]/foo/con)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `[ROOT]/home/.cargo/build/[HASH]/target/debug/con[EXE]`

"#]])
        .run();
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
fn test_name_is_sysroot_package_name() {
    let script = ECHO_SCRIPT;
    let p = cargo_test_support::project().file("test", script).build();

    p.cargo("-Zscript -v ./test")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout_data(str![[r#"
current_exe: [ROOT]/home/.cargo/build/[HASH]/target/debug/test[EXE]
arg0: [..]
args: []

"#]])
        .with_stderr_data(str![[r#"
[WARNING] `package.edition` is unspecified, defaulting to `2024`
[COMPILING] test v0.0.0 ([ROOT]/foo/test)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `[ROOT]/home/.cargo/build/[HASH]/target/debug/test[EXE]`

"#]])
        .run();
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
fn test_name_is_keyword() {
    let script = ECHO_SCRIPT;
    let p = cargo_test_support::project().file("self", script).build();

    p.cargo("-Zscript -v ./self")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout_data(str![[r#"
current_exe: [ROOT]/home/.cargo/build/[HASH]/target/debug/self[EXE]
arg0: [..]
args: []

"#]])
        .with_stderr_data(str![[r#"
[WARNING] `package.edition` is unspecified, defaulting to `2024`
[COMPILING] self v0.0.0 ([ROOT]/foo/self)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `[ROOT]/home/.cargo/build/[HASH]/target/debug/self[EXE]`

"#]])
        .run();
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
fn test_name_is_deps_dir_implicit() {
    let script = ECHO_SCRIPT;
    let p = cargo_test_support::project()
        .file("deps.rs", script)
        .build();

    p.cargo("-Zscript -v deps.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_status(101)
        .with_stdout_data(str![""])
        .with_stderr_data(str![[r#"
[WARNING] `package.edition` is unspecified, defaulting to `2024`
[ERROR] failed to parse manifest at `[ROOT]/foo/deps.rs`

Caused by:
  the binary target name `deps` is forbidden, it conflicts with cargo's build directory names

"#]])
        .run();
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
fn test_name_is_deps_dir_explicit() {
    let script = r#"#!/usr/bin/env cargo
---
package.name = "deps"
---

fn main() {
    let current_exe = std::env::current_exe().unwrap().to_str().unwrap().to_owned();
    let mut args = std::env::args_os();
    let arg0 = args.next().unwrap().to_str().unwrap().to_owned();
    let args = args.collect::<Vec<_>>();
    println!("current_exe: {current_exe}");
    println!("arg0: {arg0}");
    println!("args: {args:?}");
}

#[test]
fn test () {}
"#;
    let p = cargo_test_support::project()
        .file("deps.rs", script)
        .build();

    p.cargo("-Zscript -v deps.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_status(101)
        .with_stdout_data(str![""])
        .with_stderr_data(str![[r#"
[WARNING] `package.edition` is unspecified, defaulting to `2024`
[ERROR] failed to parse manifest at `[ROOT]/foo/deps.rs`

Caused by:
  the binary target name `deps` is forbidden, it conflicts with cargo's build directory names

"#]])
        .run();
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
fn script_like_dir() {
    let p = cargo_test_support::project()
        .file("foo.rs/foo", "something")
        .build();

    p.cargo("-Zscript -v foo.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] no such file or subcommand `foo.rs`: `foo.rs` is a directory

"#]])
        .run();
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
fn non_existent_rs() {
    let p = cargo_test_support::project().build();

    p.cargo("-Zscript -v foo.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] no such file or subcommand `foo.rs`

"#]])
        .run();
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
fn non_existent_rs_stable() {
    let p = cargo_test_support::project().build();

    p.cargo("-v foo.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_status(101)
        .with_stdout_data("")
        .with_stderr_data(str![[r#"
[ERROR] no such subcommand `foo.rs`

"#]])
        .run();
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
fn did_you_mean_file() {
    let p = cargo_test_support::project()
        .file("food.rs", ECHO_SCRIPT)
        .build();

    p.cargo("-Zscript -v foo.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_status(101)
        .with_stdout_data("")
        .with_stderr_data(str![[r#"
[ERROR] no such file or subcommand `foo.rs`
[HELP] there is a script with a similar name: `./food.rs`

"#]])
        .run();
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
fn did_you_mean_file_stable() {
    let p = cargo_test_support::project()
        .file("food.rs", ECHO_SCRIPT)
        .build();

    p.cargo("-v foo.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_status(101)
        .with_stdout_data("")
        .with_stderr_data(str![[r#"
[ERROR] no such subcommand `foo.rs`
[HELP] there is a script with a similar name: `./food.rs` (requires `-Zscript`)

"#]])
        .run();
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
fn did_you_mean_command() {
    let p = cargo_test_support::project().build();

    p.cargo("-Zscript -v build--manifest-path=./Cargo.toml")
        .masquerade_as_nightly_cargo(&["script"])
        .with_status(101)
        .with_stdout_data("")
        .with_stderr_data(str![[r#"
[ERROR] no such file or subcommand `build--manifest-path=./Cargo.toml`
[HELP] there is a command with a similar name: `build --manifest-path=./Cargo.toml`

"#]])
        .run();
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
fn did_you_mean_command_stable() {
    let p = cargo_test_support::project().build();

    p.cargo("-v build--manifest-path=./Cargo.toml")
        .masquerade_as_nightly_cargo(&["script"])
        .with_status(101)
        .with_stdout_data("")
        .with_stderr_data(str![[r#"
[ERROR] no such subcommand `build--manifest-path=./Cargo.toml`
[HELP] there is a command with a similar name: `build --manifest-path=./Cargo.toml`

"#]])
        .run();
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
fn test_name_same_as_dependency() {
    Package::new("script", "1.0.0").publish();
    let script = r#"#!/usr/bin/env cargo
---
[dependencies]
script = "1.0.0"
---

fn main() {
    println!("Hello world!");
}"#;
    let p = cargo_test_support::project()
        .file("script.rs", script)
        .build();

    p.cargo("-Zscript -v script.rs --help")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout_data(str![[r#"
Hello world!

"#]])
        .with_stderr_data(str![[r#"
[WARNING] `package.edition` is unspecified, defaulting to `2024`
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest Rust [..] compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] script v1.0.0 (registry `dummy-registry`)
[COMPILING] script v1.0.0
[COMPILING] script v0.0.0 ([ROOT]/foo/script.rs)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `[ROOT]/home/.cargo/build/[HASH]/target/debug/script[EXE] --help`

"#]])
        .run();
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
fn test_path_dep() {
    let script = r#"#!/usr/bin/env cargo
---
[dependencies]
bar.path = "./bar"
---

fn main() {
    println!("Hello world!");
}"#;
    let p = cargo_test_support::project()
        .file("script.rs", script)
        .file("src/lib.rs", "pub fn foo() {}")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file("bar/src/lib.rs", "pub fn bar() {}")
        .build();

    p.cargo("-Zscript -v script.rs --help")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout_data(str![[r#"
Hello world!

"#]])
        .with_stderr_data(str![[r#"
[WARNING] `package.edition` is unspecified, defaulting to `2024`
[LOCKING] 1 package to latest Rust [..] compatible version
[COMPILING] bar v0.0.1 ([ROOT]/foo/bar)
[COMPILING] script v0.0.0 ([ROOT]/foo/script.rs)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `[ROOT]/home/.cargo/build/[HASH]/target/debug/script[EXE] --help`

"#]])
        .run();
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
fn test_no_build_rs() {
    let script = r#"#!/usr/bin/env cargo

fn main() {
    println!("Hello world!");
}"#;
    let p = cargo_test_support::project()
        .file("script.rs", script)
        .file("build.rs", "broken")
        .build();

    p.cargo("-Zscript -v script.rs --help")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout_data(str![[r#"
Hello world!

"#]])
        .with_stderr_data(str![[r#"
[WARNING] `package.edition` is unspecified, defaulting to `2024`
[COMPILING] script v0.0.0 ([ROOT]/foo/script.rs)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `[ROOT]/home/.cargo/build/[HASH]/target/debug/script[EXE] --help`

"#]])
        .run();
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
fn test_no_autobins() {
    let script = r#"#!/usr/bin/env cargo

fn main() {
    println!("Hello world!");
}"#;
    let p = cargo_test_support::project()
        .file("script.rs", script)
        .file("src/bin/not-script/main.rs", "fn main() {}")
        .build();

    p.cargo("-Zscript -v script.rs --help")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout_data(str![[r#"
Hello world!

"#]])
        .with_stderr_data(str![[r#"
[WARNING] `package.edition` is unspecified, defaulting to `2024`
[COMPILING] script v0.0.0 ([ROOT]/foo/script.rs)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `[ROOT]/home/.cargo/build/[HASH]/target/debug/script[EXE] --help`

"#]])
        .run();
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
fn test_no_autolib() {
    let script = r#"#!/usr/bin/env cargo

fn main() {
    println!("Hello world!");
}"#;
    let p = cargo_test_support::project()
        .file("script.rs", script)
        .file("src/lib.rs", r#"compile_error!{"must not be built"}"#)
        .build();

    p.cargo("-Zscript -v script.rs --help")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout_data(str![[r#"
Hello world!

"#]])
        .with_stderr_data(str![[r#"
[WARNING] `package.edition` is unspecified, defaulting to `2024`
[COMPILING] script v0.0.0 ([ROOT]/foo/script.rs)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `[ROOT]/home/.cargo/build/[HASH]/target/debug/script[EXE] --help`

"#]])
        .run();
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
fn disallow_explicit_workspace() {
    let p = cargo_test_support::project()
        .file(
            "script.rs",
            r#"
----
package.edition = "2021"

[workspace]
----

fn main() {}
"#,
        )
        .build();

    p.cargo("-Zscript -v script.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/script.rs`

Caused by:
  `workspace` is not allowed in embedded manifests

"#]])
        .run();
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
fn disallow_explicit_lib() {
    let p = cargo_test_support::project()
        .file(
            "script.rs",
            r#"
----
package.edition = "2021"

[lib]
name = "script"
path = "script.rs"
----

fn main() {}
"#,
        )
        .build();

    p.cargo("-Zscript -v script.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/script.rs`

Caused by:
  `lib` is not allowed in embedded manifests

"#]])
        .run();
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
fn disallow_explicit_bin() {
    let p = cargo_test_support::project()
        .file(
            "script.rs",
            r#"
----
package.edition = "2021"

[[bin]]
name = "script"
path = "script.rs"
----

fn main() {}
"#,
        )
        .build();

    p.cargo("-Zscript -v script.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/script.rs`

Caused by:
  `bin` is not allowed in embedded manifests

"#]])
        .run();
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
fn disallow_explicit_example() {
    let p = cargo_test_support::project()
        .file(
            "script.rs",
            r#"
----
package.edition = "2021"

[[example]]
name = "script"
path = "script.rs"
----

fn main() {}
"#,
        )
        .build();

    p.cargo("-Zscript -v script.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/script.rs`

Caused by:
  `example` is not allowed in embedded manifests

"#]])
        .run();
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
fn disallow_explicit_test() {
    let p = cargo_test_support::project()
        .file(
            "script.rs",
            r#"
----
package.edition = "2021"

[[test]]
name = "script"
path = "script.rs"
----

fn main() {}
"#,
        )
        .build();

    p.cargo("-Zscript -v script.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/script.rs`

Caused by:
  `test` is not allowed in embedded manifests

"#]])
        .run();
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
fn disallow_explicit_bench() {
    let p = cargo_test_support::project()
        .file(
            "script.rs",
            r#"
----
package.edition = "2021"

[[bench]]
name = "script"
path = "script.rs"
----

fn main() {}
"#,
        )
        .build();

    p.cargo("-Zscript -v script.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/script.rs`

Caused by:
  `bench` is not allowed in embedded manifests

"#]])
        .run();
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
fn disallow_explicit_package_build() {
    let p = cargo_test_support::project()
        .file(
            "script.rs",
            r#"
----
[package]
edition = "2021"
build = "script.rs"
----

fn main() {}
"#,
        )
        .build();

    p.cargo("-Zscript -v script.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/script.rs`

Caused by:
  `package.build` is not allowed in embedded manifests

"#]])
        .run();
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
fn disallow_explicit_package_links() {
    let p = cargo_test_support::project()
        .file(
            "script.rs",
            r#"
----
[package]
edition = "2021"
links = "script"
----

fn main() {}
"#,
        )
        .build();

    p.cargo("-Zscript -v script.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/script.rs`

Caused by:
  `package.links` is not allowed in embedded manifests

"#]])
        .run();
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
fn disallow_explicit_package_autolib() {
    let p = cargo_test_support::project()
        .file(
            "script.rs",
            r#"
----
[package]
edition = "2021"
autolib = true
----

fn main() {}
"#,
        )
        .build();

    p.cargo("-Zscript -v script.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/script.rs`

Caused by:
  `package.autolib` is not allowed in embedded manifests

"#]])
        .run();
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
fn disallow_explicit_package_autobins() {
    let p = cargo_test_support::project()
        .file(
            "script.rs",
            r#"
----
[package]
edition = "2021"
autobins = true
----

fn main() {}
"#,
        )
        .build();

    p.cargo("-Zscript -v script.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/script.rs`

Caused by:
  `package.autobins` is not allowed in embedded manifests

"#]])
        .run();
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
fn disallow_explicit_package_autoexamples() {
    let p = cargo_test_support::project()
        .file(
            "script.rs",
            r#"
----
[package]
edition = "2021"
autoexamples = true
----

fn main() {}
"#,
        )
        .build();

    p.cargo("-Zscript -v script.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/script.rs`

Caused by:
  `package.autoexamples` is not allowed in embedded manifests

"#]])
        .run();
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
fn disallow_explicit_package_autotests() {
    let p = cargo_test_support::project()
        .file(
            "script.rs",
            r#"
----
[package]
edition = "2021"
autotests = true
----

fn main() {}
"#,
        )
        .build();

    p.cargo("-Zscript -v script.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/script.rs`

Caused by:
  `package.autotests` is not allowed in embedded manifests

"#]])
        .run();
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
fn disallow_explicit_package_autobenches() {
    let p = cargo_test_support::project()
        .file(
            "script.rs",
            r#"
----
[package]
edition = "2021"
autobenches = true
----

fn main() {}
"#,
        )
        .build();

    p.cargo("-Zscript -v script.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/script.rs`

Caused by:
  `package.autobenches` is not allowed in embedded manifests

"#]])
        .run();
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
fn implicit_target_dir() {
    let script = ECHO_SCRIPT;
    let p = cargo_test_support::project()
        .file("script.rs", script)
        .build();

    p.cargo("-Zscript -v script.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout_data(str![[r#"
current_exe: [ROOT]/home/.cargo/build/[HASH]/target/debug/script[EXE]
arg0: [..]
args: []

"#]])
        .with_stderr_data(str![[r#"
[WARNING] `package.edition` is unspecified, defaulting to `2024`
[COMPILING] script v0.0.0 ([ROOT]/foo/script.rs)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `[ROOT]/home/.cargo/build/[HASH]/target/debug/script[EXE]`

"#]])
        .run();
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
fn no_local_lockfile() {
    let script = ECHO_SCRIPT;
    let p = cargo_test_support::project()
        .file("script.rs", script)
        .build();
    let local_lockfile_path = p.root().join("Cargo.lock");

    assert!(!local_lockfile_path.exists());

    p.cargo("-Zscript -v script.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout_data(str![[r#"
current_exe: [ROOT]/home/.cargo/build/[HASH]/target/debug/script[EXE]
arg0: [..]
args: []

"#]])
        .with_stderr_data(str![[r#"
[WARNING] `package.edition` is unspecified, defaulting to `2024`
[COMPILING] script v0.0.0 ([ROOT]/foo/script.rs)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `[ROOT]/home/.cargo/build/[HASH]/target/debug/script[EXE]`

"#]])
        .run();

    assert!(!local_lockfile_path.exists());
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
fn cmd_check_requires_nightly() {
    let script = ECHO_SCRIPT;
    let p = cargo_test_support::project()
        .file("script.rs", script)
        .build();

    p.cargo("check --manifest-path script.rs")
        .with_status(101)
        .with_stdout_data("")
        .with_stderr_data(str![[r#"
[ERROR] embedded manifest `[ROOT]/foo/script.rs` requires `-Zscript`

"#]])
        .run();
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
fn cmd_check_requires_z_flag() {
    let script = ECHO_SCRIPT;
    let p = cargo_test_support::project()
        .file("script.rs", script)
        .build();

    p.cargo("check --manifest-path script.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_status(101)
        .with_stdout_data("")
        .with_stderr_data(str![[r#"
[ERROR] embedded manifest `[ROOT]/foo/script.rs` requires `-Zscript`

"#]])
        .run();
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
fn cmd_check_with_embedded() {
    let script = ECHO_SCRIPT;
    let p = cargo_test_support::project()
        .file("script.rs", script)
        .build();

    p.cargo("-Zscript check --manifest-path script.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout_data("")
        .with_stderr_data(str![[r#"
[WARNING] `package.edition` is unspecified, defaulting to `2024`
[CHECKING] script v0.0.0 ([ROOT]/foo/script.rs)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
fn cmd_check_with_missing_script_rs() {
    let p = cargo_test_support::project().build();

    p.cargo("-Zscript check --manifest-path script.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_status(101)
        .with_stdout_data("")
        .with_stderr_data(str![[r#"
[ERROR] the manifest-path must be a path to a Cargo.toml file: `[ROOT]/foo/script.rs`

"#]])
        .run();
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
fn cmd_check_with_missing_script() {
    let p = cargo_test_support::project().build();

    p.cargo("-Zscript check --manifest-path script")
        .masquerade_as_nightly_cargo(&["script"])
        .with_status(101)
        .with_stdout_data("")
        .with_stderr_data(str![[r#"
[ERROR] the manifest-path must be a path to a Cargo.toml file: `[ROOT]/foo/script`

"#]])
        .run();
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
fn cmd_build_with_embedded() {
    let script = ECHO_SCRIPT;
    let p = cargo_test_support::project()
        .file("script.rs", script)
        .build();

    p.cargo("-Zscript build --manifest-path script.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout_data("")
        .with_stderr_data(str![[r#"
[WARNING] `package.edition` is unspecified, defaulting to `2024`
[COMPILING] script v0.0.0 ([ROOT]/foo/script.rs)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
fn cmd_test_with_embedded() {
    let script = ECHO_SCRIPT;
    let p = cargo_test_support::project()
        .file("script.rs", script)
        .build();

    p.cargo("-Zscript test --manifest-path script.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout_data(str![[r#"

running 1 test
test test ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in [ELAPSED]s


"#]])
        .with_stderr_data(str![[r#"
[WARNING] `package.edition` is unspecified, defaulting to `2024`
[COMPILING] script v0.0.0 ([ROOT]/foo/script.rs)
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] unittests script.rs ([ROOT]/home/.cargo/build/[HASH]/debug/deps/script-[HASH][EXE])

"#]])
        .run();
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
fn cmd_clean_with_embedded() {
    let script = ECHO_SCRIPT;
    let p = cargo_test_support::project()
        .file("script.rs", script)
        .build();

    // Ensure there is something to clean
    p.cargo("-Zscript script.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .run();

    p.cargo("-Zscript clean --manifest-path script.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout_data("")
        .with_stderr_data(str![[r#"
[WARNING] `package.edition` is unspecified, defaulting to `2024`
[REMOVED] [FILE_NUM] files, [FILE_SIZE]B total

"#]])
        .run();
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
fn cmd_generate_lockfile_with_embedded() {
    let script = ECHO_SCRIPT;
    let p = cargo_test_support::project()
        .file("script.rs", script)
        .build();

    p.cargo("-Zscript generate-lockfile --manifest-path script.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout_data("")
        .with_stderr_data(str![[r#"
[WARNING] `package.edition` is unspecified, defaulting to `2024`

"#]])
        .run();
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
fn cmd_metadata_with_embedded() {
    let script = ECHO_SCRIPT;
    let p = cargo_test_support::project()
        .file("script.rs", script)
        .build();

    p.cargo("-Zscript metadata --manifest-path script.rs --format-version=1")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout_data(
            str![[r#"
{
  "metadata": null,
  "packages": [
    {
      "authors": [],
      "categories": [],
      "default_run": null,
      "dependencies": [],
      "description": null,
      "documentation": null,
      "edition": "2024",
      "features": {},
      "homepage": null,
      "id": "path+[ROOTURL]/foo/script.rs#script@0.0.0",
      "keywords": [],
      "license": null,
      "license_file": null,
      "links": null,
      "manifest_path": "[ROOT]/foo/script.rs",
      "metadata": null,
      "name": "script",
      "publish": [],
      "readme": null,
      "repository": null,
      "rust_version": null,
      "source": null,
      "targets": [
        {
          "crate_types": [
            "bin"
          ],
          "doc": true,
          "doctest": false,
          "edition": "2024",
          "kind": [
            "bin"
          ],
          "name": "script",
          "src_path": "[ROOT]/foo/script.rs",
          "test": true
        }
      ],
      "version": "0.0.0"
    }
  ],
  "resolve": {
    "nodes": [
      {
        "dependencies": [],
        "deps": [],
        "features": [],
        "id": "path+[ROOTURL]/foo/script.rs#script@0.0.0"
      }
    ],
    "root": "path+[ROOTURL]/foo/script.rs#script@0.0.0"
  },
  "target_directory": "[ROOT]/home/.cargo/build/[HASH]/target",
  "build_directory": "[ROOT]/home/.cargo/build/[HASH]",
  "version": 1,
  "workspace_default_members": [
    "path+[ROOTURL]/foo/script.rs#script@0.0.0"
  ],
  "workspace_members": [
    "path+[ROOTURL]/foo/script.rs#script@0.0.0"
  ],
  "workspace_root": "[ROOT]/foo"
}
"#]]
            .is_json(),
        )
        .with_stderr_data(str![[r#"
[WARNING] `package.edition` is unspecified, defaulting to `2024`

"#]])
        .run();
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
fn cmd_read_manifest_with_embedded() {
    let script = ECHO_SCRIPT;
    let p = cargo_test_support::project()
        .file("script.rs", script)
        .build();

    p.cargo("-Zscript read-manifest --manifest-path script.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout_data(
            str![[r#"
{
  "authors": [],
  "categories": [],
  "default_run": null,
  "dependencies": [],
  "description": null,
  "documentation": null,
  "edition": "2024",
  "features": {},
  "homepage": null,
  "id": "path+[ROOTURL]/foo/script.rs#script@0.0.0",
  "keywords": [],
  "license": null,
  "license_file": null,
  "links": null,
  "manifest_path": "[ROOT]/foo/script.rs",
  "metadata": null,
  "name": "script",
  "publish": [],
  "readme": null,
  "repository": null,
  "rust_version": null,
  "source": null,
  "targets": [
    {
      "crate_types": [
        "bin"
      ],
      "doc": true,
      "doctest": false,
      "edition": "2024",
      "kind": [
        "bin"
      ],
      "name": "script",
      "src_path": "[ROOT]/foo/script.rs",
      "test": true
    }
  ],
  "version": "0.0.0"
}
"#]]
            .is_json(),
        )
        .with_stderr_data(str![[r#"
[WARNING] `package.edition` is unspecified, defaulting to `2024`

"#]])
        .run();
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
fn cmd_run_with_embedded() {
    let p = cargo_test_support::project()
        .file("script.rs", ECHO_SCRIPT)
        .build();

    p.cargo("-Zscript run --manifest-path script.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout_data(str![[r#"
current_exe: [ROOT]/home/.cargo/build/[HASH]/target/debug/script[EXE]
arg0: [..]
args: []

"#]])
        .with_stderr_data(str![[r#"
[WARNING] `package.edition` is unspecified, defaulting to `2024`
[COMPILING] script v0.0.0 ([ROOT]/foo/script.rs)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `[ROOT]/home/.cargo/build/[HASH]/target/debug/script[EXE]`

"#]])
        .run();
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
fn cmd_tree_with_embedded() {
    let p = cargo_test_support::project()
        .file("script.rs", ECHO_SCRIPT)
        .build();

    p.cargo("-Zscript tree --manifest-path script.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout_data(str![[r#"
script v0.0.0 ([ROOT]/foo/script.rs)

"#]])
        .with_stderr_data(str![[r#"
[WARNING] `package.edition` is unspecified, defaulting to `2024`

"#]])
        .run();
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
fn cmd_update_with_embedded() {
    let p = cargo_test_support::project()
        .file("script.rs", ECHO_SCRIPT)
        .build();

    p.cargo("-Zscript update --manifest-path script.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout_data("")
        .with_stderr_data(str![[r#"
[WARNING] `package.edition` is unspecified, defaulting to `2024`

"#]])
        .run();
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
fn cmd_verify_project_with_embedded() {
    let p = cargo_test_support::project()
        .file("script.rs", ECHO_SCRIPT)
        .build();

    p.cargo("-Zscript verify-project --manifest-path script.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout_data(
            str![[r#"
{
  "success": "true"
}
"#]]
            .is_json(),
        )
        .with_stderr_data(str![[r#"
[WARNING] `package.edition` is unspecified, defaulting to `2024`

"#]])
        .run();
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
fn cmd_pkgid_with_embedded() {
    let p = cargo_test_support::project()
        .file("script.rs", ECHO_SCRIPT)
        .build();

    p.cargo("-Zscript script.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .run();

    p.cargo("-Zscript pkgid --manifest-path script.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout_data(str![[r#"
path+[ROOTURL]/foo/script.rs#script@0.0.0

"#]])
        .with_stderr_data(str![[r#"
[WARNING] `package.edition` is unspecified, defaulting to `2024`

"#]])
        .run();
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
fn cmd_pkgid_with_embedded_no_lock_file() {
    let p = cargo_test_support::project()
        .file("script.rs", ECHO_SCRIPT)
        .build();

    p.cargo("-Zscript pkgid --manifest-path script.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[WARNING] `package.edition` is unspecified, defaulting to `2024`
[ERROR] a Cargo.lock must exist for this command

"#]])
        .run();
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
fn cmd_pkgid_with_embedded_dep() {
    Package::new("dep", "1.0.0").publish();
    let script = r#"#!/usr/bin/env cargo
---
[dependencies]
dep = "1.0.0"
---

fn main() {
    println!("Hello world!");
}"#;
    let p = cargo_test_support::project()
        .file("script.rs", script)
        .build();

    p.cargo("-Zscript script.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .run();

    p.cargo("-Zscript pkgid --manifest-path script.rs -p dep")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout_data(str![[r#"
registry+https://github.com/rust-lang/crates.io-index#dep@1.0.0

"#]])
        .with_stderr_data(str![[r#"
[WARNING] `package.edition` is unspecified, defaulting to `2024`

"#]])
        .run();
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
fn script_as_dep() {
    let p = cargo_test_support::project()
        .file("script.rs", ECHO_SCRIPT)
        .file("src/lib.rs", "pub fn foo() {}")
        .file(
            "Cargo.toml",
            r#"
[package]
name = "foo"
version = "0.1.0"

[dependencies]
script.path = "script.rs"
"#,
        )
        .build();

    p.cargo("build")
        .masquerade_as_nightly_cargo(&["script"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[WARNING] no edition set: defaulting to the 2015 edition while the latest is 2024
[ERROR] failed to get `script` as a dependency of package `foo v0.1.0 ([ROOT]/foo)`

Caused by:
  failed to load source for dependency `script`

Caused by:
  Unable to update [ROOT]/foo/script.rs

Caused by:
  Single file packages cannot be used as dependencies

"#]])
        .run();
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
fn cmd_install_with_embedded() {
    let p = cargo_test_support::project()
        .file("script.rs", ECHO_SCRIPT)
        .build();

    p.cargo("-Zscript install --path script.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] `[ROOT]/foo/script.rs` is not a directory. --path must point to a directory containing a Cargo.toml file.

"#]])
        .run();
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
fn cmd_package_with_embedded() {
    let p = cargo_test_support::project()
        .file("script.rs", ECHO_SCRIPT)
        .build();

    p.cargo("-Zscript package --manifest-path script.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[WARNING] `package.edition` is unspecified, defaulting to `2024`
[ERROR] [ROOT]/foo/script.rs is unsupported by `cargo package`

"#]])
        .run();
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
fn cmd_publish_with_embedded() {
    let p = cargo_test_support::project()
        .file("script.rs", ECHO_SCRIPT)
        .build();

    p.cargo("-Zscript publish --manifest-path script.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[WARNING] `package.edition` is unspecified, defaulting to `2024`
[ERROR] [ROOT]/foo/script.rs is unsupported by `cargo publish`

"#]])
        .run();
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
fn manifest_path_env() {
    let p = cargo_test_support::project()
        .file(
            "script.rs",
            r#"#!/usr/bin/env cargo

fn main() {
    let path = env!("CARGO_MANIFEST_PATH");
    println!("CARGO_MANIFEST_PATH: {}", path);
}
"#,
        )
        .build();
    p.cargo("-Zscript -v script.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout_data(str![[r#"
CARGO_MANIFEST_PATH: [ROOT]/foo/script.rs

"#]])
        .with_stderr_data(str![[r#"
[WARNING] `package.edition` is unspecified, defaulting to `2024`
[COMPILING] script v0.0.0 ([ROOT]/foo/script.rs)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `[ROOT]/home/.cargo/build/[HASH]/target/debug/script[EXE]`

"#]])
        .run();
}

#[cargo_test(nightly, reason = "-Zscript is unstable")]
fn ignore_surrounding_workspace() {
    let p = cargo_test_support::project()
        .file(
            std::path::Path::new(".cargo").join("config.toml"),
            r#"
[registries.test-reg]
index = "https://github.com/rust-lang/crates.io-index"
"#,
        )
        .file(
            std::path::Path::new("inner").join("Cargo.toml"),
            r#"
[package]
name = "inner"
version = "0.1.0"

[dependencies]
serde = { version = "1.0", registry = "test-reg" }
"#,
        )
        .file(std::path::Path::new("inner").join("src").join("lib.rs"), "")
        .file(std::path::Path::new("script").join("echo.rs"), ECHO_SCRIPT)
        .file(
            "Cargo.toml",
            r#"
[workspace]
members = [
    "inner",
]
"#,
        )
        .build();

    p.cargo("-Zscript -v script/echo.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout_data(str![[r#"
current_exe: [ROOT]/home/.cargo/build/[HASH]/target/debug/echo[EXE]
arg0: [..]
args: []

"#]])
        .with_stderr_data(str![[r#"
[WARNING] `package.edition` is unspecified, defaulting to `2024`
[COMPILING] echo v0.0.0 ([ROOT]/foo/script/echo.rs)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `[ROOT]/home/.cargo/build/[HASH]/target/debug/echo[EXE]`

"#]])
        .run();
}
