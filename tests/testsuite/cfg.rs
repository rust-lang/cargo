//! Tests for cfg() expressions.

use cargo_test_support::registry::Package;
use cargo_test_support::rustc_host;
use cargo_test_support::{basic_manifest, project};

#[cargo_test]
fn cfg_easy() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "a"
                version = "0.0.1"
                authors = []

                [target.'cfg(unix)'.dependencies]
                b = { path = 'b' }
                [target."cfg(windows)".dependencies]
                b = { path = 'b' }
            "#,
        )
        .file("src/lib.rs", "extern crate b;")
        .file("b/Cargo.toml", &basic_manifest("b", "0.0.1"))
        .file("b/src/lib.rs", "")
        .build();
    p.cargo("check -v").run();
}

#[cargo_test]
fn dont_include() {
    let other_family = if cfg!(unix) { "windows" } else { "unix" };
    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "a"
                    version = "0.0.1"
                    authors = []

                    [target.'cfg({})'.dependencies]
                    b = {{ path = 'b' }}
                "#,
                other_family
            ),
        )
        .file("src/lib.rs", "")
        .file("b/Cargo.toml", &basic_manifest("b", "0.0.1"))
        .file("b/src/lib.rs", "")
        .build();
    p.cargo("check")
        .with_stderr(
            "\
[CHECKING] a v0.0.1 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cargo_test]
fn works_through_the_registry() {
    Package::new("baz", "0.1.0").publish();
    Package::new("bar", "0.1.0")
        .target_dep("baz", "0.1.0", "cfg(unix)")
        .target_dep("baz", "0.1.0", "cfg(windows)")
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies]
                bar = "0.1.0"
            "#,
        )
        .file(
            "src/lib.rs",
            "#[allow(unused_extern_crates)] extern crate bar;",
        )
        .build();

    p.cargo("check")
        .with_stderr(
            "\
[UPDATING] [..] index
[DOWNLOADING] crates ...
[DOWNLOADED] [..]
[DOWNLOADED] [..]
[CHECKING] baz v0.1.0
[CHECKING] bar v0.1.0
[CHECKING] foo v0.0.1 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cargo_test]
fn ignore_version_from_other_platform() {
    let this_family = if cfg!(unix) { "unix" } else { "windows" };
    let other_family = if cfg!(unix) { "windows" } else { "unix" };
    Package::new("bar", "0.1.0").publish();
    Package::new("bar", "0.2.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"
                    version = "0.0.1"
                    authors = []

                    [target.'cfg({})'.dependencies]
                    bar = "0.1.0"

                    [target.'cfg({})'.dependencies]
                    bar = "0.2.0"
                "#,
                this_family, other_family
            ),
        )
        .file(
            "src/lib.rs",
            "#[allow(unused_extern_crates)] extern crate bar;",
        )
        .build();

    p.cargo("check")
        .with_stderr(
            "\
[UPDATING] [..] index
[DOWNLOADING] crates ...
[DOWNLOADED] [..]
[CHECKING] bar v0.1.0
[CHECKING] foo v0.0.1 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cargo_test]
fn bad_target_spec() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [target.'cfg(4)'.dependencies]
                bar = "0.1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  failed to parse `4` as a cfg expression: unexpected character `4` in cfg, [..]
",
        )
        .run();
}

#[cargo_test]
fn bad_target_spec2() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [target.'cfg(bar =)'.dependencies]
                baz = "0.1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  failed to parse `bar =` as a cfg expression: expected a string, but cfg expression ended
",
        )
        .run();
}

#[cargo_test]
fn multiple_match_ok() {
    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "a"
                    version = "0.0.1"
                    authors = []

                    [target.'cfg(unix)'.dependencies]
                    b = {{ path = 'b' }}
                    [target.'cfg(target_family = "unix")'.dependencies]
                    b = {{ path = 'b' }}
                    [target."cfg(windows)".dependencies]
                    b = {{ path = 'b' }}
                    [target.'cfg(target_family = "windows")'.dependencies]
                    b = {{ path = 'b' }}
                    [target."cfg(any(windows, unix))".dependencies]
                    b = {{ path = 'b' }}

                    [target.{}.dependencies]
                    b = {{ path = 'b' }}
                "#,
                rustc_host()
            ),
        )
        .file("src/lib.rs", "extern crate b;")
        .file("b/Cargo.toml", &basic_manifest("b", "0.0.1"))
        .file("b/src/lib.rs", "")
        .build();
    p.cargo("check -v").run();
}

#[cargo_test]
fn any_ok() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "a"
                version = "0.0.1"
                authors = []

                [target."cfg(any(windows, unix))".dependencies]
                b = { path = 'b' }
            "#,
        )
        .file("src/lib.rs", "extern crate b;")
        .file("b/Cargo.toml", &basic_manifest("b", "0.0.1"))
        .file("b/src/lib.rs", "")
        .build();
    p.cargo("check -v").run();
}

// https://github.com/rust-lang/cargo/issues/5313
#[cargo_test]
#[cfg(all(target_arch = "x86_64", target_os = "linux", target_env = "gnu"))]
fn cfg_looks_at_rustflags_for_target() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "a"
                version = "0.0.1"
                authors = []

                [target.'cfg(with_b)'.dependencies]
                b = { path = 'b' }
            "#,
        )
        .file(
            "src/main.rs",
            r#"
                #[cfg(with_b)]
                extern crate b;

                fn main() { b::foo(); }
            "#,
        )
        .file("b/Cargo.toml", &basic_manifest("b", "0.0.1"))
        .file("b/src/lib.rs", "pub fn foo() {}")
        .build();

    p.cargo("check --target x86_64-unknown-linux-gnu")
        .env("RUSTFLAGS", "--cfg with_b")
        .run();
}

#[cargo_test]
fn bad_cfg_discovery() {
    // Check error messages when `rustc -v` and `rustc --print=*` parsing fails.
    //
    // This is a `rustc` replacement which behaves differently based on an
    // environment variable.
    let p = project()
        .at("compiler")
        .file("Cargo.toml", &basic_manifest("compiler", "0.1.0"))
        .file(
            "src/main.rs",
            r#"
            fn run_rustc() -> String {
                let mut cmd = std::process::Command::new("rustc");
                for arg in std::env::args_os().skip(1) {
                    cmd.arg(arg);
                }
                String::from_utf8(cmd.output().unwrap().stdout).unwrap()
            }

            fn main() {
                let mode = std::env::var("FUNKY_MODE").unwrap();
                if mode == "bad-version" {
                    println!("foo");
                    return;
                }
                if std::env::args_os().any(|a| a == "-vV") {
                    print!("{}", run_rustc());
                    return;
                }
                if mode == "no-crate-types" {
                    return;
                }
                if mode == "bad-crate-type" {
                    println!("foo");
                    return;
                }
                let output = run_rustc();
                let mut lines = output.lines();
                let sysroot = loop {
                    let line = lines.next().unwrap();
                    if line.contains("___") {
                        println!("{}", line);
                    } else {
                        break line;
                    }
                };
                if mode == "no-sysroot" {
                    return;
                }
                println!("{}", sysroot);

                if mode == "no-split-debuginfo" {
                    return;
                }
                loop {
                    let line = lines.next().unwrap();
                    if line == "___" {
                        println!("\n{line}");
                        break;
                    } else {
                        // As the number split-debuginfo options varies,
                        // concat them into one line.
                        print!("{line},");
                    }
                };

                if mode != "bad-cfg" {
                    panic!("unexpected");
                }
                println!("123");
            }
            "#,
        )
        .build();
    p.cargo("build").run();
    let funky_rustc = p.bin("compiler");

    let p = project().file("src/lib.rs", "").build();

    p.cargo("check")
        .env("RUSTC", &funky_rustc)
        .env("FUNKY_MODE", "bad-version")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] `rustc -vV` didn't have a line for `host:`, got:
foo

",
        )
        .run();

    p.cargo("check")
        .env("RUSTC", &funky_rustc)
        .env("FUNKY_MODE", "no-crate-types")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] malformed output when learning about crate-type bin information
command was: `[..]compiler[..] --crate-name ___ [..]`
(no output received)
",
        )
        .run();

    p.cargo("check")
        .env("RUSTC", &funky_rustc)
        .env("FUNKY_MODE", "no-sysroot")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] output of --print=sysroot missing when learning about target-specific information from rustc
command was: `[..]compiler[..]--crate-type [..]`

--- stdout
[..]___[..]
[..]___[..]
[..]___[..]
[..]___[..]
[..]___[..]
[..]___[..]

",
        )
        .run();

    p.cargo("check")
        .env("RUSTC", &funky_rustc)
        .env("FUNKY_MODE", "no-split-debuginfo")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] output of --print=split-debuginfo missing when learning about target-specific information from rustc
command was: `[..]compiler[..]--crate-type [..]`

--- stdout
[..]___[..]
[..]___[..]
[..]___[..]
[..]___[..]
[..]___[..]
[..]___[..]
[..]

",
        )
        .run();

    p.cargo("check")
        .env("RUSTC", &funky_rustc)
        .env("FUNKY_MODE", "bad-cfg")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] failed to parse the cfg from `rustc --print=cfg`, got:
[..]___[..]
[..]___[..]
[..]___[..]
[..]___[..]
[..]___[..]
[..]___[..]
[..]
[..],[..]
___
123


Caused by:
  failed to parse `123` as a cfg expression: unexpected character `1` in cfg, \
  expected parens, a comma, an identifier, or a string
",
        )
        .run();
}

#[cargo_test]
fn exclusive_dep_kinds() {
    // Checks for a bug where the same package with different cfg expressions
    // was not being filtered correctly.
    Package::new("bar", "1.0.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [target.'cfg(abc)'.dependencies]
                bar = "1.0"

                [target.'cfg(not(abc))'.build-dependencies]
                bar = "1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .file("build.rs", "extern crate bar; fn main() {}")
        .build();

    p.cargo("check").run();
    p.change_file("src/lib.rs", "extern crate bar;");
    p.cargo("check")
        .with_status(101)
        // can't find crate for `bar`
        .with_stderr_contains("[..]E0463[..]")
        .run();
}
