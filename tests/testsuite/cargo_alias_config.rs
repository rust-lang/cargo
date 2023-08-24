//! Tests for `[alias]` config command aliases.

use std::env;

use cargo_test_support::tools::echo_subcommand;
use cargo_test_support::{basic_bin_manifest, project};

#[cargo_test]
fn alias_incorrect_config_type() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", "fn main() {}")
        .file(
            ".cargo/config",
            r#"
                [alias]
                b-cargo-test = 5
            "#,
        )
        .build();

    p.cargo("b-cargo-test -v")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] invalid configuration for key `alias.b-cargo-test`
expected a list, but found a integer for [..]",
        )
        .run();
}

#[cargo_test]
fn alias_malformed_config_string() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", "fn main() {}")
        .file(
            ".cargo/config",
            r#"
                [alias]
                b-cargo-test = `
            "#,
        )
        .build();

    p.cargo("b-cargo-test -v")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] could not load Cargo configuration

Caused by:
  could not parse TOML configuration in `[..]/config`

Caused by:
  TOML parse error at line [..]
    |
  3 |                 b-cargo-test = `
    |                                ^
  invalid string
  expected `\"`, `'`
",
        )
        .run();
}

#[cargo_test]
fn alias_malformed_config_list() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", "fn main() {}")
        .file(
            ".cargo/config",
            r#"
                [alias]
                b-cargo-test = [1, 2]
            "#,
        )
        .build();

    p.cargo("b-cargo-test -v")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] could not load Cargo configuration

Caused by:
  failed to load TOML configuration from `[..]/config`

Caused by:
  [..] `alias`

Caused by:
  [..] `b-cargo-test`

Caused by:
  expected string but found integer in list
",
        )
        .run();
}

#[cargo_test]
fn alias_config() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", "fn main() {}")
        .file(
            ".cargo/config",
            r#"
                [alias]
                b-cargo-test = "build"
            "#,
        )
        .build();

    p.cargo("b-cargo-test -v")
        .with_stderr_contains(
            "\
[COMPILING] foo v0.5.0 [..]
[RUNNING] `rustc --crate-name foo [..]",
        )
        .run();
}

#[cargo_test]
fn dependent_alias() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", "fn main() {}")
        .file(
            ".cargo/config",
            r#"
                [alias]
                b-cargo-test = "build"
                a-cargo-test = ["b-cargo-test", "-v"]
            "#,
        )
        .build();

    p.cargo("a-cargo-test")
        .with_stderr_contains(
            "\
[COMPILING] foo v0.5.0 [..]
[RUNNING] `rustc --crate-name foo [..]",
        )
        .run();
}

#[cargo_test]
fn builtin_alias_shadowing_external_subcommand() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", "fn main() {}")
        .executable("cargo-t", "")
        .build();

    let mut paths: Vec<_> = env::split_paths(&env::var_os("PATH").unwrap_or_default()).collect();
    paths.push(p.root());
    let path = env::join_paths(paths).unwrap();

    p.cargo("t")
        .env("PATH", &path)
        .with_stderr(
            "\
[COMPILING] foo v0.5.0 [..]
[FINISHED] test [unoptimized + debuginfo] target(s) in [..]
[RUNNING] unittests src/main.rs [..]
",
        )
        .run();
}

#[cargo_test]
fn alias_shadowing_external_subcommand() {
    let echo = echo_subcommand();
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", "fn main() {}")
        .file(
            ".cargo/config",
            r#"
                [alias]
                echo = "build"
            "#,
        )
        .build();

    let mut paths: Vec<_> = env::split_paths(&env::var_os("PATH").unwrap_or_default()).collect();
    paths.push(echo.target_debug_dir());
    let path = env::join_paths(paths).unwrap();

    p.cargo("echo")
        .env("PATH", &path)
        .with_stderr("\
[WARNING] user-defined alias `echo` is shadowing an external subcommand found at: `[ROOT]/cargo-echo/target/debug/cargo-echo[EXE]`
This was previously accepted but is being phased out; it will become a hard error in a future release.
For more information, see issue #10049 <https://github.com/rust-lang/cargo/issues/10049>.
[COMPILING] foo v0.5.0 [..]
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cargo_test]
fn default_args_alias() {
    let echo = echo_subcommand();
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", "fn main() {}")
        .file(
            ".cargo/config",
            r#"
                [alias]
                echo = "echo --flag1 --flag2"
                test-1 = "echo"
                build = "build --verbose"
            "#,
        )
        .build();

    let mut paths: Vec<_> = env::split_paths(&env::var_os("PATH").unwrap_or_default()).collect();
    paths.push(echo.target_debug_dir());
    let path = env::join_paths(paths).unwrap();

    p.cargo("echo")
        .env("PATH", &path)
        .with_status(101)
        .with_stderr("\
[WARNING] user-defined alias `echo` is shadowing an external subcommand found at: `[ROOT]/cargo-echo/target/debug/cargo-echo[EXE]`
This was previously accepted but is being phased out; it will become a hard error in a future release.
For more information, see issue #10049 <https://github.com/rust-lang/cargo/issues/10049>.
error: alias echo has unresolvable recursive definition: echo -> echo
",
        )
        .run();

    p.cargo("test-1")
        .env("PATH", &path)
        .with_status(101)
        .with_stderr("\
[WARNING] user-defined alias `echo` is shadowing an external subcommand found at: `[ROOT]/cargo-echo/target/debug/cargo-echo[EXE]`
This was previously accepted but is being phased out; it will become a hard error in a future release.
For more information, see issue #10049 <https://github.com/rust-lang/cargo/issues/10049>.
error: alias test-1 has unresolvable recursive definition: test-1 -> echo -> echo
",
        )
        .run();

    // Builtins are not expanded by rule
    p.cargo("build")
        .with_stderr(
            "\
[WARNING] user-defined alias `build` is ignored, because it is shadowed by a built-in command
[COMPILING] foo v0.5.0 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cargo_test]
fn corecursive_alias() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", "fn main() {}")
        .file(
            ".cargo/config",
            r#"
                [alias]
                test-1 = "test-2 --flag1"
                test-2 = "test-3 --flag2"
                test-3 = "test-1 --flag3"
            "#,
        )
        .build();

    p.cargo("test-1")
        .with_status(101)
        .with_stderr(
            "error: alias test-1 has unresolvable recursive definition: test-1 -> test-2 -> test-3 -> test-1",
        )
        .run();

    p.cargo("test-2")
        .with_status(101)
        .with_stderr(
            "error: alias test-2 has unresolvable recursive definition: test-2 -> test-3 -> test-1 -> test-2",
        )
        .run();
}

#[cargo_test]
fn alias_list_test() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", "fn main() {}")
        .file(
            ".cargo/config",
            r#"
               [alias]
               b-cargo-test = ["build", "--release"]
            "#,
        )
        .build();

    p.cargo("b-cargo-test -v")
        .with_stderr_contains("[COMPILING] foo v0.5.0 [..]")
        .with_stderr_contains("[RUNNING] `rustc --crate-name [..]")
        .run();
}

#[cargo_test]
fn alias_with_flags_config() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", "fn main() {}")
        .file(
            ".cargo/config",
            r#"
               [alias]
               b-cargo-test = "build --release"
            "#,
        )
        .build();

    p.cargo("b-cargo-test -v")
        .with_stderr_contains("[COMPILING] foo v0.5.0 [..]")
        .with_stderr_contains("[RUNNING] `rustc --crate-name foo [..]")
        .run();
}

#[cargo_test]
fn alias_cannot_shadow_builtin_command() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", "fn main() {}")
        .file(
            ".cargo/config",
            r#"
               [alias]
               build = "fetch"
            "#,
        )
        .build();

    p.cargo("build")
        .with_stderr(
            "\
[WARNING] user-defined alias `build` is ignored, because it is shadowed by a built-in command
[COMPILING] foo v0.5.0 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cargo_test]
fn alias_override_builtin_alias() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", "fn main() {}")
        .file(
            ".cargo/config",
            r#"
               [alias]
               b = "run"
            "#,
        )
        .build();

    p.cargo("b")
        .with_stderr(
            "\
[COMPILING] foo v0.5.0 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] `target/debug/foo[EXE]`
",
        )
        .run();
}

#[cargo_test]
fn builtin_alias_takes_options() {
    // #6381
    let p = project()
        .file("src/lib.rs", "")
        .file(
            "examples/ex1.rs",
            r#"fn main() { println!("{}", std::env::args().skip(1).next().unwrap()) }"#,
        )
        .build();

    p.cargo("r --example ex1 -- asdf").with_stdout("asdf").run();
}

#[cargo_test]
fn global_options_with_alias() {
    // Check that global options are passed through.
    let p = project().file("src/lib.rs", "").build();

    p.cargo("-v c")
        .with_stderr(
            "\
[CHECKING] foo [..]
[RUNNING] `rustc [..]
[FINISHED] dev [..]
",
        )
        .run();
}

#[cargo_test]
fn weird_check() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("-- check --invalid_argument -some-other-argument")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] trailing arguments after built-in command `check` are unsupported: `--invalid_argument -some-other-argument`

To pass the arguments to the subcommand, remove `--`
",
        )
        .run();
}
