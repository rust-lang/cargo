//! Tests for `[alias]` config command aliases.

use std::env;

use crate::prelude::*;
use crate::utils::tools::echo_subcommand;
use cargo_test_support::str;
use cargo_test_support::{basic_bin_manifest, project};

#[cargo_test]
fn alias_incorrect_config_type() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", "fn main() {}")
        .file(
            ".cargo/config.toml",
            r#"
                [alias]
                b-cargo-test = 5
            "#,
        )
        .build();

    p.cargo("b-cargo-test -v")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] invalid configuration for key `alias.b-cargo-test`
expected a list, but found a integer for `alias.b-cargo-test` in [ROOT]/foo/.cargo/config.toml

"#]])
        .run();
}

#[cargo_test]
fn alias_malformed_config_string() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", "fn main() {}")
        .file(
            ".cargo/config.toml",
            r#"
                [alias]
                b-cargo-test = `
            "#,
        )
        .build();

    p.cargo("b-cargo-test -v")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] could not load Cargo configuration

Caused by:
  could not parse TOML configuration in `[ROOT]/foo/.cargo/config.toml`

Caused by:
  TOML parse error at line 3, column 32
    |
  3 |                 b-cargo-test = `
    |                                ^
  string values must be quoted, expected literal string

"#]])
        .run();
}

#[cargo_test]
fn alias_malformed_config_list() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", "fn main() {}")
        .file(
            ".cargo/config.toml",
            r#"
                [alias]
                b-cargo-test = [1, 2]
            "#,
        )
        .build();

    p.cargo("b-cargo-test -v")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] error in [ROOT]/foo/.cargo/config.toml: failed to parse config at `alias.b-cargo-test[0]`

Caused by:
  invalid type: integer `1`, expected a string

"#]])
        .run();
}

#[cargo_test]
fn alias_config() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", "fn main() {}")
        .file(
            ".cargo/config.toml",
            r#"
                [alias]
                b-cargo-test = "build"
            "#,
        )
        .build();

    p.cargo("b-cargo-test -v")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.5.0 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo [..]
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn dependent_alias() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", "fn main() {}")
        .file(
            ".cargo/config.toml",
            r#"
                [alias]
                b-cargo-test = "build"
                a-cargo-test = ["b-cargo-test", "-v"]
            "#,
        )
        .build();

    p.cargo("a-cargo-test")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.5.0 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo [..]
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
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
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.5.0 ([ROOT]/foo)
[FINISHED] `test` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] unittests src/main.rs (target/debug/deps/foo-[HASH][EXE])

"#]])
        .run();
}

#[cargo_test]
fn alias_shadowing_external_subcommand() {
    let echo = echo_subcommand();
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", "fn main() {}")
        .file(
            ".cargo/config.toml",
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
        .with_stderr_data(str![[r#"
[WARNING] user-defined alias `echo` is shadowing an external subcommand found at `[ROOT]/cargo-echo/target/debug/cargo-echo[EXE]`
  |
  = [NOTE] this was previously accepted but will become a hard error in the future; see <https://github.com/rust-lang/cargo/issues/10049>
[COMPILING] foo v0.5.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn default_args_alias() {
    let echo = echo_subcommand();
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", "fn main() {}")
        .file(
            ".cargo/config.toml",
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
        .with_stderr_data(str![[r#"
[WARNING] user-defined alias `echo` is shadowing an external subcommand found at `[ROOT]/cargo-echo/target/debug/cargo-echo[EXE]`
  |
  = [NOTE] this was previously accepted but will become a hard error in the future; see <https://github.com/rust-lang/cargo/issues/10049>
[ERROR] alias echo has unresolvable recursive definition: echo -> echo

"#]])
        .run();

    p.cargo("test-1")
        .env("PATH", &path)
        .with_status(101)
        .with_stderr_data(str![[r#"
[WARNING] user-defined alias `echo` is shadowing an external subcommand found at `[ROOT]/cargo-echo/target/debug/cargo-echo[EXE]`
  |
  = [NOTE] this was previously accepted but will become a hard error in the future; see <https://github.com/rust-lang/cargo/issues/10049>
[ERROR] alias test-1 has unresolvable recursive definition: test-1 -> echo -> echo

"#]])
        .run();

    // Builtins are not expanded by rule
    p.cargo("build")
        .with_stderr_data(str![[r#"
[WARNING] user-defined alias `build` is ignored, because it is shadowed by a built-in command
[COMPILING] foo v0.5.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn corecursive_alias() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", "fn main() {}")
        .file(
            ".cargo/config.toml",
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
        .with_stderr_data(str![[r#"
[ERROR] alias test-1 has unresolvable recursive definition: test-1 -> test-2 -> test-3 -> test-1

"#]])
        .run();

    p.cargo("test-2")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] alias test-2 has unresolvable recursive definition: test-2 -> test-3 -> test-1 -> test-2

"#]])
        .run();
}

#[cargo_test]
fn alias_list_test() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", "fn main() {}")
        .file(
            ".cargo/config.toml",
            r#"
               [alias]
               b-cargo-test = ["build", "--release"]
            "#,
        )
        .build();

    p.cargo("b-cargo-test -v")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.5.0 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo [..]
[FINISHED] `release` profile [optimized] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn alias_with_flags_config() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", "fn main() {}")
        .file(
            ".cargo/config.toml",
            r#"
               [alias]
               b-cargo-test = "build --release"
            "#,
        )
        .build();

    p.cargo("b-cargo-test -v")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.5.0 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo [..]
[FINISHED] `release` profile [optimized] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn alias_cannot_shadow_builtin_command() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", "fn main() {}")
        .file(
            ".cargo/config.toml",
            r#"
               [alias]
               build = "fetch"
            "#,
        )
        .build();

    p.cargo("build")
        .with_stderr_data(str![[r#"
[WARNING] user-defined alias `build` is ignored, because it is shadowed by a built-in command
[COMPILING] foo v0.5.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn alias_override_builtin_alias() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", "fn main() {}")
        .file(
            ".cargo/config.toml",
            r#"
               [alias]
               b = "run"
            "#,
        )
        .build();

    p.cargo("b")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.5.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `target/debug/foo[EXE]`

"#]])
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

    p.cargo("r --example ex1 -- asdf")
        .with_stdout_data(str![[r#"
asdf

"#]])
        .run();
}

#[cargo_test]
fn global_options_with_alias() {
    // Check that global options are passed through.
    let p = project().file("src/lib.rs", "").build();

    p.cargo("-v c")
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo --edition=2015 src/lib.rs [..]
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
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
        .with_stderr_data(str![[r#"
[ERROR] trailing arguments after built-in command `check` are unsupported: `--invalid_argument -some-other-argument`

To pass the arguments to the subcommand, remove `--`

"#]])
        .run();
}

#[cargo_test]
fn empty_alias() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", "fn main() {}")
        .file(
            ".cargo/config.toml",
            r#"
               [alias]
               string = ""
               array = []
            "#,
        )
        .build();

    p.cargo("string")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] subcommand is required, but `alias.string` is empty

"#]])
        .run();

    p.cargo("array")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] subcommand is required, but `alias.array` is empty

"#]])
        .run();
}

#[cargo_test]
fn alias_no_subcommand() {
    let p = project()
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", "fn main() {}")
        .file(
            ".cargo/config.toml",
            r#"
               [alias]
               a = "--locked"
            "#,
        )
        .build();

    p.cargo("a")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] subcommand is required, add a subcommand to the command alias `alias.a`

"#]])
        .run();
}
