const ECHO_SCRIPT: &str = r#"#!/usr/bin/env cargo

fn main() {
    let mut args = std::env::args_os();
    let bin = args.next().unwrap().to_str().unwrap().to_owned();
    let args = args.collect::<Vec<_>>();
    println!("bin: {bin}");
    println!("args: {args:?}");
}
"#;

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
