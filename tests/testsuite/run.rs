//! Tests for the `cargo run` command.

use crate::prelude::*;
use cargo_test_support::{
    Project, basic_bin_manifest, basic_lib_manifest, basic_manifest, project, str,
};
use cargo_util::paths::dylib_path_envvar;

#[cargo_test]
fn simple() {
    let p = project()
        .file("src/main.rs", r#"fn main() { println!("hello"); }"#)
        .build();

    p.cargo("run")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `target/debug/foo[EXE]`

"#]])
        .with_stdout_data(str![[r#"
hello

"#]])
        .run();
    assert!(p.bin("foo").is_file());
}

#[cargo_test]
fn quiet_arg() {
    let p = project()
        .file("src/main.rs", r#"fn main() { println!("hello"); }"#)
        .build();

    p.cargo("run -q")
        .with_stderr_data("")
        .with_stdout_data(str![[r#"
hello

"#]])
        .run();

    p.cargo("run --quiet")
        .with_stderr_data("")
        .with_stdout_data(str![[r#"
hello

"#]])
        .run();
}

#[cargo_test]
fn unsupported_silent_arg() {
    let p = project()
        .file("src/main.rs", r#"fn main() { println!("hello"); }"#)
        .build();

    p.cargo("run -s")
        .with_stderr_data(str![[r#"
[ERROR] unexpected argument '--silent' found

  tip: a similar argument exists: '--quiet'

Usage: cargo[EXE] run [OPTIONS] [ARGS]...

For more information, try '--help'.

"#]])
        .with_status(1)
        .run();

    p.cargo("run --silent")
        .with_stderr_data(str![[r#"
[ERROR] unexpected argument '--silent' found

  tip: a similar argument exists: '--quiet'

Usage: cargo[EXE] run [OPTIONS] [ARGS]...

For more information, try '--help'.

"#]])
        .with_status(1)
        .run();
}

#[cargo_test]
fn quiet_arg_and_verbose_arg() {
    let p = project()
        .file("src/main.rs", r#"fn main() { println!("hello"); }"#)
        .build();

    p.cargo("run -q -v")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] cannot set both --verbose and --quiet

"#]])
        .run();
}

#[cargo_test]
fn quiet_arg_and_verbose_config() {
    let p = project()
        .file(
            ".cargo/config.toml",
            r#"
                [term]
                verbose = true
            "#,
        )
        .file("src/main.rs", r#"fn main() { println!("hello"); }"#)
        .build();

    p.cargo("run -q")
        .with_stderr_data("")
        .with_stdout_data(str![[r#"
hello

"#]])
        .run();
}

#[cargo_test]
fn verbose_arg_and_quiet_config() {
    let p = project()
        .file(
            ".cargo/config.toml",
            r#"
                [term]
                quiet = true
            "#,
        )
        .file("src/main.rs", r#"fn main() { println!("hello"); }"#)
        .build();

    p.cargo("run -v")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `target/debug/foo[EXE]`

"#]])
        .with_stdout_data(str![[r#"
hello

"#]])
        .run();
}

#[cargo_test]
fn quiet_config_alone() {
    let p = project()
        .file(
            ".cargo/config.toml",
            r#"
                [term]
                quiet = true
            "#,
        )
        .file("src/main.rs", r#"fn main() { println!("hello"); }"#)
        .build();

    p.cargo("run")
        .with_stderr_data("")
        .with_stdout_data(str![[r#"
hello

"#]])
        .run();
}

#[cargo_test]
fn verbose_config_alone() {
    let p = project()
        .file(
            ".cargo/config.toml",
            r#"
                [term]
                verbose = true
            "#,
        )
        .file("src/main.rs", r#"fn main() { println!("hello"); }"#)
        .build();

    p.cargo("run")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `target/debug/foo[EXE]`

"#]])
        .with_stdout_data(str![[r#"
hello

"#]])
        .run();
}

#[cargo_test]
fn quiet_config_and_verbose_config() {
    let p = project()
        .file(
            ".cargo/config.toml",
            r#"
                [term]
                verbose = true
                quiet = true
            "#,
        )
        .file("src/main.rs", r#"fn main() { println!("hello"); }"#)
        .build();

    p.cargo("run")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] cannot set both `term.verbose` and `term.quiet`

"#]])
        .run();
}

#[cargo_test]
fn simple_with_args() {
    let p = project()
        .file(
            "src/main.rs",
            r#"
                fn main() {
                    assert_eq!(std::env::args().nth(1).unwrap(), "hello");
                    assert_eq!(std::env::args().nth(2).unwrap(), "world");
                }
            "#,
        )
        .build();

    p.cargo("run hello world").run();
}

#[cfg(unix)]
#[cargo_test]
fn simple_with_non_utf8_args() {
    use std::os::unix::ffi::OsStrExt;

    let p = project()
        .file(
            "src/main.rs",
            r#"
                use std::ffi::OsStr;
                use std::os::unix::ffi::OsStrExt;

                fn main() {
                    assert_eq!(std::env::args_os().nth(1).unwrap(), OsStr::from_bytes(b"hello"));
                    assert_eq!(std::env::args_os().nth(2).unwrap(), OsStr::from_bytes(b"ab\xffcd"));
                }
            "#,
        )
        .build();

    p.cargo("run")
        .arg("hello")
        .arg(std::ffi::OsStr::from_bytes(b"ab\xFFcd"))
        .run();
}

#[cargo_test]
fn exit_code() {
    let p = project()
        .file("src/main.rs", "fn main() { std::process::exit(2); }")
        .build();

    let expected = if !cfg!(unix) {
        str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `target/debug/foo[EXE]`
[ERROR] process didn't exit successfully: `target/debug/foo[EXE]` ([EXIT_STATUS]: 2)

"#]]
    } else {
        str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `target/debug/foo`

"#]]
    };
    p.cargo("run")
        .with_status(2)
        .with_stderr_data(expected)
        .run();
}

#[cargo_test]
fn exit_code_verbose() {
    let p = project()
        .file("src/main.rs", "fn main() { std::process::exit(2); }")
        .build();

    let expected = if !cfg!(unix) {
        str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `target/debug/foo[EXE]`
[ERROR] process didn't exit successfully: `target/debug/foo[EXE]` ([EXIT_STATUS]: 2)

"#]]
    } else {
        str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `target/debug/foo[EXE]`

"#]]
    };

    p.cargo("run -v")
        .with_status(2)
        .with_stderr_data(expected)
        .run();
}

#[cargo_test]
fn no_main_file() {
    let p = project().file("src/lib.rs", "").build();

    p.cargo("run")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] a bin target must be available for `cargo run`

"#]])
        .run();
}

#[cargo_test]
fn too_many_bins() {
    let p = project()
        .file("src/lib.rs", "")
        .file("src/bin/a.rs", "")
        .file("src/bin/b.rs", "")
        .build();

    // Using [..] here because the order is not stable
    p.cargo("run")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] `cargo run` could not determine which binary to run. Use the `--bin` option to specify a binary, or the `default-run` manifest key.
available binaries: a, b

"#]])
        .run();
}

#[cargo_test]
fn specify_name() {
    let p = project()
        .file("src/lib.rs", "")
        .file(
            "src/bin/a.rs",
            r#"
                #[allow(unused_extern_crates)]
                extern crate foo;
                fn main() { println!("hello a.rs"); }
            "#,
        )
        .file(
            "src/bin/b.rs",
            r#"
                #[allow(unused_extern_crates)]
                extern crate foo;
                fn main() { println!("hello b.rs"); }
            "#,
        )
        .build();

    p.cargo("run --bin a -v")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc [..] src/lib.rs [..]`
[RUNNING] `rustc [..] src/bin/a.rs [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `target/debug/a[EXE]`

"#]])
        .with_stdout_data(str![[r#"
hello a.rs

"#]])
        .run();

    p.cargo("run --bin b -v")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc [..] src/bin/b.rs [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `target/debug/b[EXE]`

"#]])
        .with_stdout_data(str![[r#"
hello b.rs

"#]])
        .run();
}

#[cargo_test]
fn specify_default_run() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []
                default-run = "a"
            "#,
        )
        .file("src/lib.rs", "")
        .file("src/bin/a.rs", r#"fn main() { println!("hello A"); }"#)
        .file("src/bin/b.rs", r#"fn main() { println!("hello B"); }"#)
        .build();

    p.cargo("run")
        .with_stdout_data(str![[r#"
hello A

"#]])
        .run();
    p.cargo("run --bin a")
        .with_stdout_data(str![[r#"
hello A

"#]])
        .run();
    p.cargo("run --bin b")
        .with_stdout_data(str![[r#"
hello B

"#]])
        .run();
}

#[cargo_test]
fn bogus_default_run() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []
                default-run = "b"
            "#,
        )
        .file("src/lib.rs", "")
        .file("src/bin/a.rs", r#"fn main() { println!("hello A"); }"#)
        .build();

    p.cargo("run")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  default-run target `b` not found

  [HELP] a target with a similar name exists: `a`

"#]])
        .run();
}

#[cargo_test]
fn run_example() {
    let p = project()
        .file("src/lib.rs", "")
        .file("examples/a.rs", r#"fn main() { println!("example"); }"#)
        .file("src/bin/a.rs", r#"fn main() { println!("bin"); }"#)
        .build();

    p.cargo("run --example a")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `target/debug/examples/a[EXE]`

"#]])
        .with_stdout_data(str![[r#"
example

"#]])
        .run();
}

#[cargo_test]
fn run_library_example() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []
                [[example]]
                name = "bar"
                crate-type = ["lib"]
            "#,
        )
        .file("src/lib.rs", "")
        .file("examples/bar.rs", "fn foo() {}")
        .build();

    p.cargo("run --example bar")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] example target `bar` is a library and cannot be executed

"#]])
        .run();
}

#[cargo_test]
fn run_bin_example() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                [[example]]
                name = "bar"
                crate-type = ["bin"]
            "#,
        )
        .file("src/lib.rs", "")
        .file("examples/bar.rs", r#"fn main() { println!("example"); }"#)
        .build();

    p.cargo("run --example bar")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `target/debug/examples/bar[EXE]`

"#]])
        .with_stdout_data(str![[r#"
example

"#]])
        .run();
}

fn autodiscover_examples_project(rust_edition: &str, autoexamples: Option<bool>) -> Project {
    let autoexamples = match autoexamples {
        None => "".to_string(),
        Some(bool) => format!("autoexamples = {}", bool),
    };
    project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"
                    version = "0.0.1"
                    authors = []
                    edition = "{rust_edition}"
                    {autoexamples}

                    [features]
                    magic = []

                    [[example]]
                    name = "do_magic"
                    required-features = ["magic"]
                "#,
                rust_edition = rust_edition,
                autoexamples = autoexamples
            ),
        )
        .file("examples/a.rs", r#"fn main() { println!("example"); }"#)
        .file(
            "examples/do_magic.rs",
            r#"
                fn main() { println!("magic example"); }
            "#,
        )
        .build()
}

#[cargo_test]
fn run_example_autodiscover_2015() {
    let p = autodiscover_examples_project("2015", None);
    p.cargo("run --example a")
        .with_status(101)
        .with_stderr_data(str![[r#"
[WARNING] An explicit [[example]] section is specified in Cargo.toml which currently
disables Cargo from automatically inferring other example targets.
This inference behavior will change in the Rust 2018 edition and the following
files will be included as a example target:

* examples/a.rs

This is likely to break cargo build or cargo test as these files may not be
ready to be compiled as a example target today. You can future-proof yourself
and disable this warning by adding `autoexamples = false` to your [package]
section. You may also move the files to a location where Cargo would not
automatically infer them to be a target, such as in subfolders.

For more information on this warning you can consult
https://github.com/rust-lang/cargo/issues/5330
[ERROR] no example target named `a` in default-run packages
[HELP] available example targets:
    do_magic

"#]])
        .run();
}

#[cargo_test]
fn run_example_autodiscover_2015_with_autoexamples_enabled() {
    let p = autodiscover_examples_project("2015", Some(true));
    p.cargo("run --example a")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `target/debug/examples/a[EXE]`

"#]])
        .with_stdout_data(str![[r#"
example

"#]])
        .run();
}

#[cargo_test]
fn run_example_autodiscover_2015_with_autoexamples_disabled() {
    let p = autodiscover_examples_project("2015", Some(false));
    p.cargo("run --example a")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] no example target named `a` in default-run packages
[HELP] available example targets:
    do_magic

"#]])
        .run();
}

#[cargo_test]
fn run_example_autodiscover_2018() {
    let p = autodiscover_examples_project("2018", None);
    p.cargo("run --example a")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `target/debug/examples/a[EXE]`

"#]])
        .with_stdout_data(str![[r#"
example

"#]])
        .run();
}

#[cargo_test]
fn autobins_disables() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            edition = "2015"
            autobins = false
            "#,
        )
        .file("src/lib.rs", "pub mod bin;")
        .file("src/bin/mod.rs", "// empty")
        .build();

    p.cargo("run")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] a bin target must be available for `cargo run`

"#]])
        .run();
}

#[cargo_test]
fn run_bins() {
    let p = project()
        .file("src/lib.rs", "")
        .file("examples/a.rs", r#"fn main() { println!("example"); }"#)
        .file("src/bin/a.rs", r#"fn main() { println!("bin"); }"#)
        .build();

    p.cargo("run --bins")
        .with_status(1)
        .with_stderr_data(str![[r#"
[ERROR] unexpected argument '--bins' found

  tip: a similar argument exists: '--bin'
...
"#]])
        .run();
}

#[cargo_test]
fn run_with_filename() {
    let p = project()
        .file("src/lib.rs", "")
        .file(
            "src/bin/a.rs",
            r#"
                extern crate foo;
                fn main() { println!("hello a.rs"); }
            "#,
        )
        .file("examples/a.rs", r#"fn main() { println!("example"); }"#)
        .build();

    p.cargo("run --bin bin.rs")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] no bin target named `bin.rs` in default-run packages
[HELP] available bin targets:
    a

"#]])
        .run();

    p.cargo("run --bin a.rs")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] no bin target named `a.rs` in default-run packages

[HELP] a target with a similar name exists: `a`

"#]])
        .run();

    p.cargo("run --example example.rs")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] no example target named `example.rs` in default-run packages
[HELP] available example targets:
    a

"#]])
        .run();

    p.cargo("run --example a.rs")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] no example target named `a.rs` in default-run packages

[HELP] a target with a similar name exists: `a`

"#]])
        .run();
}

#[cargo_test]
fn ambiguous_bin_name() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
        [workspace]
        resolver = "3"
        members = ["crate1", "crate2", "crate3", "crate4"]
        "#,
        )
        .file("crate1/src/bin/ambiguous.rs", "fn main(){}")
        .file(
            "crate1/Cargo.toml",
            r#"
        [package]
        name = "crate1"
        version = "0.1.0"
        edition = "2024"
    "#,
        )
        .file("crate2/src/bin/ambiguous.rs", "fn main(){}")
        .file(
            "crate2/Cargo.toml",
            r#"
        [package]
        name = "crate2"
        version = "0.1.0"
        edition = "2024"
    "#,
        )
        .file("crate3/src/bin/ambiguous.rs", "fn main(){}")
        .file(
            "crate3/Cargo.toml",
            r#"
        [package]
        name = "crate3"
        version = "0.1.0"
        edition = "2024"
    "#,
        )
        .file("crate4/src/bin/ambiguous.rs", "fn main(){}")
        .file(
            "crate4/Cargo.toml",
            r#"
        [package]
        name = "crate4"
        version = "0.1.0"
        edition = "2024"
    "#,
        );
    let p = p.build();

    p.cargo("run --bin ambiguous")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] `cargo run` can run at most one executable, but multiple were specified
[HELP] available targets:
    bin `ambiguous` in package `crate1`
    bin `ambiguous` in package `crate2`
    bin `ambiguous` in package `crate3`
    bin `ambiguous` in package `crate4`

"#]])
        .run();

    p.cargo("run --bin crate1/ambiguous")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] no bin target named `crate1/ambiguous` in default-run packages
[HELP] available bin targets:
    ambiguous in package crate1
    ambiguous in package crate2
    ambiguous in package crate3
    ambiguous in package crate4

"#]])
        .run();
}

// See rust-lang/cargo#14544
#[cargo_test]
fn print_available_targets_within_virtual_workspace() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
        [workspace]
        resolver = "3"
        members = ["crate1", "crate2", "pattern1", "pattern2"]

        default-members = ["crate1"]
        "#,
        )
        .file("crate1/src/main.rs", "fn main(){}")
        .file(
            "crate1/Cargo.toml",
            r#"
        [package]
        name = "crate1"
        version = "0.1.0"
        edition = "2024"
    "#,
        )
        .file("crate2/src/main.rs", "fn main(){}")
        .file(
            "crate2/Cargo.toml",
            r#"
        [package]
        name = "crate2"
        version = "0.1.0"
        edition = "2024"
    "#,
        )
        .file("pattern1/src/main.rs", "fn main(){}")
        .file(
            "pattern1/Cargo.toml",
            r#"
        [package]
        name = "pattern1"
        version = "0.1.0"
        edition = "2024"
    "#,
        )
        .file("pattern2/src/main.rs", "fn main(){}")
        .file(
            "pattern2/Cargo.toml",
            r#"
        [package]
        name = "pattern2"
        version = "0.1.0"
        edition = "2024"
    "#,
        )
        .file("another/src/main.rs", "fn main(){}")
        .file(
            "another/Cargo.toml",
            r#"
        [package]
        name = "another"
        version = "0.1.0"
        edition = "2024"
    "#,
        );

    let p = p.build();
    p.cargo("run --bin")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] "--bin" takes one argument.
Available binaries:
    crate1


"#]])
        .run();

    p.cargo("run -p crate1 --bin crate2")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] no bin target named `crate2` in `crate1` package

[HELP] a target with a similar name exists: `crate1`
[HELP] available bin in `crate2` package:
    crate2

"#]])
        .run();

    p.cargo("check -p crate1 -p pattern1 -p pattern2 --bin crate2")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] no bin target named `crate2` in `crate1`, ... packages

[HELP] a target with a similar name exists: `crate1`
[HELP] available bin in `crate2` package:
    crate2

"#]])
        .run();

    p.cargo("run --bin crate2")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] no bin target named `crate2` in default-run packages

[HELP] a target with a similar name exists: `crate1`
[HELP] available bin in `crate2` package:
    crate2

"#]])
        .run();

    p.cargo("check --bin pattern*")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] no bin target matches pattern `pattern*` in default-run packages
[HELP] available bin in `pattern1` package:
    pattern1
[HELP] available bin in `pattern2` package:
    pattern2

"#]])
        .run();

    // This another branch that none of similar name exists, and print available targets in the
    // default-members.
    p.change_file(
        "Cargo.toml",
        r#"
        [workspace]
        resolver = "3"
        members = ["crate1", "crate2", "another"]

        default-members = ["another"]
        "#,
    );

    p.cargo("run --bin crate2")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] no bin target named `crate2` in default-run packages
[HELP] available bin in `crate2` package:
    crate2

"#]])
        .run();
}

#[cargo_test]
fn either_name_or_example() {
    let p = project()
        .file("src/bin/a.rs", r#"fn main() { println!("hello a.rs"); }"#)
        .file("examples/b.rs", r#"fn main() { println!("hello b.rs"); }"#)
        .build();

    p.cargo("run --bin a --example b")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] `cargo run` can run at most one executable, but multiple were specified
[HELP] available targets:
    bin `a` in package `foo`
    example `b` in package `foo`

"#]])
        .run();
}

#[cargo_test]
fn one_bin_multiple_examples() {
    let p = project()
        .file("src/lib.rs", "")
        .file(
            "src/bin/main.rs",
            r#"fn main() { println!("hello main.rs"); }"#,
        )
        .file("examples/a.rs", r#"fn main() { println!("hello a.rs"); }"#)
        .file("examples/b.rs", r#"fn main() { println!("hello b.rs"); }"#)
        .build();

    p.cargo("run")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `target/debug/main[EXE]`

"#]])
        .with_stdout_data(str![[r#"
hello main.rs

"#]])
        .run();
}

#[cargo_test]
fn example_with_release_flag() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies.bar]
                version = "*"
                path = "bar"
            "#,
        )
        .file(
            "examples/a.rs",
            r#"
                extern crate bar;

                fn main() {
                    if cfg!(debug_assertions) {
                        println!("slow1")
                    } else {
                        println!("fast1")
                    }
                    bar::baz();
                }
            "#,
        )
        .file("bar/Cargo.toml", &basic_lib_manifest("bar"))
        .file(
            "bar/src/bar.rs",
            r#"
                pub fn baz() {
                    if cfg!(debug_assertions) {
                        println!("slow2")
                    } else {
                        println!("fast2")
                    }
                }
            "#,
        )
        .build();

    p.cargo("run -v --release --example a")
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to latest compatible version
[COMPILING] bar v0.5.0 ([ROOT]/foo/bar)
[RUNNING] `rustc --crate-name bar --edition=2015 bar/src/bar.rs [..]--crate-type lib --emit=[..]link -C opt-level=3[..] -C metadata=[..] --out-dir [ROOT]/foo/target/release/deps -C strip=debuginfo -L dependency=[ROOT]/foo/target/release/deps`
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name a --edition=2015 examples/a.rs [..]--crate-type bin --emit=[..]link -C opt-level=3[..] -C metadata=[..] --out-dir [ROOT]/foo/target/release/examples -C strip=debuginfo -L dependency=[ROOT]/foo/target/release/deps --extern bar=[ROOT]/foo/target/release/deps/libbar-[HASH].rlib`
[FINISHED] `release` profile [optimized] target(s) in [ELAPSED]s
[RUNNING] `target/release/examples/a[EXE]`

"#]])
        .with_stdout_data(str![[r#"
fast1
fast2

"#]])
        .run();

    p.cargo("run -v --example a")
        .with_stderr_data(str![[r#"
[COMPILING] bar v0.5.0 ([ROOT]/foo/bar)
[RUNNING] `rustc --crate-name bar --edition=2015 bar/src/bar.rs [..]--crate-type lib --emit=[..]link [..]-C debuginfo=2 [..]-C metadata=[..] --out-dir [ROOT]/foo/target/debug/deps -L dependency=[ROOT]/foo/target/debug/deps`
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[RUNNING] `rustc --crate-name a --edition=2015 examples/a.rs [..]--crate-type bin --emit=[..]link [..]-C debuginfo=2 [..]-C metadata=[..] --out-dir [ROOT]/foo/target/debug/examples -L dependency=[ROOT]/foo/target/debug/deps --extern bar=[ROOT]/foo/target/debug/deps/libbar-[HASH].rlib`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `target/debug/examples/a[EXE]`

"#]])
        .with_stdout_data(str![[r#"
slow1
slow2

"#]])
        .run();
}

#[cargo_test]
fn run_dylib_dep() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies.bar]
                path = "bar"
            "#,
        )
        .file(
            "src/main.rs",
            r#"extern crate bar; fn main() { bar::bar(); }"#,
        )
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.0.1"
                authors = []

                [lib]
                name = "bar"
                crate-type = ["dylib"]
            "#,
        )
        .file("bar/src/lib.rs", "pub fn bar() {}")
        .build();

    p.cargo("run hello world").run();
}

#[cargo_test]
fn run_with_bin_dep() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"

                [dependencies.bar]
                path = "bar"
            "#,
        )
        .file("src/main.rs", r#"fn main() { println!("hello"); }"#)
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [[bin]]
                name = "bar"
            "#,
        )
        .file("bar/src/main.rs", r#"fn main() { println!("bar"); }"#)
        .build();

    p.cargo("run")
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to latest compatible version
[WARNING] foo v0.0.1 ([ROOT]/foo) ignoring invalid dependency `bar` which is missing a lib target
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `target/debug/foo[EXE]`

"#]])
        .with_stdout_data(str![[r#"
hello

"#]])
        .run();
}

#[cargo_test]
fn run_with_bin_deps() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"

                [dependencies.bar1]
                path = "bar1"
                [dependencies.bar2]
                path = "bar2"
            "#,
        )
        .file("src/main.rs", r#"fn main() { println!("hello"); }"#)
        .file(
            "bar1/Cargo.toml",
            r#"
                [package]
                name = "bar1"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [[bin]]
                name = "bar1"
            "#,
        )
        .file("bar1/src/main.rs", r#"fn main() { println!("bar1"); }"#)
        .file(
            "bar2/Cargo.toml",
            r#"
                [package]
                name = "bar2"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [[bin]]
                name = "bar2"
            "#,
        )
        .file("bar2/src/main.rs", r#"fn main() { println!("bar2"); }"#)
        .build();

    p.cargo("run")
        .with_stderr_data(str![[r#"
[LOCKING] 2 packages to latest compatible versions
[WARNING] foo v0.0.1 ([ROOT]/foo) ignoring invalid dependency `bar1` which is missing a lib target
[WARNING] foo v0.0.1 ([ROOT]/foo) ignoring invalid dependency `bar2` which is missing a lib target
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `target/debug/foo[EXE]`

"#]])
        .with_stdout_data(str![[r#"
hello

"#]])
        .run();
}

#[cargo_test]
fn run_with_bin_dep_in_workspace() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["foo1", "foo2"]
            "#,
        )
        .file(
            "foo1/Cargo.toml",
            r#"
                [package]
                name = "foo1"
                version = "0.0.1"
                edition = "2015"

                [dependencies.bar1]
                path = "bar1"
            "#,
        )
        .file("foo1/src/main.rs", r#"fn main() { println!("hello"); }"#)
        .file(
            "foo1/bar1/Cargo.toml",
            r#"
                [package]
                name = "bar1"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [[bin]]
                name = "bar1"
            "#,
        )
        .file(
            "foo1/bar1/src/main.rs",
            r#"fn main() { println!("bar1"); }"#,
        )
        .file(
            "foo2/Cargo.toml",
            r#"
                [package]
                name = "foo2"
                version = "0.0.1"
                edition = "2015"

                [dependencies.bar2]
                path = "bar2"
            "#,
        )
        .file("foo2/src/main.rs", r#"fn main() { println!("hello"); }"#)
        .file(
            "foo2/bar2/Cargo.toml",
            r#"
                [package]
                name = "bar2"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [[bin]]
                name = "bar2"
            "#,
        )
        .file(
            "foo2/bar2/src/main.rs",
            r#"fn main() { println!("bar2"); }"#,
        )
        .build();

    p.cargo("run")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] `cargo run` could not determine which binary to run. Use the `--bin` option to specify a binary, or the `default-run` manifest key.
available binaries: bar1, bar2, foo1, foo2

"#]])
        .run();

    p.cargo("run --bin foo1")
        .with_stderr_data(str![[r#"
[WARNING] foo1 v0.0.1 ([ROOT]/foo/foo1) ignoring invalid dependency `bar1` which is missing a lib target
[WARNING] foo2 v0.0.1 ([ROOT]/foo/foo2) ignoring invalid dependency `bar2` which is missing a lib target
[COMPILING] foo1 v0.0.1 ([ROOT]/foo/foo1)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `target/debug/foo1[EXE]`

"#]])
        .with_stdout_data(str![[r#"
hello

"#]])
        .run();
}

#[cargo_test]
fn release_works() {
    let p = project()
        .file(
            "src/main.rs",
            r#"
                fn main() { if cfg!(debug_assertions) { panic!() } }
            "#,
        )
        .build();

    p.cargo("run --release")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `release` profile [optimized] target(s) in [ELAPSED]s
[RUNNING] `target/release/foo[EXE]`

"#]])
        .run();
    assert!(p.release_bin("foo").is_file());
}

#[cargo_test]
fn release_short_works() {
    let p = project()
        .file(
            "src/main.rs",
            r#"
                fn main() { if cfg!(debug_assertions) { panic!() } }
            "#,
        )
        .build();

    p.cargo("run -r")
        .with_stderr_data(str![[r#"
[COMPILING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `release` profile [optimized] target(s) in [ELAPSED]s
[RUNNING] `target/release/foo[EXE]`

"#]])
        .run();
    assert!(p.release_bin("foo").is_file());
}

#[cargo_test]
fn run_bin_different_name() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [[bin]]
                name = "bar"
            "#,
        )
        .file("src/bar.rs", "fn main() {}")
        .build();

    p.cargo("run").run();
}

#[cargo_test]
fn dashes_are_forwarded() {
    let p = project()
        .file(
            "src/bin/bar.rs",
            r#"
                fn main() {
                    let s: Vec<String> = std::env::args().collect();
                    assert_eq!(s[1], "--");
                    assert_eq!(s[2], "a");
                    assert_eq!(s[3], "--");
                    assert_eq!(s[4], "b");
                }
            "#,
        )
        .build();

    p.cargo("run -- -- a -- b").run();
}

#[cargo_test]
fn run_from_executable_folder() {
    let p = project()
        .file("src/main.rs", r#"fn main() { println!("hello"); }"#)
        .build();

    let cwd = p.root().join("target").join("debug");
    p.cargo("build").run();

    p.cargo("run")
        .cwd(cwd)
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `./foo[EXE]`

"#]])
        .with_stdout_data(str![[r#"
hello

"#]])
        .run();
}

#[cargo_test]
fn run_with_library_paths() {
    let p = project();

    // Only link search directories within the target output directory are
    // propagated through to dylib_path_envvar() (see #3366).
    let mut dir1 = p.target_debug_dir();
    dir1.push("foo\\backslash");

    let mut dir2 = p.target_debug_dir();
    dir2.push("dir=containing=equal=signs");

    let p = p
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []
                build = "build.rs"
            "#,
        )
        .file(
            "build.rs",
            &format!(
                r##"
                    fn main() {{
                        println!(r#"cargo::rustc-link-search=native={}"#);
                        println!(r#"cargo::rustc-link-search={}"#);
                    }}
                "##,
                dir1.display(),
                dir2.display()
            ),
        )
        .file(
            "src/main.rs",
            &format!(
                r##"
                    fn main() {{
                        let search_path = std::env::var_os("{}").unwrap();
                        let paths = std::env::split_paths(&search_path).collect::<Vec<_>>();
                        println!("{{:#?}}", paths);
                        assert!(paths.contains(&r#"{}"#.into()));
                        assert!(paths.contains(&r#"{}"#.into()));
                    }}
                "##,
                dylib_path_envvar(),
                dir1.display(),
                dir2.display()
            ),
        )
        .build();

    p.cargo("run").run();
}

#[cargo_test]
fn library_paths_sorted_alphabetically() {
    let p = project();

    let mut dir1 = p.target_debug_dir();
    dir1.push("zzzzzzz");

    let mut dir2 = p.target_debug_dir();
    dir2.push("BBBBBBB");

    let mut dir3 = p.target_debug_dir();
    dir3.push("aaaaaaa");

    let p = p
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []
                build = "build.rs"
            "#,
        )
        .file(
            "build.rs",
            &format!(
                r##"
                    fn main() {{
                        println!(r#"cargo::rustc-link-search=native={}"#);
                        println!(r#"cargo::rustc-link-search=native={}"#);
                        println!(r#"cargo::rustc-link-search=native={}"#);
                    }}
                "##,
                dir1.display(),
                dir2.display(),
                dir3.display()
            ),
        )
        .file(
            "src/main.rs",
            &format!(
                r##"
                    fn main() {{
                        let search_path = std::env::var_os("{}").unwrap();
                        let paths = std::env::split_paths(&search_path).collect::<Vec<_>>();
                        // ASCII case-sensitive sort
                        assert_eq!("BBBBBBB", paths[0].file_name().unwrap().to_string_lossy());
                        assert_eq!("aaaaaaa", paths[1].file_name().unwrap().to_string_lossy());
                        assert_eq!("zzzzzzz", paths[2].file_name().unwrap().to_string_lossy());
                    }}
                "##,
                dylib_path_envvar()
            ),
        )
        .build();

    p.cargo("run").run();
}

#[cargo_test]
fn fail_no_extra_verbose() {
    let p = project()
        .file("src/main.rs", "fn main() { std::process::exit(1); }")
        .build();

    p.cargo("run -q")
        .with_status(1)
        .with_stdout_data("")
        .with_stderr_data("")
        .run();
}

#[cargo_test]
fn run_multiple_packages() {
    let p = project()
        .no_manifest()
        .file(
            "foo/Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [workspace]

                [dependencies]
                d1 = { path = "d1" }
                d2 = { path = "d2" }
                d3 = { path = "../d3" } # outside of the workspace

                [[bin]]
                name = "foo"
            "#,
        )
        .file("foo/src/foo.rs", "fn main() { println!(\"foo\"); }")
        .file("foo/d1/Cargo.toml", &basic_bin_manifest("d1"))
        .file("foo/d1/src/lib.rs", "")
        .file("foo/d1/src/main.rs", "fn main() { println!(\"d1\"); }")
        .file("foo/d2/Cargo.toml", &basic_bin_manifest("d2"))
        .file("foo/d2/src/main.rs", "fn main() { println!(\"d2\"); }")
        .file("d3/Cargo.toml", &basic_bin_manifest("d3"))
        .file("d3/src/main.rs", "fn main() { println!(\"d2\"); }")
        .build();

    let cargo = || {
        let mut process_builder = p.cargo("run");
        process_builder.cwd("foo");
        process_builder
    };

    cargo()
        .arg("-p")
        .arg("d1")
        .with_stdout_data(str![[r#"
d1

"#]])
        .run();

    cargo()
        .arg("-p")
        .arg("d2")
        .arg("--bin")
        .arg("d2")
        .with_stdout_data(str![[r#"
d2

"#]])
        .run();

    cargo()
        .with_stdout_data(str![[r#"
foo

"#]])
        .run();

    cargo()
        .arg("-p")
        .arg("d1")
        .arg("-p")
        .arg("d2")
        .with_status(1)
        .with_stderr_data(str![[r#"
[ERROR] the argument '--package [<SPEC>]' cannot be used multiple times
...
"#]])
        .run();

    cargo()
        .arg("-p")
        .arg("d3")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] package(s) `d3` not found in workspace `[ROOT]/foo/foo`

"#]])
        .run();

    cargo()
        .arg("-p")
        .arg("d*")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] `cargo run` does not support glob pattern `d*` on package selection

"#]])
        .run();
}

#[cargo_test]
fn explicit_bin_with_args() {
    let p = project()
        .file(
            "src/main.rs",
            r#"
                fn main() {
                    assert_eq!(std::env::args().nth(1).unwrap(), "hello");
                    assert_eq!(std::env::args().nth(2).unwrap(), "world");
                }
            "#,
        )
        .build();

    p.cargo("run --bin foo hello world").run();
}

#[cargo_test]
fn run_workspace() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["a", "b"]
            "#,
        )
        .file("a/Cargo.toml", &basic_bin_manifest("a"))
        .file("a/src/main.rs", r#"fn main() {println!("run-a");}"#)
        .file("b/Cargo.toml", &basic_bin_manifest("b"))
        .file("b/src/main.rs", r#"fn main() {println!("run-b");}"#)
        .build();

    p.cargo("run")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] `cargo run` could not determine which binary to run. Use the `--bin` option to specify a binary, or the `default-run` manifest key.
available binaries: a, b

"#]])
        .run();
    p.cargo("run --bin a")
        .with_stdout_data(str![[r#"
run-a

"#]])
        .run();
}

#[cargo_test]
fn default_run_workspace() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["a", "b"]
            "#,
        )
        .file(
            "a/Cargo.toml",
            r#"
                [package]
                name = "a"
                version = "0.0.1"
                edition = "2015"
                default-run = "a"
            "#,
        )
        .file("a/src/main.rs", r#"fn main() {println!("run-a");}"#)
        .file("b/Cargo.toml", &basic_bin_manifest("b"))
        .file("b/src/main.rs", r#"fn main() {println!("run-b");}"#)
        .build();

    p.cargo("run")
        .with_stdout_data(str![[r#"
run-a

"#]])
        .run();
}

#[cargo_test]
fn print_env_verbose() {
    let p = project()
        .file("Cargo.toml", &basic_manifest("a", "0.0.1"))
        .file("src/main.rs", r#"fn main() {println!("run-a");}"#)
        .build();

    p.cargo("run -vv")
        .with_stderr_data(str![[r#"
[COMPILING] a v0.0.1 ([ROOT]/foo)
[RUNNING] `[..]CARGO_MANIFEST_DIR=[ROOT]/foo[..] rustc --crate-name a[..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[RUNNING] `[..]CARGO_MANIFEST_DIR=[ROOT]/foo[..] target/debug/a[EXE]`

"#]])
        .run();
}

#[cargo_test]
#[cfg(target_os = "macos")]
fn run_link_system_path_macos() {
    use cargo_test_support::paths;
    use std::fs;
    // Check that the default system library path is honored.
    // First, build a shared library that will be accessed from
    // DYLD_FALLBACK_LIBRARY_PATH.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            edition = "2015"
            [lib]
            crate-type = ["cdylib"]
            "#,
        )
        .file(
            "src/lib.rs",
            "#[no_mangle] pub extern fn something_shared() {}",
        )
        .build();
    p.cargo("build").run();

    // This is convoluted. Since this test can't modify things in /usr,
    // this needs to dance around to check that things work.
    //
    // The default DYLD_FALLBACK_LIBRARY_PATH is:
    //      $(HOME)/lib:/usr/local/lib:/lib:/usr/lib
    //
    // This will make use of ~/lib in the path, but the default cc link
    // path is /usr/lib:/usr/local/lib. So first need to build in one
    // location, and then move it to ~/lib.
    //
    // 1. Build with rustc-link-search pointing to libfoo so the initial
    //    binary can be linked.
    // 2. Move the library to ~/lib
    // 3. Run `cargo run` to make sure it can still find the library in
    //    ~/lib.
    //
    // This should be equivalent to having the library in /usr/local/lib.
    let p2 = project()
        .at("bar")
        .file("Cargo.toml", &basic_bin_manifest("bar"))
        .file(
            "src/main.rs",
            r#"
            extern {
                fn something_shared();
            }
            fn main() {
                unsafe { something_shared(); }
            }
            "#,
        )
        .file(
            "build.rs",
            &format!(
                r#"
                fn main() {{
                    println!("cargo::rustc-link-lib=foo");
                    println!("cargo::rustc-link-search={}");
                }}
                "#,
                p.target_debug_dir().display()
            ),
        )
        .build();
    p2.cargo("build").run();
    p2.cargo("test").run();

    let libdir = paths::home().join("lib");
    fs::create_dir(&libdir).unwrap();
    fs::rename(
        p.target_debug_dir().join("libfoo.dylib"),
        libdir.join("libfoo.dylib"),
    )
    .unwrap();
    p.root().rm_rf();
    const VAR: &str = "DYLD_FALLBACK_LIBRARY_PATH";
    // Reset DYLD_FALLBACK_LIBRARY_PATH so that we don't inherit anything that
    // was set by the cargo that invoked the test.
    p2.cargo("run").env_remove(VAR).run();
    p2.cargo("test").env_remove(VAR).run();
    // Ensure this still works when DYLD_FALLBACK_LIBRARY_PATH has
    // a value set.
    p2.cargo("run").env(VAR, &libdir).run();
    p2.cargo("test").env(VAR, &libdir).run();
}

#[cargo_test]
fn run_binary_with_same_name_as_dependency() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.5.0"
                edition = "2015"
                authors = []

                [dependencies]
                foo = { path = "foo" }
            "#,
        )
        .file(
            "src/main.rs",
            r#"
                fn main() {}
            "#,
        )
        .file(
            "foo/Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"
                authors = []

                [lib]
                name = "foo"
                path = "foo.rs"
            "#,
        )
        .file("foo/foo.rs", "")
        .build();
    p.cargo("run").run();
    p.cargo("check -p foo@0.5.0").run();
    p.cargo("run -p foo@0.5.0").run();
    p.cargo("run -p foo@0.5").run();
    p.cargo("run -p foo@0.4")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] package(s) `foo@0.4` not found in workspace `[ROOT]/foo`

"#]])
        .run();
}
