use cargo_test_support::basic_manifest;
use cargo_test_support::registry::Package;

const ECHO_SCRIPT: &str = r#"#!/usr/bin/env cargo

fn main() {
    let mut args = std::env::args_os();
    let bin = args.next().unwrap().to_str().unwrap().to_owned();
    let args = args.collect::<Vec<_>>();
    println!("bin: {bin}");
    println!("args: {args:?}");
}
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

    p.cargo("-Zscript echo.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout(
            r#"bin: [ROOT]/home/.cargo/eval/target/eval/[..]
args: []
"#,
        )
        .with_stderr(
            "\
[WARNING] `package.edition` is unspecifiead, defaulting to `2021`
[COMPILING] echo v0.0.0 ([ROOT]/home/.cargo/eval/target/eval/[..]/echo)
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
[RUNNING] `[ROOT]/home/.cargo/eval/target/eval/[..]/echo/target/debug/echo[EXE]`
",
        )
        .run();
}

#[cargo_test]
fn basic_path() {
    let p = cargo_test_support::project()
        .file("echo", ECHO_SCRIPT)
        .build();

    p.cargo("-Zscript ./echo")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout(
            r#"bin: [ROOT]/home/.cargo/eval/target/eval/[..]
args: []
"#,
        )
        .with_stderr(
            "\
[WARNING] `package.edition` is unspecifiead, defaulting to `2021`
[COMPILING] echo v0.0.0 ([ROOT]/home/.cargo/eval/target/eval/[..]/echo)
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
[RUNNING] `[ROOT]/home/.cargo/eval/target/eval/[..]/echo/target/debug/echo[EXE]`
",
        )
        .run();
}

#[cargo_test]
fn path_required() {
    let p = cargo_test_support::project()
        .file("echo", ECHO_SCRIPT)
        .build();

    p.cargo("-Zscript echo")
        .masquerade_as_nightly_cargo(&["script"])
        .with_status(101)
        .with_stdout("")
        .with_stderr(
            "\
error: no such command: `echo`

<tab>Did you mean `bench`?

<tab>View all installed commands with `cargo --list`
",
        )
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

    p.cargo("-Zscript echo.rs")
        .env("PATH", &path)
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout(
            r#"bin: [ROOT]/home/.cargo/eval/target/eval/[..]
args: []
"#,
        )
        .with_stderr(
            "\
[WARNING] `package.edition` is unspecifiead, defaulting to `2021`
[COMPILING] echo v0.0.0 ([ROOT]/home/.cargo/eval/target/eval/[..]/echo)
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
[RUNNING] `[ROOT]/home/.cargo/eval/target/eval/[..]/echo/target/debug/echo[EXE]`
",
        )
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

    p.cargo("echo.rs")
        .env("PATH", &path)
        .with_stdout("")
        .with_stderr(
            "\
warning: external subcommand `echo.rs` has the appearance of a manfiest-command
This was previously accepted but will be phased out when `-Zscript` is stabilized.
For more information, see issue #12207 <https://github.com/rust-lang/cargo/issues/12207>.
",
        )
        .run();
}

#[cargo_test]
fn requires_nightly() {
    let p = cargo_test_support::project()
        .file("echo.rs", ECHO_SCRIPT)
        .build();

    p.cargo("echo.rs")
        .with_status(101)
        .with_stdout("")
        .with_stderr(
            "\
error: running `echo.rs` requires `-Zscript`
",
        )
        .run();
}

#[cargo_test]
fn requires_z_flag() {
    let p = cargo_test_support::project()
        .file("echo.rs", ECHO_SCRIPT)
        .build();

    p.cargo("echo.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_status(101)
        .with_stdout("")
        .with_stderr(
            "\
error: running `echo.rs` requires `-Zscript`
",
        )
        .run();
}

#[cargo_test]
fn clean_output_with_edition() {
    let script = r#"#!/usr/bin/env cargo

//! ```cargo
//! [package]
//! edition = "2018"
//! ```

fn main() {
    println!("Hello world!");
}"#;
    let p = cargo_test_support::project()
        .file("script.rs", script)
        .build();

    p.cargo("-Zscript script.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout(
            r#"Hello world!
"#,
        )
        .with_stderr(
            "\
[COMPILING] script v0.0.0 ([ROOT]/home/.cargo/eval/target/eval/[..]/script)
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
[RUNNING] `[ROOT]/home/.cargo/eval/target/eval/[..]/script/target/debug/script[EXE]`
",
        )
        .run();
}

#[cargo_test]
fn warning_without_edition() {
    let script = r#"#!/usr/bin/env cargo

//! ```cargo
//! [package]
//! ```

fn main() {
    println!("Hello world!");
}"#;
    let p = cargo_test_support::project()
        .file("script.rs", script)
        .build();

    p.cargo("-Zscript script.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout(
            r#"Hello world!
"#,
        )
        .with_stderr(
            "\
[WARNING] `package.edition` is unspecifiead, defaulting to `2021`
[COMPILING] script v0.0.0 ([ROOT]/home/.cargo/eval/target/eval/[..]/script)
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
[RUNNING] `[ROOT]/home/.cargo/eval/target/eval/[..]/script/target/debug/script[EXE]`
",
        )
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

    p.cargo("-Zscript script.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout(
            r#"msg = undefined
"#,
        )
        .with_stderr(
            "\
[WARNING] `package.edition` is unspecifiead, defaulting to `2021`
[COMPILING] script v0.0.0 ([ROOT]/home/.cargo/eval/target/eval/[..]/script)
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
[RUNNING] `[ROOT]/home/.cargo/eval/target/eval/[..]/script/target/debug/script[EXE]`
",
        )
        .run();

    // Verify we don't rebuild
    p.cargo("-Zscript script.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout(
            r#"msg = undefined
"#,
        )
        .with_stderr(
            "\
[WARNING] `package.edition` is unspecifiead, defaulting to `2021`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
[RUNNING] `[ROOT]/home/.cargo/eval/target/eval/[..]/script/target/debug/script[EXE]`
",
        )
        .run();

    // Verify we do rebuild
    p.cargo("-Zscript script.rs")
        .env("_MESSAGE", "hello")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout(
            r#"msg = hello
"#,
        )
        .with_stderr(
            "\
[WARNING] `package.edition` is unspecifiead, defaulting to `2021`
[COMPILING] script v0.0.0 ([ROOT]/home/.cargo/eval/target/eval/[..]/script)
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
[RUNNING] `[ROOT]/home/.cargo/eval/target/eval/[..]/script/target/debug/script[EXE]`
",
        )
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

    p.cargo("-Zscript script.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout(
            r#"line: 4
"#,
        )
        .with_stderr(
            "\
[WARNING] `package.edition` is unspecifiead, defaulting to `2021`
[COMPILING] script v0.0.0 ([ROOT]/home/.cargo/eval/target/eval/[..]/script)
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
[RUNNING] `[ROOT]/home/.cargo/eval/target/eval/[..]/script/target/debug/script[EXE]`
",
        )
        .run();
}

#[cargo_test]
fn test_escaped_hyphen_arg() {
    let script = ECHO_SCRIPT;
    let p = cargo_test_support::project()
        .file("script.rs", script)
        .build();

    p.cargo("-Zscript -- script.rs -NotAnArg")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout(
            r#"bin: [ROOT]/home/.cargo/eval/target/eval/[..]
args: ["-NotAnArg"]
"#,
        )
        .with_stderr(
            "\
[WARNING] `package.edition` is unspecifiead, defaulting to `2021`
[COMPILING] script v0.0.0 ([ROOT]/home/.cargo/eval/target/eval/[..]/script)
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
[RUNNING] `[ROOT]/home/.cargo/eval/target/eval/[..]/script/target/debug/script[EXE] -NotAnArg`
",
        )
        .run();
}

#[cargo_test]
fn test_unescaped_hyphen_arg() {
    let script = ECHO_SCRIPT;
    let p = cargo_test_support::project()
        .file("script.rs", script)
        .build();

    p.cargo("-Zscript script.rs -NotAnArg")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout(
            r#"bin: [ROOT]/home/.cargo/eval/target/eval/[..]
args: ["-NotAnArg"]
"#,
        )
        .with_stderr(
            "\
[WARNING] `package.edition` is unspecifiead, defaulting to `2021`
[COMPILING] script v0.0.0 ([ROOT]/home/.cargo/eval/target/eval/[..]/script)
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
[RUNNING] `[ROOT]/home/.cargo/eval/target/eval/[..]/script/target/debug/script[EXE] -NotAnArg`
",
        )
        .run();
}

#[cargo_test]
fn test_same_flags() {
    let script = ECHO_SCRIPT;
    let p = cargo_test_support::project()
        .file("script.rs", script)
        .build();

    p.cargo("-Zscript script.rs --help")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout(
            r#"bin: [ROOT]/home/.cargo/eval/target/eval/[..]
args: ["--help"]
"#,
        )
        .with_stderr(
            "\
[WARNING] `package.edition` is unspecifiead, defaulting to `2021`
[COMPILING] script v0.0.0 ([ROOT]/home/.cargo/eval/target/eval/[..]/script)
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
[RUNNING] `[ROOT]/home/.cargo/eval/target/eval/[..]/script/target/debug/script[EXE] --help`
",
        )
        .run();
}

#[cargo_test]
fn test_name_has_weird_chars() {
    let script = ECHO_SCRIPT;
    let p = cargo_test_support::project()
        .file("s-h.w§c!.rs", script)
        .build();

    p.cargo("-Zscript s-h.w§c!.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout(
            r#"bin: [ROOT]/home/.cargo/eval/target/eval/[..]
args: []
"#,
        )
        .with_stderr(
            r#"[WARNING] `package.edition` is unspecifiead, defaulting to `2021`
[COMPILING] s-h-w-c- v0.0.0 ([ROOT]/home/.cargo/eval/target/eval/[..]/s-h-w-c-)
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
[RUNNING] `[ROOT]/home/.cargo/eval/target/eval/[..]/s-h-w-c-/target/debug/s-h-w-c-[EXE]`
"#,
        )
        .run();
}

#[cargo_test]
fn test_name_same_as_dependency() {
    Package::new("script", "1.0.0").publish();
    let script = r#"#!/usr/bin/env cargo

//! ```cargo
//! [dependencies]
//! script = "1.0.0"
//! ```

fn main() {
    println!("Hello world!");
}"#;
    let p = cargo_test_support::project()
        .file("script.rs", script)
        .build();

    p.cargo("-Zscript script.rs --help")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout(
            r#"Hello world!
"#,
        )
        .with_stderr(
            "\
[WARNING] `package.edition` is unspecifiead, defaulting to `2021`
[UPDATING] `dummy-registry` index
[DOWNLOADING] crates ...
[DOWNLOADED] script v1.0.0 (registry `dummy-registry`)
[COMPILING] script v1.0.0
[COMPILING] script v0.0.0 ([ROOT]/home/.cargo/eval/target/eval/[..]/script)
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
[RUNNING] `[ROOT]/home/.cargo/eval/target/eval/[..]/script/target/debug/script[EXE] --help`
",
        )
        .run();
}

#[cargo_test]
fn test_path_dep() {
    let script = r#"#!/usr/bin/env cargo

//! ```cargo
//! [dependencies]
//! bar.path = "./bar"
//! ```

fn main() {
    println!("Hello world!");
}"#;
    let p = cargo_test_support::project()
        .file("script.rs", script)
        .file("src/lib.rs", "pub fn foo() {}")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file("bar/src/lib.rs", "pub fn bar() {}")
        .build();

    p.cargo("-Zscript script.rs --help")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout(
            r#"Hello world!
"#,
        )
        .with_stderr(
            "\
[WARNING] `package.edition` is unspecifiead, defaulting to `2021`
[COMPILING] bar v0.0.1 ([ROOT]/foo/bar)
[COMPILING] script v0.0.0 ([ROOT]/home/.cargo/eval/target/eval/[..]/script)
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
[RUNNING] `[ROOT]/home/.cargo/eval/target/eval/[..]/script/target/debug/script[EXE] --help`
",
        )
        .run();
}
