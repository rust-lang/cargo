use cargo_test_support::basic_manifest;
use cargo_test_support::prelude::*;
use cargo_test_support::registry::Package;
use cargo_test_support::str;

const ECHO_SCRIPT: &str = r#"#!/usr/bin/env cargo

fn main() {
    let mut args = std::env::args_os();
    let bin = args.next().unwrap().to_str().unwrap().to_owned();
    let args = args.collect::<Vec<_>>();
    println!("bin: {bin}");
    println!("args: {args:?}");
}

#[test]
fn test () {}
"#;

#[cfg(unix)]
fn path() -> Vec<std::path::PathBuf> {
    std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()).collect()
}

#[cargo_test]
fn basic_rs() {
    let p = cargo_test_support::project()
        .file("echo.rs", ECHO_SCRIPT)
        .build();

    p.cargo("-Zscript -v echo.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout_data(str![[r#"
bin: [ROOT]/home/.cargo/target/[HASH]/debug/echo[EXE]
args: []

"#]])
        .with_stderr_data(str![[r#"
[WARNING] `package.edition` is unspecified, defaulting to `2021`
[COMPILING] echo v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `[ROOT]/home/.cargo/target/[HASH]/debug/echo[EXE]`

"#]])
        .run();
}

#[cargo_test]
fn basic_path() {
    let p = cargo_test_support::project()
        .file("echo", ECHO_SCRIPT)
        .build();

    p.cargo("-Zscript -v ./echo")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout_data(str![[r#"
bin: [ROOT]/home/.cargo/target/[HASH]/debug/echo[EXE]
args: []

"#]])
        .with_stderr_data(str![[r#"
[WARNING] `package.edition` is unspecified, defaulting to `2021`
[COMPILING] echo v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `[ROOT]/home/.cargo/target/[HASH]/debug/echo[EXE]`

"#]])
        .run();
}

#[cargo_test]
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

	Did you mean `bench`?

	View all installed commands with `cargo --list`
	Find a package to install `echo` with `cargo search cargo-echo`
	To run the file `echo`, provide a relative path like `./echo`

"#]])
        .run();
}

#[cargo_test]
#[cfg(unix)]
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
bin: [ROOT]/home/.cargo/target/[HASH]/debug/echo[EXE]
args: []

"#]])
        .with_stderr_data(str![[r#"
[WARNING] `package.edition` is unspecified, defaulting to `2021`
[COMPILING] echo v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `[ROOT]/home/.cargo/target/[HASH]/debug/echo[EXE]`

"#]])
        .run();
}

#[cargo_test]
#[cfg(unix)]
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
This was previously accepted but will be phased out when `-Zscript` is stabilized.
For more information, see issue #12207 <https://github.com/rust-lang/cargo/issues/12207>.

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

#[cargo_test]
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

#[cargo_test]
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
[COMPILING] script v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `[ROOT]/home/.cargo/target/[HASH]/debug/script[EXE]`

"#]])
        .run();
}

#[cargo_test]
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
[WARNING] `package.edition` is unspecified, defaulting to `2021`
[COMPILING] script v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `[ROOT]/home/.cargo/target/[HASH]/debug/script[EXE]`

"#]])
        .run();
}

#[cargo_test]
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
[WARNING] `package.edition` is unspecified, defaulting to `2021`
[COMPILING] script v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `[ROOT]/home/.cargo/target/[HASH]/debug/script[EXE]`

"#]])
        .run();

    // Verify we don't rebuild
    p.cargo("-Zscript -v script.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout_data(str![[r#"
msg = undefined

"#]])
        .with_stderr_data(str![[r#"
[WARNING] `package.edition` is unspecified, defaulting to `2021`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `[ROOT]/home/.cargo/target/[HASH]/debug/script[EXE]`

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
[WARNING] `package.edition` is unspecified, defaulting to `2021`
[COMPILING] script v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `[ROOT]/home/.cargo/target/[HASH]/debug/script[EXE]`

"#]])
        .run();
}

#[cargo_test]
fn use_script_config() {
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

    // Verify the config is bad
    p.cargo("-Zscript script.rs -NotAnArg")
        .masquerade_as_nightly_cargo(&["script"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] could not execute process `non-existent-rustc -vV` (never executed)

Caused by:
  [NOT_FOUND]

"#]])
        .run();

    // Verify that the config isn't used
    p.cargo("-Zscript ../script/script.rs -NotAnArg")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout_data(str![[r#"
bin: [ROOT]/home/.cargo/target/[HASH]/debug/script[EXE]
args: ["-NotAnArg"]

"#]])
        .run();
}

#[cargo_test]
fn default_programmatic_verbosity() {
    let script = ECHO_SCRIPT;
    let p = cargo_test_support::project()
        .file("script.rs", script)
        .build();

    p.cargo("-Zscript script.rs -NotAnArg")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout_data(str![[r#"
bin: [ROOT]/home/.cargo/target/[HASH]/debug/script[EXE]
args: ["-NotAnArg"]

"#]])
        .with_stderr_data("")
        .run();
}

#[cargo_test]
fn quiet() {
    let script = ECHO_SCRIPT;
    let p = cargo_test_support::project()
        .file("script.rs", script)
        .build();

    p.cargo("-Zscript -q script.rs -NotAnArg")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout_data(str![[r#"
bin: [ROOT]/home/.cargo/target/[HASH]/debug/script[EXE]
args: ["-NotAnArg"]

"#]])
        .with_stderr_data("")
        .run();
}

#[cargo_test]
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
[WARNING] `package.edition` is unspecified, defaulting to `2021`
[COMPILING] script v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `[ROOT]/home/.cargo/target/[HASH]/debug/script[EXE]`

"#]])
        .run();
}

#[cargo_test]
fn test_escaped_hyphen_arg() {
    let script = ECHO_SCRIPT;
    let p = cargo_test_support::project()
        .file("script.rs", script)
        .build();

    p.cargo("-Zscript -v -- script.rs -NotAnArg")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout_data(str![[r#"
bin: [ROOT]/home/.cargo/target/[HASH]/debug/script[EXE]
args: ["-NotAnArg"]

"#]])
        .with_stderr_data(str![[r#"
[WARNING] `package.edition` is unspecified, defaulting to `2021`
[COMPILING] script v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `[ROOT]/home/.cargo/target/[HASH]/debug/script[EXE] -NotAnArg`

"#]])
        .run();
}

#[cargo_test]
fn test_unescaped_hyphen_arg() {
    let script = ECHO_SCRIPT;
    let p = cargo_test_support::project()
        .file("script.rs", script)
        .build();

    p.cargo("-Zscript -v script.rs -NotAnArg")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout_data(str![[r#"
bin: [ROOT]/home/.cargo/target/[HASH]/debug/script[EXE]
args: ["-NotAnArg"]

"#]])
        .with_stderr_data(str![[r#"
[WARNING] `package.edition` is unspecified, defaulting to `2021`
[COMPILING] script v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `[ROOT]/home/.cargo/target/[HASH]/debug/script[EXE] -NotAnArg`

"#]])
        .run();
}

#[cargo_test]
fn test_same_flags() {
    let script = ECHO_SCRIPT;
    let p = cargo_test_support::project()
        .file("script.rs", script)
        .build();

    p.cargo("-Zscript -v script.rs --help")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout_data(str![[r#"
bin: [ROOT]/home/.cargo/target/[HASH]/debug/script[EXE]
args: ["--help"]

"#]])
        .with_stderr_data(str![[r#"
[WARNING] `package.edition` is unspecified, defaulting to `2021`
[COMPILING] script v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `[ROOT]/home/.cargo/target/[HASH]/debug/script[EXE] --help`

"#]])
        .run();
}

#[cargo_test]
fn test_name_has_weird_chars() {
    let script = ECHO_SCRIPT;
    let p = cargo_test_support::project()
        .file("s-h.w§c!.rs", script)
        .build();

    p.cargo("-Zscript -v s-h.w§c!.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout_data(str![[r#"
bin: [ROOT]/home/.cargo/target/[HASH]/debug/s-h-w-c-[EXE]
args: []

"#]])
        .with_stderr_data(str![[r#"
[WARNING] `package.edition` is unspecified, defaulting to `2021`
[COMPILING] s-h-w-c- v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `[ROOT]/home/.cargo/target/[HASH]/debug/s-h-w-c-[EXE]`

"#]])
        .run();
}

#[cargo_test]
fn test_name_has_leading_number() {
    let script = ECHO_SCRIPT;
    let p = cargo_test_support::project()
        .file("42answer.rs", script)
        .build();

    p.cargo("-Zscript -v 42answer.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout_data(str![[r#"
bin: [ROOT]/home/.cargo/target/[HASH]/debug/answer[EXE]
args: []

"#]])
        .with_stderr_data(str![[r#"
[WARNING] `package.edition` is unspecified, defaulting to `2021`
[COMPILING] answer v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `[ROOT]/home/.cargo/target/[HASH]/debug/answer[EXE]`

"#]])
        .run();
}

#[cargo_test]
fn test_name_is_number() {
    let script = ECHO_SCRIPT;
    let p = cargo_test_support::project().file("42.rs", script).build();

    p.cargo("-Zscript -v 42.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout_data(str![[r#"
bin: [ROOT]/home/.cargo/target/[HASH]/debug/package[EXE]
args: []

"#]])
        .with_stderr_data(str![[r#"
[WARNING] `package.edition` is unspecified, defaulting to `2021`
[COMPILING] package v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `[ROOT]/home/.cargo/target/[HASH]/debug/package[EXE]`

"#]])
        .run();
}

#[cargo_test]
fn script_like_dir() {
    let p = cargo_test_support::project()
        .file("foo.rs/foo", "something")
        .build();

    p.cargo("-Zscript -v foo.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] no such file or subcommand `foo.rs`
	`foo.rs` is a directory

"#]])
        .run();
}

#[cargo_test]
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

#[cargo_test]
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

#[cargo_test]
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
	Did you mean the file `./food.rs`

"#]])
        .run();
}

#[cargo_test]
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
	Did you mean the file `./food.rs` with `-Zscript`

"#]])
        .run();
}

#[cargo_test]
fn did_you_mean_command() {
    let p = cargo_test_support::project().build();

    p.cargo("-Zscript -v build--manifest-path=./Cargo.toml")
        .masquerade_as_nightly_cargo(&["script"])
        .with_status(101)
        .with_stdout_data("")
        .with_stderr_data(str![[r#"
[ERROR] no such file or subcommand `build--manifest-path=./Cargo.toml`
	Did you mean the command `build --manifest-path=./Cargo.toml`

"#]])
        .run();
}

#[cargo_test]
fn did_you_mean_command_stable() {
    let p = cargo_test_support::project().build();

    p.cargo("-v build--manifest-path=./Cargo.toml")
        .masquerade_as_nightly_cargo(&["script"])
        .with_status(101)
        .with_stdout_data("")
        .with_stderr_data(str![[r#"
[ERROR] no such subcommand `build--manifest-path=./Cargo.toml`
	Did you mean the command `build --manifest-path=./Cargo.toml`

"#]])
        .run();
}

#[cargo_test]
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
[WARNING] `package.edition` is unspecified, defaulting to `2021`
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] script v1.0.0 (registry `dummy-registry`)
[COMPILING] script v1.0.0
[COMPILING] script v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `[ROOT]/home/.cargo/target/[HASH]/debug/script[EXE] --help`

"#]])
        .run();
}

#[cargo_test]
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
[WARNING] `package.edition` is unspecified, defaulting to `2021`
[LOCKING] 1 package to latest compatible version
[COMPILING] bar v0.0.1 ([ROOT]/foo/bar)
[COMPILING] script v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `[ROOT]/home/.cargo/target/[HASH]/debug/script[EXE] --help`

"#]])
        .run();
}

#[cargo_test]
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
[WARNING] `package.edition` is unspecified, defaulting to `2021`
[COMPILING] script v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `[ROOT]/home/.cargo/target/[HASH]/debug/script[EXE] --help`

"#]])
        .run();
}

#[cargo_test]
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
[WARNING] `package.edition` is unspecified, defaulting to `2021`
[COMPILING] script v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `[ROOT]/home/.cargo/target/[HASH]/debug/script[EXE] --help`

"#]])
        .run();
}

#[cargo_test]
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
[WARNING] `package.edition` is unspecified, defaulting to `2021`
[COMPILING] script v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `[ROOT]/home/.cargo/target/[HASH]/debug/script[EXE] --help`

"#]])
        .run();
}

#[cargo_test]
fn implicit_target_dir() {
    let script = ECHO_SCRIPT;
    let p = cargo_test_support::project()
        .file("script.rs", script)
        .build();

    p.cargo("-Zscript -v script.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout_data(str![[r#"
bin: [ROOT]/home/.cargo/target/[HASH]/debug/script[EXE]
args: []

"#]])
        .with_stderr_data(str![[r#"
[WARNING] `package.edition` is unspecified, defaulting to `2021`
[COMPILING] script v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `[ROOT]/home/.cargo/target/[HASH]/debug/script[EXE]`

"#]])
        .run();
}

#[cargo_test]
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
bin: [ROOT]/home/.cargo/target/[HASH]/debug/script[EXE]
args: []

"#]])
        .with_stderr_data(str![[r#"
[WARNING] `package.edition` is unspecified, defaulting to `2021`
[COMPILING] script v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `[ROOT]/home/.cargo/target/[HASH]/debug/script[EXE]`

"#]])
        .run();

    assert!(!local_lockfile_path.exists());
}

#[cargo_test]
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

#[cargo_test]
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

#[cargo_test]
fn cmd_check_with_embedded() {
    let script = ECHO_SCRIPT;
    let p = cargo_test_support::project()
        .file("script.rs", script)
        .build();

    p.cargo("-Zscript check --manifest-path script.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout_data("")
        .with_stderr_data(str![[r#"
[WARNING] `package.edition` is unspecified, defaulting to `2021`
[CHECKING] script v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn cmd_check_with_missing_script_rs() {
    let p = cargo_test_support::project().build();

    p.cargo("-Zscript check --manifest-path script.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_status(101)
        .with_stdout_data("")
        .with_stderr_data(str![[r#"
[ERROR] manifest path `script.rs` does not exist

"#]])
        .run();
}

#[cargo_test]
fn cmd_check_with_missing_script() {
    let p = cargo_test_support::project().build();

    p.cargo("-Zscript check --manifest-path script")
        .masquerade_as_nightly_cargo(&["script"])
        .with_status(101)
        .with_stdout_data("")
        .with_stderr_data(str![[r#"
[ERROR] the manifest-path must be a path to a Cargo.toml file

"#]])
        .run();
}

#[cargo_test]
fn cmd_build_with_embedded() {
    let script = ECHO_SCRIPT;
    let p = cargo_test_support::project()
        .file("script.rs", script)
        .build();

    p.cargo("-Zscript build --manifest-path script.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout_data("")
        .with_stderr_data(str![[r#"
[WARNING] `package.edition` is unspecified, defaulting to `2021`
[COMPILING] script v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
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
[WARNING] `package.edition` is unspecified, defaulting to `2021`
[COMPILING] script v0.0.0 ([ROOT]/foo)
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] unittests script.rs ([ROOT]/home/.cargo/target/[HASH]/debug/deps/script-[HASH][EXE])

"#]])
        .run();
}

#[cargo_test]
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
[WARNING] `package.edition` is unspecified, defaulting to `2021`
[REMOVED] [FILE_NUM] files, [FILE_SIZE]B total

"#]])
        .run();
}

#[cargo_test]
fn cmd_generate_lockfile_with_embedded() {
    let script = ECHO_SCRIPT;
    let p = cargo_test_support::project()
        .file("script.rs", script)
        .build();

    p.cargo("-Zscript generate-lockfile --manifest-path script.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout_data("")
        .with_stderr_data(str![[r#"
[WARNING] `package.edition` is unspecified, defaulting to `2021`

"#]])
        .run();
}

#[cargo_test]
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
      "edition": "2021",
      "features": {},
      "homepage": null,
      "id": "path+[ROOTURL]/foo#script@0.0.0",
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
          "edition": "2021",
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
        "id": "path+[ROOTURL]/foo#script@0.0.0"
      }
    ],
    "root": "path+[ROOTURL]/foo#script@0.0.0"
  },
  "target_directory": "[ROOT]/home/.cargo/target/[HASH]",
  "version": 1,
  "workspace_default_members": [
    "path+[ROOTURL]/foo#script@0.0.0"
  ],
  "workspace_members": [
    "path+[ROOTURL]/foo#script@0.0.0"
  ],
  "workspace_root": "[ROOT]/foo"
}
"#]]
            .is_json(),
        )
        .with_stderr_data(str![[r#"
[WARNING] `package.edition` is unspecified, defaulting to `2021`

"#]])
        .run();
}

#[cargo_test]
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
  "edition": "2021",
  "features": {},
  "homepage": null,
  "id": "path+[ROOTURL]/foo#script@0.0.0",
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
      "edition": "2021",
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
[WARNING] `package.edition` is unspecified, defaulting to `2021`

"#]])
        .run();
}

#[cargo_test]
fn cmd_run_with_embedded() {
    let p = cargo_test_support::project()
        .file("script.rs", ECHO_SCRIPT)
        .build();

    p.cargo("-Zscript run --manifest-path script.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout_data(str![[r#"
bin: [ROOT]/home/.cargo/target/[HASH]/debug/script[EXE]
args: []

"#]])
        .with_stderr_data(str![[r#"
[WARNING] `package.edition` is unspecified, defaulting to `2021`
[COMPILING] script v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `[ROOT]/home/.cargo/target/[HASH]/debug/script[EXE]`

"#]])
        .run();
}

#[cargo_test]
fn cmd_tree_with_embedded() {
    let p = cargo_test_support::project()
        .file("script.rs", ECHO_SCRIPT)
        .build();

    p.cargo("-Zscript tree --manifest-path script.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout_data(str![[r#"
script v0.0.0 ([ROOT]/foo)

"#]])
        .with_stderr_data(str![[r#"
[WARNING] `package.edition` is unspecified, defaulting to `2021`

"#]])
        .run();
}

#[cargo_test]
fn cmd_update_with_embedded() {
    let p = cargo_test_support::project()
        .file("script.rs", ECHO_SCRIPT)
        .build();

    p.cargo("-Zscript update --manifest-path script.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout_data("")
        .with_stderr_data(str![[r#"
[WARNING] `package.edition` is unspecified, defaulting to `2021`

"#]])
        .run();
}

#[cargo_test]
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
[WARNING] `package.edition` is unspecified, defaulting to `2021`

"#]])
        .run();
}

#[cargo_test]
fn cmd_pkgid_with_embedded() {
    let p = cargo_test_support::project()
        .file("script.rs", ECHO_SCRIPT)
        .build();

    p.cargo("-Zscript pkgid --manifest-path script.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[WARNING] `package.edition` is unspecified, defaulting to `2021`
[ERROR] [ROOT]/foo/script.rs is unsupported by `cargo pkgid`

"#]])
        .run();
}

#[cargo_test]
fn cmd_package_with_embedded() {
    let p = cargo_test_support::project()
        .file("script.rs", ECHO_SCRIPT)
        .build();

    p.cargo("-Zscript package --manifest-path script.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[WARNING] `package.edition` is unspecified, defaulting to `2021`
[ERROR] [ROOT]/foo/script.rs is unsupported by `cargo package`

"#]])
        .run();
}

#[cargo_test]
fn cmd_publish_with_embedded() {
    let p = cargo_test_support::project()
        .file("script.rs", ECHO_SCRIPT)
        .build();

    p.cargo("-Zscript publish --manifest-path script.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[WARNING] `package.edition` is unspecified, defaulting to `2021`
[ERROR] [ROOT]/foo/script.rs is unsupported by `cargo publish`

"#]])
        .run();
}

#[cargo_test]
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
[WARNING] `package.edition` is unspecified, defaulting to `2021`
[COMPILING] script v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `[ROOT]/home/.cargo/target/[HASH]/debug/script[EXE]`

"#]])
        .run();
}
