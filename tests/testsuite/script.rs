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
        .with_stdout(
            r#"bin: [..]/debug/echo[EXE]
args: []
"#,
        )
        .with_stderr(
            "\
[WARNING] `package.edition` is unspecified, defaulting to `2021`
[COMPILING] echo v0.0.0 ([ROOT]/foo)
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
[RUNNING] `[..]/debug/echo[EXE]`
",
        )
        .run();
}

#[cargo_test]
fn basic_path() {
    let p = cargo_test_support::project()
        .file("echo", ECHO_SCRIPT)
        .build();

    p.cargo("-Zscript -v ./echo")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout(
            r#"bin: [..]/debug/echo[EXE]
args: []
"#,
        )
        .with_stderr(
            "\
[WARNING] `package.edition` is unspecified, defaulting to `2021`
[COMPILING] echo v0.0.0 ([ROOT]/foo)
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
[RUNNING] `[..]/debug/echo[EXE]`
",
        )
        .run();
}

#[cargo_test]
fn basic_cargo_toml() {
    let p = cargo_test_support::project()
        .file("src/main.rs", ECHO_SCRIPT)
        .build();

    p.cargo("-Zscript -v Cargo.toml")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout(
            r#"bin: target/debug/foo[EXE]
args: []
"#,
        )
        .with_stderr(
            "\
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
[RUNNING] `target/debug/foo[EXE]`
",
        )
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
        .with_stdout("")
        .with_stderr(
            "\
error: no such command: `echo`

<tab>Did you mean `bench`?

<tab>View all installed commands with `cargo --list`
<tab>Find a package to install `echo` with `cargo search cargo-echo`
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

    p.cargo("-Zscript -v echo.rs")
        .env("PATH", &path)
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout(
            r#"bin: [..]/debug/echo[EXE]
args: []
"#,
        )
        .with_stderr(
            "\
[WARNING] `package.edition` is unspecified, defaulting to `2021`
[COMPILING] echo v0.0.0 ([ROOT]/foo)
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
[RUNNING] `[..]/debug/echo[EXE]`
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

    p.cargo("-v echo.rs")
        .env("PATH", &path)
        .with_stdout("")
        .with_stderr(
            "\
warning: external subcommand `echo.rs` has the appearance of a manifest-command
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

    p.cargo("-v echo.rs")
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

    p.cargo("-v echo.rs")
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
```cargo
[package]
edition = "2018"
```

fn main() {
    println!("Hello world!");
}"#;
    let p = cargo_test_support::project()
        .file("script.rs", script)
        .build();

    p.cargo("-Zscript -v script.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout(
            r#"Hello world!
"#,
        )
        .with_stderr(
            "\
[COMPILING] script v0.0.0 ([ROOT]/foo)
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
[RUNNING] `[..]/debug/script[EXE]`
",
        )
        .run();
}

#[cargo_test]
fn warning_without_edition() {
    let script = r#"#!/usr/bin/env cargo
```cargo
[package]
```

fn main() {
    println!("Hello world!");
}"#;
    let p = cargo_test_support::project()
        .file("script.rs", script)
        .build();

    p.cargo("-Zscript -v script.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout(
            r#"Hello world!
"#,
        )
        .with_stderr(
            "\
[WARNING] `package.edition` is unspecified, defaulting to `2021`
[COMPILING] script v0.0.0 ([ROOT]/foo)
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
[RUNNING] `[..]/debug/script[EXE]`
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

    p.cargo("-Zscript -v script.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout(
            r#"msg = undefined
"#,
        )
        .with_stderr(
            "\
[WARNING] `package.edition` is unspecified, defaulting to `2021`
[COMPILING] script v0.0.0 ([ROOT]/foo)
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
[RUNNING] `[..]/debug/script[EXE]`
",
        )
        .run();

    // Verify we don't rebuild
    p.cargo("-Zscript -v script.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout(
            r#"msg = undefined
"#,
        )
        .with_stderr(
            "\
[WARNING] `package.edition` is unspecified, defaulting to `2021`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
[RUNNING] `[..]/debug/script[EXE]`
",
        )
        .run();

    // Verify we do rebuild
    p.cargo("-Zscript -v script.rs")
        .env("_MESSAGE", "hello")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout(
            r#"msg = hello
"#,
        )
        .with_stderr(
            "\
[WARNING] `package.edition` is unspecified, defaulting to `2021`
[COMPILING] script v0.0.0 ([ROOT]/foo)
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
[RUNNING] `[..]/debug/script[EXE]`
",
        )
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
            ".cargo/config",
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
        .with_stderr_contains(
            "\
[ERROR] could not execute process `non-existent-rustc -vV` (never executed)
",
        )
        .run();

    // Verify that the config isn't used
    p.cargo("-Zscript ../script/script.rs -NotAnArg")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout(
            r#"bin: [..]/debug/script[EXE]
args: ["-NotAnArg"]
"#,
        )
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
        .with_stdout(
            r#"bin: [..]/debug/script[EXE]
args: ["-NotAnArg"]
"#,
        )
        .with_stderr(
            "\
",
        )
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
        .with_stdout(
            r#"bin: [..]/debug/script[EXE]
args: ["-NotAnArg"]
"#,
        )
        .with_stderr(
            "\
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

    p.cargo("-Zscript -v script.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout(
            r#"line: 4
"#,
        )
        .with_stderr(
            "\
[WARNING] `package.edition` is unspecified, defaulting to `2021`
[COMPILING] script v0.0.0 ([ROOT]/foo)
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
[RUNNING] `[..]/debug/script[EXE]`
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

    p.cargo("-Zscript -v -- script.rs -NotAnArg")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout(
            r#"bin: [..]/debug/script[EXE]
args: ["-NotAnArg"]
"#,
        )
        .with_stderr(
            "\
[WARNING] `package.edition` is unspecified, defaulting to `2021`
[COMPILING] script v0.0.0 ([ROOT]/foo)
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
[RUNNING] `[..]/debug/script[EXE] -NotAnArg`
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

    p.cargo("-Zscript -v script.rs -NotAnArg")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout(
            r#"bin: [..]/debug/script[EXE]
args: ["-NotAnArg"]
"#,
        )
        .with_stderr(
            "\
[WARNING] `package.edition` is unspecified, defaulting to `2021`
[COMPILING] script v0.0.0 ([ROOT]/foo)
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
[RUNNING] `[..]/debug/script[EXE] -NotAnArg`
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

    p.cargo("-Zscript -v script.rs --help")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout(
            r#"bin: [..]/debug/script[EXE]
args: ["--help"]
"#,
        )
        .with_stderr(
            "\
[WARNING] `package.edition` is unspecified, defaulting to `2021`
[COMPILING] script v0.0.0 ([ROOT]/foo)
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
[RUNNING] `[..]/debug/script[EXE] --help`
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

    p.cargo("-Zscript -v s-h.w§c!.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout(
            r#"bin: [..]/debug/s-h-w-c-[EXE]
args: []
"#,
        )
        .with_stderr(
            r#"[WARNING] `package.edition` is unspecified, defaulting to `2021`
[COMPILING] s-h-w-c- v0.0.0 ([ROOT]/foo)
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
[RUNNING] `[..]/debug/s-h-w-c-[EXE]`
"#,
        )
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
        .with_stdout(
            r#"bin: [..]/debug/answer[EXE]
args: []
"#,
        )
        .with_stderr(
            r#"[WARNING] `package.edition` is unspecified, defaulting to `2021`
[COMPILING] answer v0.0.0 ([ROOT]/foo)
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
[RUNNING] `[..]/debug/answer[EXE]`
"#,
        )
        .run();
}

#[cargo_test]
fn test_name_is_number() {
    let script = ECHO_SCRIPT;
    let p = cargo_test_support::project().file("42.rs", script).build();

    p.cargo("-Zscript -v 42.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout(
            r#"bin: [..]/debug/package[EXE]
args: []
"#,
        )
        .with_stderr(
            r#"[WARNING] `package.edition` is unspecified, defaulting to `2021`
[COMPILING] package v0.0.0 ([ROOT]/foo)
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
[RUNNING] `[..]/debug/package[EXE]`
"#,
        )
        .run();
}

#[cargo_test]
fn script_like_dir() {
    let p = cargo_test_support::project()
        .file("script.rs/foo", "something")
        .build();

    p.cargo("-Zscript -v script.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_status(101)
        .with_stderr(
            "\
error: manifest path `script.rs` is a directory but expected a file
",
        )
        .run();
}

#[cargo_test]
fn missing_script_rs() {
    let p = cargo_test_support::project().build();

    p.cargo("-Zscript -v script.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_status(101)
        .with_stderr(
            "\
[ERROR] manifest path `script.rs` does not exist
",
        )
        .run();
}

#[cargo_test]
fn test_name_same_as_dependency() {
    Package::new("script", "1.0.0").publish();
    let script = r#"#!/usr/bin/env cargo
```cargo
[dependencies]
script = "1.0.0"
```

fn main() {
    println!("Hello world!");
}"#;
    let p = cargo_test_support::project()
        .file("script.rs", script)
        .build();

    p.cargo("-Zscript -v script.rs --help")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout(
            r#"Hello world!
"#,
        )
        .with_stderr(
            "\
[WARNING] `package.edition` is unspecified, defaulting to `2021`
[UPDATING] `dummy-registry` index
[DOWNLOADING] crates ...
[DOWNLOADED] script v1.0.0 (registry `dummy-registry`)
[COMPILING] script v1.0.0
[COMPILING] script v0.0.0 ([ROOT]/foo)
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
[RUNNING] `[..]/debug/script[EXE] --help`
",
        )
        .run();
}

#[cargo_test]
fn test_path_dep() {
    let script = r#"#!/usr/bin/env cargo
```cargo
[dependencies]
bar.path = "./bar"
```

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
        .with_stdout(
            r#"Hello world!
"#,
        )
        .with_stderr(
            "\
[WARNING] `package.edition` is unspecified, defaulting to `2021`
[COMPILING] bar v0.0.1 ([ROOT]/foo/bar)
[COMPILING] script v0.0.0 ([ROOT]/foo)
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
[RUNNING] `[..]/debug/script[EXE] --help`
",
        )
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
        .with_stdout(
            r#"Hello world!
"#,
        )
        .with_stderr(
            "\
[WARNING] `package.edition` is unspecified, defaulting to `2021`
[COMPILING] script v0.0.0 ([ROOT]/foo)
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
[RUNNING] `[..]/debug/script[EXE] --help`
",
        )
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
        .with_stdout(
            r#"Hello world!
"#,
        )
        .with_stderr(
            "\
[WARNING] `package.edition` is unspecified, defaulting to `2021`
[COMPILING] script v0.0.0 ([ROOT]/foo)
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
[RUNNING] `[..]/debug/script[EXE] --help`
",
        )
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
        .with_stdout(
            r#"bin: [ROOT]/home/.cargo/target/[..]/debug/script[EXE]
args: []
"#,
        )
        .with_stderr(
            "\
[WARNING] `package.edition` is unspecified, defaulting to `2021`
[COMPILING] script v0.0.0 ([ROOT]/foo)
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
[RUNNING] `[ROOT]/home/.cargo/target/[..]/debug/script[EXE]`
",
        )
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
        .with_stdout(
            r#"bin: [ROOT]/home/.cargo/target/[..]/debug/script[EXE]
args: []
"#,
        )
        .with_stderr(
            "\
[WARNING] `package.edition` is unspecified, defaulting to `2021`
[COMPILING] script v0.0.0 ([ROOT]/foo)
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
[RUNNING] `[ROOT]/home/.cargo/target/[..]/debug/script[EXE]`
",
        )
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
        .with_stdout("")
        .with_stderr(
            "\
error: embedded manifest `[ROOT]/foo/script.rs` requires `-Zscript`
",
        )
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
        .with_stdout("")
        .with_stderr(
            "\
error: embedded manifest `[ROOT]/foo/script.rs` requires `-Zscript`
",
        )
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
        .with_stdout(
            "\
",
        )
        .with_stderr(
            "\
[WARNING] `package.edition` is unspecified, defaulting to `2021`
[CHECKING] script v0.0.0 ([ROOT]/foo)
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
",
        )
        .run();
}

#[cargo_test]
fn cmd_check_with_missing_script_rs() {
    let p = cargo_test_support::project().build();

    p.cargo("-Zscript check --manifest-path script.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_status(101)
        .with_stdout(
            "\
",
        )
        .with_stderr(
            "\
[ERROR] manifest path `script.rs` does not exist
",
        )
        .run();
}

#[cargo_test]
fn cmd_check_with_missing_script() {
    let p = cargo_test_support::project().build();

    p.cargo("-Zscript check --manifest-path script")
        .masquerade_as_nightly_cargo(&["script"])
        .with_status(101)
        .with_stdout(
            "\
",
        )
        .with_stderr(
            "\
[ERROR] the manifest-path must be a path to a Cargo.toml file
",
        )
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
        .with_stdout(
            "\
",
        )
        .with_stderr(
            "\
[WARNING] `package.edition` is unspecified, defaulting to `2021`
[COMPILING] script v0.0.0 ([ROOT]/foo)
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
",
        )
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
        .with_stdout(
            "
running 1 test
test test ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in [..]s

",
        )
        .with_stderr(
            "\
[WARNING] `package.edition` is unspecified, defaulting to `2021`
[COMPILING] script v0.0.0 ([ROOT]/foo)
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]s
[RUNNING] unittests script.rs ([..])
",
        )
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
        .with_stdout(
            "\
",
        )
        .with_stderr(
            "\
[WARNING] `package.edition` is unspecified, defaulting to `2021`
[REMOVED] [..] files, [..] total
",
        )
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
        .with_stdout(
            "\
",
        )
        .with_stderr(
            "\
[WARNING] `package.edition` is unspecified, defaulting to `2021`
",
        )
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
        .with_json(
            r#"
    {
        "packages": [
            {
                "authors": [
                ],
                "categories": [],
                "default_run": null,
                "name": "script",
                "version": "0.0.0",
                "id": "script[..]",
                "keywords": [],
                "source": null,
                "dependencies": [],
                "edition": "[..]",
                "license": null,
                "license_file": null,
                "links": null,
                "description": null,
                "readme": null,
                "repository": null,
                "rust_version": null,
                "homepage": null,
                "documentation": null,
                "homepage": null,
                "documentation": null,
                "targets": [
                    {
                        "kind": [
                            "bin"
                        ],
                        "crate_types": [
                            "bin"
                        ],
                        "doc": true,
                        "doctest": false,
                        "test": true,
                        "edition": "[..]",
                        "name": "script",
                        "src_path": "[..]/script.rs"
                    }
                ],
                "features": {},
                "manifest_path": "[..]script.rs",
                "metadata": null,
                "publish": []
            }
        ],
        "workspace_members": ["script 0.0.0 (path+file:[..]foo)"],
        "workspace_default_members": ["script 0.0.0 (path+file:[..]foo)"],
        "resolve": {
            "nodes": [
                {
                    "dependencies": [],
                    "deps": [],
                    "features": [],
                    "id": "script 0.0.0 (path+file:[..]foo)"
                }
            ],
            "root": "script 0.0.0 (path+file:[..]foo)"
        },
        "target_directory": "[ROOT]/home/.cargo/target/[..]",
        "version": 1,
        "workspace_root": "[..]/foo",
        "metadata": null
    }"#,
        )
        .with_stderr(
            "\
[WARNING] `package.edition` is unspecified, defaulting to `2021`
",
        )
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
        .with_json(
            r#"
{
    "authors": [
    ],
    "categories": [],
    "default_run": null,
    "name":"script",
    "readme": null,
    "homepage": null,
    "documentation": null,
    "repository": null,
    "rust_version": null,
    "version":"0.0.0",
    "id":"script[..]0.0.0[..](path+file://[..]/foo)",
    "keywords": [],
    "license": null,
    "license_file": null,
    "links": null,
    "description": null,
    "edition": "[..]",
    "source":null,
    "dependencies":[],
    "targets":[{
        "kind":["bin"],
        "crate_types":["bin"],
        "doc": true,
        "doctest": false,
        "test": true,
        "edition": "[..]",
        "name":"script",
        "src_path":"[..]/script.rs"
    }],
    "features":{},
    "manifest_path":"[..]script.rs",
    "metadata": null,
    "publish": []
}"#,
        )
        .with_stderr(
            "\
[WARNING] `package.edition` is unspecified, defaulting to `2021`
",
        )
        .run();
}

#[cargo_test]
fn cmd_run_with_embedded() {
    let p = cargo_test_support::project()
        .file("script.rs", ECHO_SCRIPT)
        .build();

    p.cargo("-Zscript run --manifest-path script.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout(
            r#"bin: [..]/debug/script[EXE]
args: []
"#,
        )
        .with_stderr(
            "\
[WARNING] `package.edition` is unspecified, defaulting to `2021`
[COMPILING] script v0.0.0 ([ROOT]/foo)
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
[RUNNING] `[..]/debug/script[EXE]`
",
        )
        .run();
}

#[cargo_test]
fn cmd_tree_with_embedded() {
    let p = cargo_test_support::project()
        .file("script.rs", ECHO_SCRIPT)
        .build();

    p.cargo("-Zscript tree --manifest-path script.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout(
            "\
script v0.0.0 ([ROOT]/foo)
",
        )
        .with_stderr(
            "\
[WARNING] `package.edition` is unspecified, defaulting to `2021`
",
        )
        .run();
}

#[cargo_test]
fn cmd_update_with_embedded() {
    let p = cargo_test_support::project()
        .file("script.rs", ECHO_SCRIPT)
        .build();

    p.cargo("-Zscript update --manifest-path script.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_stdout(
            "\
",
        )
        .with_stderr(
            "\
[WARNING] `package.edition` is unspecified, defaulting to `2021`
",
        )
        .run();
}

#[cargo_test]
fn cmd_verify_project_with_embedded() {
    let p = cargo_test_support::project()
        .file("script.rs", ECHO_SCRIPT)
        .build();

    p.cargo("-Zscript verify-project --manifest-path script.rs")
        .masquerade_as_nightly_cargo(&["script"])
        .with_json(r#"{"success":"true"}"#)
        .with_stderr(
            "\
[WARNING] `package.edition` is unspecified, defaulting to `2021`
",
        )
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
        .with_stderr(
            "\
[WARNING] `package.edition` is unspecified, defaulting to `2021`
[ERROR] [ROOT]/foo/script.rs is unsupported by `cargo pkgid`
",
        )
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
        .with_stderr(
            "\
[WARNING] `package.edition` is unspecified, defaulting to `2021`
[ERROR] [ROOT]/foo/script.rs is unsupported by `cargo package`
",
        )
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
        .with_stderr(
            "\
[WARNING] `package.edition` is unspecified, defaulting to `2021`
[ERROR] [ROOT]/foo/script.rs is unsupported by `cargo publish`
",
        )
        .run();
}
