const ECHO_SCRIPT: &str = r#"#!/usr/bin/env cargo

fn main() {
    let mut args = std::env::args_os();
    let bin = args.next().unwrap().to_str().unwrap().to_owned();
    let args = args.collect::<Vec<_>>();
    println!("bin: {bin}");
    println!("args: {args:?}");
}
"#;

fn path() -> Vec<std::path::PathBuf> {
    std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()).collect()
}

#[cargo_test]
fn basic_rs() {
    let p = cargo_test_support::project()
        .file("echo.rs", ECHO_SCRIPT)
        .build();

    p.cargo("-Zscript echo.rs")
        .arg("--help") // An arg that, if processed by cargo, will cause problems
        .masquerade_as_nightly_cargo(&["script"])
        .with_status(101)
        .with_stdout("")
        .with_stderr("\
thread 'main' panicked at 'not yet implemented: support for running manifest-commands is not yet implemented', [..]
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
")
        .run();
}

#[cargo_test]
fn basic_path() {
    let p = cargo_test_support::project()
        .file("echo", ECHO_SCRIPT)
        .build();

    p.cargo("-Zscript ./echo")
        .arg("--help") // An arg that, if processed by cargo, will cause problems
        .masquerade_as_nightly_cargo(&["script"])
        .with_status(101)
        .with_stdout("")
        .with_stderr("\
thread 'main' panicked at 'not yet implemented: support for running manifest-commands is not yet implemented', [..]
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
")
        .run();
}

#[cargo_test]
fn path_required() {
    let p = cargo_test_support::project()
        .file("echo", ECHO_SCRIPT)
        .build();

    p.cargo("-Zscript echo")
        .arg("--help") // An arg that, if processed by cargo, will cause problems
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
        .arg("--help") // An arg that, if processed by cargo, will cause problems
        .env("PATH", &path)
        .masquerade_as_nightly_cargo(&["script"])
        .with_status(101)
        .with_stdout("")
        .with_stderr("\
thread 'main' panicked at 'not yet implemented: support for running manifest-commands is not yet implemented', [..]
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
")
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
        .arg("--help") // An arg that, if processed by cargo, will cause problems
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
        .arg("--help") // An arg that, if processed by cargo, will cause problems
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
        .arg("--help") // An arg that, if processed by cargo, will cause problems
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
