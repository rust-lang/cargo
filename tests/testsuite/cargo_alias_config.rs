//! Tests for `[alias]` config command aliases.

use cargo_test_support::{basic_bin_manifest, project, rustc_host};

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
        .with_stderr_contains(
            "\
[ERROR] invalid configuration for key `alias.b-cargo-test`
expected a list, but found a integer for [..]",
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
fn recursive_alias() {
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
        .with_stderr(&format!(
            "\
[COMPILING] foo v0.5.0 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[RUNNING] `target/{}/debug/foo[EXE]`
",
            rustc_host()
        ))
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
        .with_stderr(
            "\
[WARNING] trailing arguments after built-in command `check` are ignored: `--invalid_argument -some-other-argument`
[CHECKING] foo v0.5.0 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}
