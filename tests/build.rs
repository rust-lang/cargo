extern crate cargo;
extern crate cargotest;
extern crate hamcrest;
extern crate tempdir;

use std::env;
use std::fs::{self, File};
use std::io::prelude::*;

use cargo::util::paths::dylib_path_envvar;
use cargo::util::process;
use cargotest::{is_nightly, rustc_host, sleep_ms};
use cargotest::support::paths::{CargoPathExt,root};
use cargotest::support::{ProjectBuilder};
use cargotest::support::{project, execs, main_file, basic_bin_manifest};
use cargotest::support::registry::Package;
use hamcrest::{assert_that, existing_file, is_not};
use tempdir::TempDir;

#[test]
fn cargo_compile_simple() {
    let p = project("foo")
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]));

    assert_that(p.cargo_process("build"), execs().with_status(0));
    assert_that(&p.bin("foo"), existing_file());

    assert_that(process(&p.bin("foo")),
                execs().with_status(0).with_stdout("i am foo\n"));
}

#[test]
fn cargo_fail_with_no_stderr() {
    let p = project("foo")
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &String::from("refusal"));
    let p = p.build();
    assert_that(p.cargo("build").arg("--message-format=json"), execs().with_status(101)
        .with_stderr_does_not_contain("--- stderr"));
}

/// Check that the `CARGO_INCREMENTAL` environment variable results in
/// `rustc` getting `-Zincremental` passed to it.
#[test]
fn cargo_compile_incremental() {
    if !is_nightly() {
        return
    }

    let p = project("foo")
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]));
    p.build();

    assert_that(
        p.cargo("build").arg("-v").env("CARGO_INCREMENTAL", "1"),
        execs().with_stderr_contains(
            "[RUNNING] `rustc [..] -Zincremental=[..][/]target[/]debug[/]incremental`\n")
            .with_status(0));

    assert_that(
        p.cargo("test").arg("-v").env("CARGO_INCREMENTAL", "1"),
        execs().with_stderr_contains(
            "[RUNNING] `rustc [..] -Zincremental=[..][/]target[/]debug[/]incremental`\n")
               .with_status(0));
}

#[test]
fn cargo_compile_manifest_path() {
    let p = project("foo")
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]));

    assert_that(p.cargo_process("build")
                 .arg("--manifest-path").arg("foo/Cargo.toml")
                 .cwd(p.root().parent().unwrap()),
                execs().with_status(0));
    assert_that(&p.bin("foo"), existing_file());
}

#[test]
fn cargo_compile_with_invalid_manifest() {
    let p = project("foo")
        .file("Cargo.toml", "");

    assert_that(p.cargo_process("build"),
        execs()
        .with_status(101)
        .with_stderr("\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  no `package` or `project` section found.
"))
}

#[test]
fn cargo_compile_with_invalid_manifest2() {
    let p = project("foo")
        .file("Cargo.toml", r"
            [project]
            foo = bar
        ");

    assert_that(p.cargo_process("build"),
        execs()
        .with_status(101)
        .with_stderr("\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  could not parse input as TOML

Caused by:
  invalid number at line 3
"))
}

#[test]
fn cargo_compile_with_invalid_manifest3() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/Cargo.toml", "a = bar");

    assert_that(p.cargo_process("build").arg("--manifest-path")
                 .arg("src/Cargo.toml"),
        execs()
        .with_status(101)
        .with_stderr("\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  could not parse input as TOML

Caused by:
  invalid number at line 1
"))
}

#[test]
fn cargo_compile_duplicate_build_targets() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [lib]
            name = "main"
            path = "src/main.rs"
            crate-type = ["dylib"]

            [dependencies]
        "#)
        .file("src/main.rs", r#"
            #![allow(warnings)]
            fn main() {}
        "#);

    assert_that(p.cargo_process("build"),
                execs()
                .with_status(0)
                .with_stderr("\
warning: file found to be present in multiple build targets: [..]main.rs
[COMPILING] foo v0.0.1 ([..])
[FINISHED] [..]
"));
}

#[test]
fn cargo_compile_with_invalid_version() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            authors = []
            version = "1.0"
        "#);

    assert_that(p.cargo_process("build"),
                execs()
                .with_status(101)
                .with_stderr("\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  Expected dot for key `project.version`
"))

}

#[test]
fn cargo_compile_with_invalid_package_name() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = ""
            authors = []
            version = "0.0.0"
        "#);

    assert_that(p.cargo_process("build"),
                execs()
                .with_status(101)
                .with_stderr("\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  package name cannot be an empty string.
"))
}

#[test]
fn cargo_compile_with_invalid_bin_target_name() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            authors = []
            version = "0.0.0"

            [[bin]]
            name = ""
        "#);

    assert_that(p.cargo_process("build"),
                execs()
                .with_status(101)
                .with_stderr("\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  binary target names cannot be empty
"))
}

#[test]
fn cargo_compile_with_forbidden_bin_target_name() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            authors = []
            version = "0.0.0"

            [[bin]]
            name = "build"
        "#);

    assert_that(p.cargo_process("build"),
                execs()
                .with_status(101)
                .with_stderr("\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  the binary target name `build` is forbidden
"))
}

#[test]
fn cargo_compile_with_invalid_lib_target_name() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            authors = []
            version = "0.0.0"

            [lib]
            name = ""
        "#);

    assert_that(p.cargo_process("build"),
                execs()
                .with_status(101)
                .with_stderr("\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  library target names cannot be empty
"))
}

#[test]
fn cargo_compile_without_manifest() {
    let tmpdir = TempDir::new("cargo").unwrap();
    let p = ProjectBuilder::new("foo", tmpdir.path().to_path_buf());

    assert_that(p.cargo_process("build"),
                execs().with_status(101)
                       .with_stderr("\
[ERROR] could not find `Cargo.toml` in `[..]` or any parent directory
"));
}

#[test]
fn cargo_compile_with_invalid_code() {
    let p = project("foo")
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", "invalid rust code!");

    assert_that(p.cargo_process("build"),
        execs()
        .with_status(101)
        .with_stderr_contains("\
[ERROR] Could not compile `foo`.

To learn more, run the command again with --verbose.\n"));
    assert_that(&p.root().join("Cargo.lock"), existing_file());
}

#[test]
fn cargo_compile_with_invalid_code_in_deps() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.bar]
            path = "../bar"
            [dependencies.baz]
            path = "../baz"
        "#)
        .file("src/main.rs", "invalid rust code!");
    let bar = project("bar")
        .file("Cargo.toml", &basic_bin_manifest("bar"))
        .file("src/lib.rs", "invalid rust code!");
    let baz = project("baz")
        .file("Cargo.toml", &basic_bin_manifest("baz"))
        .file("src/lib.rs", "invalid rust code!");
    bar.build();
    baz.build();
    assert_that(p.cargo_process("build"), execs().with_status(101));
}

#[test]
fn cargo_compile_with_warnings_in_the_root_package() {
    let p = project("foo")
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", "fn main() {} fn dead() {}");

    assert_that(p.cargo_process("build"),
                execs().with_status(0).with_stderr_contains("\
[..]function is never used: `dead`[..]
"));
}

#[test]
fn cargo_compile_with_warnings_in_a_dep_package() {
    let mut p = project("foo");

    p = p
        .file("Cargo.toml", r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dependencies.bar]
            path = "bar"

            [[bin]]

            name = "foo"
        "#)
        .file("src/foo.rs",
              &main_file(r#""{}", bar::gimme()"#, &["bar"]))
        .file("bar/Cargo.toml", r#"
            [project]

            name = "bar"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [lib]

            name = "bar"
        "#)
        .file("bar/src/bar.rs", r#"
            pub fn gimme() -> &'static str {
                "test passed"
            }

            fn dead() {}
        "#);

    assert_that(p.cargo_process("build"),
        execs().with_status(0).with_stderr_contains("\
[..]function is never used: `dead`[..]
"));

    assert_that(&p.bin("foo"), existing_file());

    assert_that(
      process(&p.bin("foo")),
      execs().with_status(0).with_stdout("test passed\n"));
}

#[test]
fn cargo_compile_with_nested_deps_inferred() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dependencies.bar]
            path = 'bar'

            [[bin]]
            name = "foo"
        "#)
        .file("src/foo.rs",
              &main_file(r#""{}", bar::gimme()"#, &["bar"]))
        .file("bar/Cargo.toml", r#"
            [project]

            name = "bar"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dependencies.baz]
            path = "../baz"
        "#)
        .file("bar/src/lib.rs", r#"
            extern crate baz;

            pub fn gimme() -> String {
                baz::gimme()
            }
        "#)
        .file("baz/Cargo.toml", r#"
            [project]

            name = "baz"
            version = "0.5.0"
            authors = ["wycats@example.com"]
        "#)
        .file("baz/src/lib.rs", r#"
            pub fn gimme() -> String {
                "test passed".to_string()
            }
        "#);

    p.cargo_process("build")
        .exec_with_output()
        .unwrap();

    assert_that(&p.bin("foo"), existing_file());
    assert_that(&p.bin("libbar.rlib"), is_not(existing_file()));
    assert_that(&p.bin("libbaz.rlib"), is_not(existing_file()));

    assert_that(
      process(&p.bin("foo")),
      execs().with_status(0).with_stdout("test passed\n"));
}

#[test]
fn cargo_compile_with_nested_deps_correct_bin() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dependencies.bar]
            path = "bar"

            [[bin]]
            name = "foo"
        "#)
        .file("src/main.rs",
              &main_file(r#""{}", bar::gimme()"#, &["bar"]))
        .file("bar/Cargo.toml", r#"
            [project]

            name = "bar"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dependencies.baz]
            path = "../baz"
        "#)
        .file("bar/src/lib.rs", r#"
            extern crate baz;

            pub fn gimme() -> String {
                baz::gimme()
            }
        "#)
        .file("baz/Cargo.toml", r#"
            [project]

            name = "baz"
            version = "0.5.0"
            authors = ["wycats@example.com"]
        "#)
        .file("baz/src/lib.rs", r#"
            pub fn gimme() -> String {
                "test passed".to_string()
            }
        "#);

    p.cargo_process("build")
        .exec_with_output()
        .unwrap();

    assert_that(&p.bin("foo"), existing_file());
    assert_that(&p.bin("libbar.rlib"), is_not(existing_file()));
    assert_that(&p.bin("libbaz.rlib"), is_not(existing_file()));

    assert_that(
      process(&p.bin("foo")),
      execs().with_status(0).with_stdout("test passed\n"));
}

#[test]
fn cargo_compile_with_nested_deps_shorthand() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dependencies.bar]
            path = "bar"
        "#)
        .file("src/main.rs",
              &main_file(r#""{}", bar::gimme()"#, &["bar"]))
        .file("bar/Cargo.toml", r#"
            [project]

            name = "bar"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dependencies.baz]
            path = "../baz"

            [lib]

            name = "bar"
        "#)
        .file("bar/src/bar.rs", r#"
            extern crate baz;

            pub fn gimme() -> String {
                baz::gimme()
            }
        "#)
        .file("baz/Cargo.toml", r#"
            [project]

            name = "baz"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [lib]

            name = "baz"
        "#)
        .file("baz/src/baz.rs", r#"
            pub fn gimme() -> String {
                "test passed".to_string()
            }
        "#);

    p.cargo_process("build")
        .exec_with_output()
        .unwrap();

    assert_that(&p.bin("foo"), existing_file());
    assert_that(&p.bin("libbar.rlib"), is_not(existing_file()));
    assert_that(&p.bin("libbaz.rlib"), is_not(existing_file()));

    assert_that(
      process(&p.bin("foo")),
      execs().with_status(0).with_stdout("test passed\n"));
}

#[test]
fn cargo_compile_with_nested_deps_longhand() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dependencies.bar]
            path = "bar"
            version = "0.5.0"

            [[bin]]

            name = "foo"
        "#)
        .file("src/foo.rs",
              &main_file(r#""{}", bar::gimme()"#, &["bar"]))
        .file("bar/Cargo.toml", r#"
            [project]

            name = "bar"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dependencies.baz]
            path = "../baz"
            version = "0.5.0"

            [lib]

            name = "bar"
        "#)
        .file("bar/src/bar.rs", r#"
            extern crate baz;

            pub fn gimme() -> String {
                baz::gimme()
            }
        "#)
        .file("baz/Cargo.toml", r#"
            [project]

            name = "baz"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [lib]

            name = "baz"
        "#)
        .file("baz/src/baz.rs", r#"
            pub fn gimme() -> String {
                "test passed".to_string()
            }
        "#);

    assert_that(p.cargo_process("build"), execs());

    assert_that(&p.bin("foo"), existing_file());
    assert_that(&p.bin("libbar.rlib"), is_not(existing_file()));
    assert_that(&p.bin("libbaz.rlib"), is_not(existing_file()));

    assert_that(process(&p.bin("foo")),
                execs().with_status(0).with_stdout("test passed\n"));
}

// Check that Cargo gives a sensible error if a dependency can't be found
// because of a name mismatch.
#[test]
fn cargo_compile_with_dep_name_mismatch() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]

            name = "foo"
            version = "0.0.1"
            authors = ["wycats@example.com"]

            [[bin]]

            name = "foo"

            [dependencies.notquitebar]

            path = "bar"
        "#)
        .file("src/bin/foo.rs", &main_file(r#""i am foo""#, &["bar"]))
        .file("bar/Cargo.toml", &basic_bin_manifest("bar"))
        .file("bar/src/bar.rs", &main_file(r#""i am bar""#, &[]));

    assert_that(p.cargo_process("build"),
                execs().with_status(101).with_stderr(&format!(
r#"[ERROR] no matching package named `notquitebar` found (required by `foo`)
location searched: {proj_dir}/bar
version required: *
"#, proj_dir = p.url())));
}

#[test]
fn cargo_compile_with_filename() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/lib.rs", "")
        .file("src/bin/a.rs", r#"
            extern crate foo;
            fn main() { println!("hello a.rs"); }
        "#)
        .file("examples/a.rs", r#"
            fn main() { println!("example"); }
        "#);
    p.build();

    assert_that(p.cargo("build").arg("--bin").arg("bin.rs"),
                execs().with_status(101).with_stderr("\
[ERROR] no bin target named `bin.rs`"));

    assert_that(p.cargo("build").arg("--bin").arg("a.rs"),
                execs().with_status(101).with_stderr("\
[ERROR] no bin target named `a.rs`

Did you mean `a`?"));

    assert_that(p.cargo("build").arg("--example").arg("example.rs"),
                execs().with_status(101).with_stderr("\
[ERROR] no example target named `example.rs`"));

    assert_that(p.cargo("build").arg("--example").arg("a.rs"),
                execs().with_status(101).with_stderr("\
[ERROR] no example target named `a.rs`

Did you mean `a`?"));
}

#[test]
fn compile_path_dep_then_change_version() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.bar]
            path = "bar"
        "#)
        .file("src/lib.rs", "")
        .file("bar/Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.0.1"
            authors = []
        "#)
        .file("bar/src/lib.rs", "");

    assert_that(p.cargo_process("build"), execs().with_status(0));

    File::create(&p.root().join("bar/Cargo.toml")).unwrap().write_all(br#"
        [package]
        name = "bar"
        version = "0.0.2"
        authors = []
    "#).unwrap();

    assert_that(p.cargo("build"),
                execs().with_status(101).with_stderr("\
[ERROR] no matching version `= 0.0.1` found for package `bar` (required by `foo`)
location searched: [..]
versions found: 0.0.2
consider running `cargo update` to update a path dependency's locked version
"));
}

#[test]
fn ignores_carriage_return_in_lockfile() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            authors = []
            version = "0.0.1"
        "#)
        .file("src/main.rs", r#"
            mod a; fn main() {}
        "#)
        .file("src/a.rs", "");

    assert_that(p.cargo_process("build"),
                execs().with_status(0));

    let lockfile = p.root().join("Cargo.lock");
    let mut lock = String::new();
    File::open(&lockfile).unwrap().read_to_string(&mut lock).unwrap();
    let lock = lock.replace("\n", "\r\n");
    File::create(&lockfile).unwrap().write_all(lock.as_bytes()).unwrap();
    assert_that(p.cargo("build"),
                execs().with_status(0));
}

#[test]
fn cargo_default_env_metadata_env_var() {
    // Ensure that path dep + dylib + env_var get metadata
    // (even though path_dep + dylib should not)
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.bar]
            path = "bar"
        "#)
        .file("src/lib.rs", "// hi")
        .file("bar/Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.0.1"
            authors = []

            [lib]
            name = "bar"
            crate_type = ["dylib"]
        "#)
        .file("bar/src/lib.rs", "// hello");
    p.build();

    // No metadata on libbar since it's a dylib path dependency
    assert_that(p.cargo("build").arg("-v"),
                execs().with_status(0).with_stderr(&format!("\
[COMPILING] bar v0.0.1 ({url}/bar)
[RUNNING] `rustc --crate-name bar bar[/]src[/]lib.rs --crate-type dylib \
        --emit=dep-info,link \
        -C prefer-dynamic -C debuginfo=2 \
        -C metadata=[..] \
        --out-dir [..] \
        -L dependency={dir}[/]target[/]debug[/]deps`
[COMPILING] foo v0.0.1 ({url})
[RUNNING] `rustc --crate-name foo src[/]lib.rs --crate-type lib \
        --emit=dep-info,link -C debuginfo=2 \
        -C metadata=[..] \
        -C extra-filename=[..] \
        --out-dir [..] \
        -L dependency={dir}[/]target[/]debug[/]deps \
        --extern bar={dir}[/]target[/]debug[/]deps[/]{prefix}bar{suffix}`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]",
dir = p.root().display(),
url = p.url(),
prefix = env::consts::DLL_PREFIX,
suffix = env::consts::DLL_SUFFIX,
)));

    assert_that(p.cargo("clean"), execs().with_status(0));

    // If you set the env-var, then we expect metadata on libbar
    assert_that(p.cargo("build").arg("-v").env("__CARGO_DEFAULT_LIB_METADATA", "stable"),
                execs().with_status(0).with_stderr(&format!("\
[COMPILING] bar v0.0.1 ({url}/bar)
[RUNNING] `rustc --crate-name bar bar[/]src[/]lib.rs --crate-type dylib \
        --emit=dep-info,link \
        -C prefer-dynamic -C debuginfo=2 \
        -C metadata=[..] \
        --out-dir [..] \
        -L dependency={dir}[/]target[/]debug[/]deps`
[COMPILING] foo v0.0.1 ({url})
[RUNNING] `rustc --crate-name foo src[/]lib.rs --crate-type lib \
        --emit=dep-info,link -C debuginfo=2 \
        -C metadata=[..] \
        -C extra-filename=[..] \
        --out-dir [..] \
        -L dependency={dir}[/]target[/]debug[/]deps \
        --extern bar={dir}[/]target[/]debug[/]deps[/]{prefix}bar-[..]{suffix}`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
dir = p.root().display(),
url = p.url(),
prefix = env::consts::DLL_PREFIX,
suffix = env::consts::DLL_SUFFIX,
)));
}

#[test]
fn crate_env_vars() {
    let p = project("foo")
        .file("Cargo.toml", r#"
        [project]
        name = "foo"
        version = "0.5.1-alpha.1"
        description = "This is foo"
        homepage = "http://example.com"
        authors = ["wycats@example.com"]
        "#)
        .file("src/main.rs", r#"
            extern crate foo;


            static VERSION_MAJOR: &'static str = env!("CARGO_PKG_VERSION_MAJOR");
            static VERSION_MINOR: &'static str = env!("CARGO_PKG_VERSION_MINOR");
            static VERSION_PATCH: &'static str = env!("CARGO_PKG_VERSION_PATCH");
            static VERSION_PRE: &'static str = env!("CARGO_PKG_VERSION_PRE");
            static VERSION: &'static str = env!("CARGO_PKG_VERSION");
            static CARGO_MANIFEST_DIR: &'static str = env!("CARGO_MANIFEST_DIR");
            static PKG_NAME: &'static str = env!("CARGO_PKG_NAME");
            static HOMEPAGE: &'static str = env!("CARGO_PKG_HOMEPAGE");
            static DESCRIPTION: &'static str = env!("CARGO_PKG_DESCRIPTION");

            fn main() {
                let s = format!("{}-{}-{} @ {} in {}", VERSION_MAJOR,
                                VERSION_MINOR, VERSION_PATCH, VERSION_PRE,
                                CARGO_MANIFEST_DIR);
                 assert_eq!(s, foo::version());
                 println!("{}", s);
                 assert_eq!("foo", PKG_NAME);
                 assert_eq!("http://example.com", HOMEPAGE);
                 assert_eq!("This is foo", DESCRIPTION);
                let s = format!("{}.{}.{}-{}", VERSION_MAJOR,
                                VERSION_MINOR, VERSION_PATCH, VERSION_PRE);
                assert_eq!(s, VERSION);
            }
        "#)
        .file("src/lib.rs", r#"
            pub fn version() -> String {
                format!("{}-{}-{} @ {} in {}",
                        env!("CARGO_PKG_VERSION_MAJOR"),
                        env!("CARGO_PKG_VERSION_MINOR"),
                        env!("CARGO_PKG_VERSION_PATCH"),
                        env!("CARGO_PKG_VERSION_PRE"),
                        env!("CARGO_MANIFEST_DIR"))
            }
        "#);

    println!("build");
    assert_that(p.cargo_process("build").arg("-v"), execs().with_status(0));

    println!("bin");
    assert_that(process(&p.bin("foo")),
                execs().with_status(0).with_stdout(&format!("0-5-1 @ alpha.1 in {}\n",
                                                   p.root().display())));

    println!("test");
    assert_that(p.cargo("test").arg("-v"),
                execs().with_status(0));
}

#[test]
fn crate_authors_env_vars() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.5.1-alpha.1"
            authors = ["wycats@example.com", "neikos@example.com"]
        "#)
        .file("src/main.rs", r#"
            extern crate foo;

            static AUTHORS: &'static str = env!("CARGO_PKG_AUTHORS");

            fn main() {
                let s = "wycats@example.com:neikos@example.com";
                assert_eq!(AUTHORS, foo::authors());
                println!("{}", AUTHORS);
                assert_eq!(s, AUTHORS);
            }
        "#)
        .file("src/lib.rs", r#"
            pub fn authors() -> String {
                format!("{}", env!("CARGO_PKG_AUTHORS"))
            }
        "#);

    println!("build");
    assert_that(p.cargo_process("build").arg("-v"), execs().with_status(0));

    println!("bin");
    assert_that(process(&p.bin("foo")),
                execs().with_status(0).with_stdout("wycats@example.com:neikos@example.com"));

    println!("test");
    assert_that(p.cargo("test").arg("-v"),
                execs().with_status(0));
}

// Regression test for #4277
#[test]
fn crate_library_path_env_var() {
    let mut p = project("foo");

    p = p.file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/main.rs", &format!(r##"
            fn main() {{
                let search_path = env!("{}");
                let paths = std::env::split_paths(&search_path).collect::<Vec<_>>();
                assert!(!paths.contains(&"".into()));
            }}
        "##, dylib_path_envvar()));

    assert_that(p.cargo_process("run"), execs().with_status(0));
}

// Regression test for #4277
#[test]
fn build_with_fake_libc_not_loading() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/main.rs", r#"
            fn main() {}
        "#)
        .file("src/lib.rs", r#" "#)
        .file("libc.so.6", r#""#);

    assert_that(p.cargo_process("build"), execs().with_status(0));
}

// this is testing that src/<pkg-name>.rs still works (for now)
#[test]
fn many_crate_types_old_style_lib_location() {
    let mut p = project("foo");
    p = p
        .file("Cargo.toml", r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [lib]

            name = "foo"
            crate_type = ["rlib", "dylib"]
        "#)
        .file("src/foo.rs", r#"
            pub fn foo() {}
        "#);
    assert_that(p.cargo_process("build"), execs().with_status(0).with_stderr_contains("\
[WARNING] path `[..]src[/]foo.rs` was erroneously implicitly accepted for library `foo`,
please rename the file to `src/lib.rs` or set lib.path in Cargo.toml"));

    assert_that(&p.root().join("target/debug/libfoo.rlib"), existing_file());
    let fname = format!("{}foo{}", env::consts::DLL_PREFIX,
                        env::consts::DLL_SUFFIX);
    assert_that(&p.root().join("target/debug").join(&fname), existing_file());
}

#[test]
fn many_crate_types_correct() {
    let mut p = project("foo");
    p = p
        .file("Cargo.toml", r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [lib]

            name = "foo"
            crate_type = ["rlib", "dylib"]
        "#)
        .file("src/lib.rs", r#"
            pub fn foo() {}
        "#);
    assert_that(p.cargo_process("build"),
                execs().with_status(0));

    assert_that(&p.root().join("target/debug/libfoo.rlib"), existing_file());
    let fname = format!("{}foo{}", env::consts::DLL_PREFIX,
                        env::consts::DLL_SUFFIX);
    assert_that(&p.root().join("target/debug").join(&fname), existing_file());
}

#[test]
fn self_dependency() {
    let mut p = project("foo");
    p = p
        .file("Cargo.toml", r#"
            [package]

            name = "test"
            version = "0.0.0"
            authors = []

            [dependencies.test]

            path = "."

            [lib]
            name = "test"
            path = "src/test.rs"
        "#)
        .file("src/test.rs", "fn main() {}");
    assert_that(p.cargo_process("build"),
                execs().with_status(101)
                       .with_stderr("\
[ERROR] cyclic package dependency: package `test v0.0.0 ([..])` depends on itself
"));
}

#[test]
fn ignore_broken_symlinks() {
    // windows and symlinks don't currently agree that well
    if cfg!(windows) { return }

    let p = project("foo")
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]))
        .symlink("Notafile", "bar");

    assert_that(p.cargo_process("build"), execs().with_status(0));
    assert_that(&p.bin("foo"), existing_file());

    assert_that(process(&p.bin("foo")),
                execs().with_status(0).with_stdout("i am foo\n"));
}

#[test]
fn missing_lib_and_bin() {
    let mut p = project("foo");
    p = p
        .file("Cargo.toml", r#"
            [package]

            name = "test"
            version = "0.0.0"
            authors = []
        "#);
    assert_that(p.cargo_process("build"),
                execs().with_status(101)
                       .with_stderr("\
[ERROR] failed to parse manifest at `[..]Cargo.toml`

Caused by:
  no targets specified in the manifest
  either src/lib.rs, src/main.rs, a [lib] section, or [[bin]] section must be present\n"));
}

#[test]
fn lto_build() {
    // FIXME: currently this hits a linker bug on 32-bit MSVC
    if cfg!(all(target_env = "msvc", target_pointer_width = "32")) {
        return
    }

    let mut p = project("foo");
    p = p
        .file("Cargo.toml", r#"
            [package]

            name = "test"
            version = "0.0.0"
            authors = []

            [profile.release]
            lto = true
        "#)
        .file("src/main.rs", "fn main() {}");
    assert_that(p.cargo_process("build").arg("-v").arg("--release"),
                execs().with_status(0).with_stderr(&format!("\
[COMPILING] test v0.0.0 ({url})
[RUNNING] `rustc --crate-name test src[/]main.rs --crate-type bin \
        --emit=dep-info,link \
        -C opt-level=3 \
        -C lto \
        -C metadata=[..] \
        --out-dir {dir}[/]target[/]release[/]deps \
        -L dependency={dir}[/]target[/]release[/]deps`
[FINISHED] release [optimized] target(s) in [..]
",
dir = p.root().display(),
url = p.url(),
)));
}

#[test]
fn verbose_build() {
    let mut p = project("foo");
    p = p
        .file("Cargo.toml", r#"
            [package]

            name = "test"
            version = "0.0.0"
            authors = []
        "#)
        .file("src/lib.rs", "");
    assert_that(p.cargo_process("build").arg("-v"),
                execs().with_status(0).with_stderr(&format!("\
[COMPILING] test v0.0.0 ({url})
[RUNNING] `rustc --crate-name test src[/]lib.rs --crate-type lib \
        --emit=dep-info,link -C debuginfo=2 \
        -C metadata=[..] \
        --out-dir [..] \
        -L dependency={dir}[/]target[/]debug[/]deps`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
dir = p.root().display(),
url = p.url(),
)));
}

#[test]
fn verbose_release_build() {
    let mut p = project("foo");
    p = p
        .file("Cargo.toml", r#"
            [package]

            name = "test"
            version = "0.0.0"
            authors = []
        "#)
        .file("src/lib.rs", "");
    assert_that(p.cargo_process("build").arg("-v").arg("--release"),
                execs().with_status(0).with_stderr(&format!("\
[COMPILING] test v0.0.0 ({url})
[RUNNING] `rustc --crate-name test src[/]lib.rs --crate-type lib \
        --emit=dep-info,link \
        -C opt-level=3 \
        -C metadata=[..] \
        --out-dir [..] \
        -L dependency={dir}[/]target[/]release[/]deps`
[FINISHED] release [optimized] target(s) in [..]
",
dir = p.root().display(),
url = p.url(),
)));
}

#[test]
fn verbose_release_build_deps() {
    let mut p = project("foo");
    p = p
        .file("Cargo.toml", r#"
            [package]

            name = "test"
            version = "0.0.0"
            authors = []

            [dependencies.foo]
            path = "foo"
        "#)
        .file("src/lib.rs", "")
        .file("foo/Cargo.toml", r#"
            [package]

            name = "foo"
            version = "0.0.0"
            authors = []

            [lib]
            name = "foo"
            crate_type = ["dylib", "rlib"]
        "#)
        .file("foo/src/lib.rs", "");
    assert_that(p.cargo_process("build").arg("-v").arg("--release"),
                execs().with_status(0).with_stderr(&format!("\
[COMPILING] foo v0.0.0 ({url}/foo)
[RUNNING] `rustc --crate-name foo foo[/]src[/]lib.rs \
        --crate-type dylib --crate-type rlib \
        --emit=dep-info,link \
        -C prefer-dynamic \
        -C opt-level=3 \
        -C metadata=[..] \
        --out-dir [..] \
        -L dependency={dir}[/]target[/]release[/]deps`
[COMPILING] test v0.0.0 ({url})
[RUNNING] `rustc --crate-name test src[/]lib.rs --crate-type lib \
        --emit=dep-info,link \
        -C opt-level=3 \
        -C metadata=[..] \
        --out-dir [..] \
        -L dependency={dir}[/]target[/]release[/]deps \
        --extern foo={dir}[/]target[/]release[/]deps[/]{prefix}foo{suffix} \
        --extern foo={dir}[/]target[/]release[/]deps[/]libfoo.rlib`
[FINISHED] release [optimized] target(s) in [..]
",
                    dir = p.root().display(),
                    url = p.url(),
                    prefix = env::consts::DLL_PREFIX,
                    suffix = env::consts::DLL_SUFFIX)));
}

#[test]
fn explicit_examples() {
    let mut p = project("world");
    p = p.file("Cargo.toml", r#"
            [package]
            name = "world"
            version = "1.0.0"
            authors = []

            [lib]
            name = "world"
            path = "src/lib.rs"

            [[example]]
            name = "hello"
            path = "examples/ex-hello.rs"

            [[example]]
            name = "goodbye"
            path = "examples/ex-goodbye.rs"
        "#)
        .file("src/lib.rs", r#"
            pub fn get_hello() -> &'static str { "Hello" }
            pub fn get_goodbye() -> &'static str { "Goodbye" }
            pub fn get_world() -> &'static str { "World" }
        "#)
        .file("examples/ex-hello.rs", r#"
            extern crate world;
            fn main() { println!("{}, {}!", world::get_hello(), world::get_world()); }
        "#)
        .file("examples/ex-goodbye.rs", r#"
            extern crate world;
            fn main() { println!("{}, {}!", world::get_goodbye(), world::get_world()); }
        "#);

    assert_that(p.cargo_process("test").arg("-v"), execs().with_status(0));
    assert_that(process(&p.bin("examples/hello")),
                        execs().with_status(0).with_stdout("Hello, World!\n"));
    assert_that(process(&p.bin("examples/goodbye")),
                        execs().with_status(0).with_stdout("Goodbye, World!\n"));
}

#[test]
fn non_existing_example() {
    let mut p = project("world");
    p = p.file("Cargo.toml", r#"
            [package]
            name = "world"
            version = "1.0.0"
            authors = []

            [lib]
            name = "world"
            path = "src/lib.rs"

            [[example]]
            name = "hello"
        "#)
        .file("src/lib.rs", "")
        .file("examples/ehlo.rs", "");

    assert_that(p.cargo_process("test").arg("-v"), execs().with_status(101).with_stderr("\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  can't find `hello` example, specify example.path"));
}

#[test]
fn non_existing_binary() {
    let mut p = project("world");
    p = p.file("Cargo.toml", r#"
            [package]
            name = "world"
            version = "1.0.0"
            authors = []

            [[bin]]
            name = "hello"
        "#)
        .file("src/lib.rs", "")
        .file("src/bin/ehlo.rs", "");

    assert_that(p.cargo_process("build").arg("-v"), execs().with_status(101).with_stderr("\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  can't find `hello` bin, specify bin.path"));
}

#[test]
fn legacy_binary_paths_warinigs() {
    let mut p = project("world");
    p = p.file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "1.0.0"
            authors = []

            [[bin]]
            name = "bar"
        "#)
        .file("src/lib.rs", "")
        .file("src/main.rs", "fn main() {}");

    assert_that(p.cargo_process("build").arg("-v"), execs().with_status(0).with_stderr_contains("\
[WARNING] path `[..]src[/]main.rs` was erroneously implicitly accepted for binary `bar`,
please set bin.path in Cargo.toml"));

    let mut p = project("world");
    p = p.file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "1.0.0"
            authors = []

            [[bin]]
            name = "bar"
        "#)
        .file("src/lib.rs", "")
        .file("src/bin/main.rs", "fn main() {}");

    assert_that(p.cargo_process("build").arg("-v"), execs().with_status(0).with_stderr_contains("\
[WARNING] path `[..]src[/]bin[/]main.rs` was erroneously implicitly accepted for binary `bar`,
please set bin.path in Cargo.toml"));

    let mut p = project("world");
    p = p.file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "1.0.0"
            authors = []

            [[bin]]
            name = "bar"
        "#)
        .file("src/bar.rs", "fn main() {}");

    assert_that(p.cargo_process("build").arg("-v"), execs().with_status(0).with_stderr_contains("\
[WARNING] path `[..]src[/]bar.rs` was erroneously implicitly accepted for binary `bar`,
please set bin.path in Cargo.toml"));
}

#[test]
fn implicit_examples() {
    let mut p = project("world");
    p = p.file("Cargo.toml", r#"
            [package]
            name = "world"
            version = "1.0.0"
            authors = []
        "#)
        .file("src/lib.rs", r#"
            pub fn get_hello() -> &'static str { "Hello" }
            pub fn get_goodbye() -> &'static str { "Goodbye" }
            pub fn get_world() -> &'static str { "World" }
        "#)
        .file("examples/hello.rs", r#"
            extern crate world;
            fn main() {
                println!("{}, {}!", world::get_hello(), world::get_world());
            }
        "#)
        .file("examples/goodbye.rs", r#"
            extern crate world;
            fn main() {
                println!("{}, {}!", world::get_goodbye(), world::get_world());
            }
        "#);

    assert_that(p.cargo_process("test"), execs().with_status(0));
    assert_that(process(&p.bin("examples/hello")),
                execs().with_status(0).with_stdout("Hello, World!\n"));
    assert_that(process(&p.bin("examples/goodbye")),
                execs().with_status(0).with_stdout("Goodbye, World!\n"));
}

#[test]
fn standard_build_no_ndebug() {
    let p = project("world")
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", r#"
            fn main() {
                if cfg!(debug_assertions) {
                    println!("slow")
                } else {
                    println!("fast")
                }
            }
        "#);

    assert_that(p.cargo_process("build"), execs().with_status(0));
    assert_that(process(&p.bin("foo")),
                execs().with_status(0).with_stdout("slow\n"));
}

#[test]
fn release_build_ndebug() {
    let p = project("world")
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", r#"
            fn main() {
                if cfg!(debug_assertions) {
                    println!("slow")
                } else {
                    println!("fast")
                }
            }
        "#);

    assert_that(p.cargo_process("build").arg("--release"),
                execs().with_status(0));
    assert_that(process(&p.release_bin("foo")),
                execs().with_status(0).with_stdout("fast\n"));
}

#[test]
fn inferred_main_bin() {
    let p = project("world")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/main.rs", r#"
            fn main() {}
        "#);

    assert_that(p.cargo_process("build"), execs().with_status(0));
    assert_that(process(&p.bin("foo")), execs().with_status(0));
}

#[test]
fn deletion_causes_failure() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.bar]
            path = "bar"
        "#)
        .file("src/main.rs", r#"
            extern crate bar;
            fn main() {}
        "#)
        .file("bar/Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.0.1"
            authors = []
        "#)
        .file("bar/src/lib.rs", "");
    p.build();

    assert_that(p.cargo("build"), execs().with_status(0));
    p.change_file("Cargo.toml", r#"
        [package]
        name = "foo"
        version = "0.0.1"
        authors = []
    "#);
    assert_that(p.cargo("build"), execs().with_status(101));
}

#[test]
fn bad_cargo_toml_in_target_dir() {
    let p = project("world")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/main.rs", r#"
            fn main() {}
        "#)
        .file("target/Cargo.toml", "bad-toml");

    assert_that(p.cargo_process("build"), execs().with_status(0));
    assert_that(process(&p.bin("foo")), execs().with_status(0));
}

#[test]
fn lib_with_standard_name() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "syntax"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/lib.rs", "
            pub fn foo() {}
        ")
        .file("src/main.rs", "
            extern crate syntax;
            fn main() { syntax::foo() }
        ");

    assert_that(p.cargo_process("build"),
                execs().with_status(0)
                       .with_stderr(&format!("\
[COMPILING] syntax v0.0.1 ({dir})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
                       dir = p.url())));
}

#[test]
fn simple_staticlib() {
    let p = project("foo")
        .file("Cargo.toml", r#"
              [package]
              name = "foo"
              authors = []
              version = "0.0.1"

              [lib]
              name = "foo"
              crate-type = ["staticlib"]
        "#)
        .file("src/lib.rs", "pub fn foo() {}");

    // env var is a test for #1381
    assert_that(p.cargo_process("build").env("RUST_LOG", "nekoneko=trace"),
                execs().with_status(0));
}

#[test]
fn staticlib_rlib_and_bin() {
    let p = project("foo")
        .file("Cargo.toml", r#"
              [package]
              name = "foo"
              authors = []
              version = "0.0.1"

              [lib]
              name = "foo"
              crate-type = ["staticlib", "rlib"]
        "#)
        .file("src/lib.rs", "pub fn foo() {}")
        .file("src/main.rs", r#"
              extern crate foo;

              fn main() {
                  foo::foo();
              }"#);

    assert_that(p.cargo_process("build").arg("-v"), execs().with_status(0));
}

#[test]
fn opt_out_of_bin() {
    let p = project("foo")
        .file("Cargo.toml", r#"
              bin = []

              [package]
              name = "foo"
              authors = []
              version = "0.0.1"
        "#)
        .file("src/lib.rs", "")
        .file("src/main.rs", "bad syntax");
    assert_that(p.cargo_process("build"), execs().with_status(0));
}

#[test]
fn single_lib() {
    let p = project("foo")
        .file("Cargo.toml", r#"
              [package]
              name = "foo"
              authors = []
              version = "0.0.1"

              [lib]
              name = "foo"
              path = "src/bar.rs"
        "#)
        .file("src/bar.rs", "");
    assert_that(p.cargo_process("build"), execs().with_status(0));
}

#[test]
fn freshness_ignores_excluded() {
    let foo = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.0"
            authors = []
            build = "build.rs"
            exclude = ["src/b*.rs"]
        "#)
        .file("build.rs", "fn main() {}")
        .file("src/lib.rs", "pub fn bar() -> i32 { 1 }");
    foo.build();
    foo.root().move_into_the_past();

    assert_that(foo.cargo("build"),
                execs().with_status(0)
                       .with_stderr(&format!("\
[COMPILING] foo v0.0.0 ({url})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
", url = foo.url())));

    // Smoke test to make sure it doesn't compile again
    println!("first pass");
    assert_that(foo.cargo("build"),
                execs().with_status(0)
                       .with_stdout(""));

    // Modify an ignored file and make sure we don't rebuild
    println!("second pass");
    File::create(&foo.root().join("src/bar.rs")).unwrap();
    assert_that(foo.cargo("build"),
                execs().with_status(0)
                       .with_stdout(""));
}

#[test]
fn rebuild_preserves_out_dir() {
    let foo = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.0"
            authors = []
            build = 'build.rs'
        "#)
        .file("build.rs", r#"
            use std::env;
            use std::fs::File;
            use std::path::Path;

            fn main() {
                let path = Path::new(&env::var("OUT_DIR").unwrap()).join("foo");
                if env::var_os("FIRST").is_some() {
                    File::create(&path).unwrap();
                } else {
                    File::create(&path).unwrap();
                }
            }
        "#)
        .file("src/lib.rs", "pub fn bar() -> i32 { 1 }");
    foo.build();
    foo.root().move_into_the_past();

    assert_that(foo.cargo("build").env("FIRST", "1"),
                execs().with_status(0)
                       .with_stderr(&format!("\
[COMPILING] foo v0.0.0 ({url})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
", url = foo.url())));

    File::create(&foo.root().join("src/bar.rs")).unwrap();
    assert_that(foo.cargo("build"),
                execs().with_status(0)
                       .with_stderr(&format!("\
[COMPILING] foo v0.0.0 ({url})
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
", url = foo.url())));
}

#[test]
fn dep_no_libs() {
    let foo = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.0"
            authors = []

            [dependencies.bar]
            path = "bar"
        "#)
        .file("src/lib.rs", "pub fn bar() -> i32 { 1 }")
        .file("bar/Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.0.0"
            authors = []
        "#)
        .file("bar/src/main.rs", "");
    assert_that(foo.cargo_process("build"),
                execs().with_status(0));
}

#[test]
fn recompile_space_in_name() {
    let foo = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.0"
            authors = []

            [lib]
            name = "foo"
            path = "src/my lib.rs"
        "#)
        .file("src/my lib.rs", "");
    assert_that(foo.cargo_process("build"), execs().with_status(0));
    foo.root().move_into_the_past();
    assert_that(foo.cargo("build"),
                execs().with_status(0).with_stdout(""));
}

#[cfg(unix)]
#[test]
fn ignore_bad_directories() {
    use std::os::unix::prelude::*;
    let foo = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.0"
            authors = []
        "#)
        .file("src/lib.rs", "");
    foo.build();
    let dir = foo.root().join("tmp");
    fs::create_dir(&dir).unwrap();
    let stat = fs::metadata(&dir).unwrap();
    let mut perms = stat.permissions();
    perms.set_mode(0o644);
    fs::set_permissions(&dir, perms.clone()).unwrap();
    assert_that(foo.cargo("build"),
                execs().with_status(0));
    perms.set_mode(0o755);
    fs::set_permissions(&dir, perms).unwrap();
}

#[test]
fn bad_cargo_config() {
    let foo = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.0"
            authors = []
        "#)
        .file("src/lib.rs", "")
        .file(".cargo/config", r#"
              this is not valid toml
        "#);
    assert_that(foo.cargo_process("build").arg("-v"),
                execs().with_status(101).with_stderr("\
[ERROR] Couldn't load Cargo configuration

Caused by:
  could not parse TOML configuration in `[..]`

Caused by:
  could not parse input as TOML

Caused by:
  expected an equals, found an identifier at line 2
"));
}

#[test]
fn cargo_platform_specific_dependency() {
    let host = rustc_host();
    let p = project("foo")
        .file("Cargo.toml", &format!(r#"
            [project]
            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]
            build = "build.rs"

            [target.{host}.dependencies]
            dep = {{ path = "dep" }}
            [target.{host}.build-dependencies]
            build = {{ path = "build" }}
            [target.{host}.dev-dependencies]
            dev = {{ path = "dev" }}
        "#, host = host))
        .file("src/main.rs", r#"
            extern crate dep;
            fn main() { dep::dep() }
        "#)
        .file("tests/foo.rs", r#"
            extern crate dev;
            #[test]
            fn foo() { dev::dev() }
        "#)
        .file("build.rs", r#"
            extern crate build;
            fn main() { build::build(); }
        "#)
        .file("dep/Cargo.toml", r#"
            [project]
            name = "dep"
            version = "0.5.0"
            authors = ["wycats@example.com"]
        "#)
        .file("dep/src/lib.rs", "pub fn dep() {}")
        .file("build/Cargo.toml", r#"
            [project]
            name = "build"
            version = "0.5.0"
            authors = ["wycats@example.com"]
        "#)
        .file("build/src/lib.rs", "pub fn build() {}")
        .file("dev/Cargo.toml", r#"
            [project]
            name = "dev"
            version = "0.5.0"
            authors = ["wycats@example.com"]
        "#)
        .file("dev/src/lib.rs", "pub fn dev() {}");

    assert_that(p.cargo_process("build"),
                execs().with_status(0));

    assert_that(&p.bin("foo"), existing_file());
    assert_that(p.cargo("test"),
                execs().with_status(0));
}

#[test]
fn bad_platform_specific_dependency() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [target.wrong-target.dependencies.bar]
            path = "bar"
        "#)
        .file("src/main.rs",
              &main_file(r#""{}", bar::gimme()"#, &["bar"]))
        .file("bar/Cargo.toml", r#"
            [project]

            name = "bar"
            version = "0.5.0"
            authors = ["wycats@example.com"]
        "#)
        .file("bar/src/lib.rs", r#"
            extern crate baz;

            pub fn gimme() -> String {
                format!("")
            }
        "#);

    assert_that(p.cargo_process("build"),
                execs().with_status(101));
}

#[test]
fn cargo_platform_specific_dependency_wrong_platform() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [target.non-existing-triplet.dependencies.bar]
            path = "bar"
        "#)
        .file("src/main.rs", r#"
            fn main() {}
        "#)
        .file("bar/Cargo.toml", r#"
            [project]

            name = "bar"
            version = "0.5.0"
            authors = ["wycats@example.com"]
        "#)
        .file("bar/src/lib.rs", r#"
            invalid rust file, should not be compiled
        "#);

    p.cargo_process("build").exec_with_output().unwrap();

    assert_that(&p.bin("foo"), existing_file());
    assert_that(process(&p.bin("foo")),
                execs().with_status(0));

    let loc = p.root().join("Cargo.lock");
    let mut lockfile = String::new();
    File::open(&loc).unwrap().read_to_string(&mut lockfile).unwrap();
    assert!(lockfile.contains("bar"))
}

#[test]
fn example_as_lib() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [[example]]
            name = "ex"
            crate-type = ["lib"]
        "#)
        .file("src/lib.rs", "")
        .file("examples/ex.rs", "");

    assert_that(p.cargo_process("build").arg("--example=ex"), execs().with_status(0));
    assert_that(&p.example_lib("ex", "lib"), existing_file());
}

#[test]
fn example_as_rlib() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [[example]]
            name = "ex"
            crate-type = ["rlib"]
        "#)
        .file("src/lib.rs", "")
        .file("examples/ex.rs", "");

    assert_that(p.cargo_process("build").arg("--example=ex"), execs().with_status(0));
    assert_that(&p.example_lib("ex", "rlib"), existing_file());
}

#[test]
fn example_as_dylib() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [[example]]
            name = "ex"
            crate-type = ["dylib"]
        "#)
        .file("src/lib.rs", "")
        .file("examples/ex.rs", "");

    assert_that(p.cargo_process("build").arg("--example=ex"), execs().with_status(0));
    assert_that(&p.example_lib("ex", "dylib"), existing_file());
}

#[test]
fn example_as_proc_macro() {
    if !is_nightly() {
        return;
    }

    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [[example]]
            name = "ex"
            crate-type = ["proc-macro"]
        "#)
        .file("src/lib.rs", "")
        .file("examples/ex.rs", "#![feature(proc_macro)]");

    assert_that(p.cargo_process("build").arg("--example=ex"), execs().with_status(0));
    assert_that(&p.example_lib("ex", "proc-macro"), existing_file());
}

#[test]
fn example_bin_same_name() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/main.rs", "fn main() {}")
        .file("examples/foo.rs", "fn main() {}");

    p.cargo_process("test").arg("--no-run").arg("-v")
        .exec_with_output()
        .unwrap();

    assert_that(&p.bin("foo"), is_not(existing_file()));
    // We expect a file of the form bin/foo-{metadata_hash}
    assert_that(&p.bin("examples/foo"), existing_file());

    p.cargo("test").arg("--no-run").arg("-v")
                   .exec_with_output()
                   .unwrap();

    assert_that(&p.bin("foo"), is_not(existing_file()));
    // We expect a file of the form bin/foo-{metadata_hash}
    assert_that(&p.bin("examples/foo"), existing_file());
}

#[test]
fn compile_then_delete() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/main.rs", "fn main() {}");

    assert_that(p.cargo_process("run").arg("-v"), execs().with_status(0));
    assert_that(&p.bin("foo"), existing_file());
    if cfg!(windows) {
        // On windows unlinking immediately after running often fails, so sleep
        sleep_ms(100);
    }
    fs::remove_file(&p.bin("foo")).unwrap();
    assert_that(p.cargo("run").arg("-v"),
                execs().with_status(0));
}

#[test]
fn transitive_dependencies_not_available() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.aaaaa]
            path = "a"
        "#)
        .file("src/main.rs", "extern crate bbbbb; extern crate aaaaa; fn main() {}")
        .file("a/Cargo.toml", r#"
            [package]
            name = "aaaaa"
            version = "0.0.1"
            authors = []

            [dependencies.bbbbb]
            path = "../b"
        "#)
        .file("a/src/lib.rs", "extern crate bbbbb;")
        .file("b/Cargo.toml", r#"
            [package]
            name = "bbbbb"
            version = "0.0.1"
            authors = []
        "#)
        .file("b/src/lib.rs", "");

    assert_that(p.cargo_process("build").arg("-v"),
                execs().with_status(101)
                       .with_stderr_contains("\
[..] can't find crate for `bbbbb`[..]
"));
}

#[test]
fn cyclic_deps_rejected() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.a]
            path = "a"
        "#)
        .file("src/lib.rs", "")
        .file("a/Cargo.toml", r#"
            [package]
            name = "a"
            version = "0.0.1"
            authors = []

            [dependencies.foo]
            path = ".."
        "#)
        .file("a/src/lib.rs", "");

    assert_that(p.cargo_process("build").arg("-v"),
                execs().with_status(101)
                       .with_stderr("\
[ERROR] cyclic package dependency: package `a v0.0.1 ([..])` depends on itself
"));
}

#[test]
fn predictable_filenames() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [lib]
            name = "foo"
            crate-type = ["dylib", "rlib"]
        "#)
        .file("src/lib.rs", "");

    assert_that(p.cargo_process("build").arg("-v"),
                execs().with_status(0));
    assert_that(&p.root().join("target/debug/libfoo.rlib"), existing_file());
    let dylib_name = format!("{}foo{}", env::consts::DLL_PREFIX,
                             env::consts::DLL_SUFFIX);
    assert_that(&p.root().join("target/debug").join(dylib_name),
                existing_file());
}

#[test]
fn dashes_to_underscores() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo-bar"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/lib.rs", "")
        .file("src/main.rs", "extern crate foo_bar; fn main() {}");

    assert_that(p.cargo_process("build").arg("-v"),
                execs().with_status(0));
    assert_that(&p.bin("foo-bar"), existing_file());
}

#[test]
fn dashes_in_crate_name_bad() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [lib]
            name = "foo-bar"
        "#)
        .file("src/lib.rs", "")
        .file("src/main.rs", "extern crate foo_bar; fn main() {}");

    assert_that(p.cargo_process("build").arg("-v"),
                execs().with_status(101));
}

#[test]
fn rustc_env_var() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/lib.rs", "");
    p.build();

    assert_that(p.cargo("build")
                 .env("RUSTC", "rustc-that-does-not-exist").arg("-v"),
                execs().with_status(101)
                       .with_stderr("\
[ERROR] could not execute process `rustc-that-does-not-exist -vV` ([..])

Caused by:
[..]
"));
    assert_that(&p.bin("a"), is_not(existing_file()));
}

#[test]
fn filtering() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/lib.rs", "")
        .file("src/bin/a.rs", "fn main() {}")
        .file("src/bin/b.rs", "fn main() {}")
        .file("examples/a.rs", "fn main() {}")
        .file("examples/b.rs", "fn main() {}");
    p.build();

    assert_that(p.cargo("build").arg("--lib"),
                execs().with_status(0));
    assert_that(&p.bin("a"), is_not(existing_file()));

    assert_that(p.cargo("build").arg("--bin=a").arg("--example=a"),
                execs().with_status(0));
    assert_that(&p.bin("a"), existing_file());
    assert_that(&p.bin("b"), is_not(existing_file()));
    assert_that(&p.bin("examples/a"), existing_file());
    assert_that(&p.bin("examples/b"), is_not(existing_file()));
}

#[test]
fn filtering_implicit_bins() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/lib.rs", "")
        .file("src/bin/a.rs", "fn main() {}")
        .file("src/bin/b.rs", "fn main() {}")
        .file("examples/a.rs", "fn main() {}")
        .file("examples/b.rs", "fn main() {}");
    p.build();

    assert_that(p.cargo("build").arg("--bins"),
                execs().with_status(0));
    assert_that(&p.bin("a"), existing_file());
    assert_that(&p.bin("b"), existing_file());
    assert_that(&p.bin("examples/a"), is_not(existing_file()));
    assert_that(&p.bin("examples/b"), is_not(existing_file()));
}

#[test]
fn filtering_implicit_examples() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/lib.rs", "")
        .file("src/bin/a.rs", "fn main() {}")
        .file("src/bin/b.rs", "fn main() {}")
        .file("examples/a.rs", "fn main() {}")
        .file("examples/b.rs", "fn main() {}");
    p.build();

    assert_that(p.cargo("build").arg("--examples"),
                execs().with_status(0));
    assert_that(&p.bin("a"), is_not(existing_file()));
    assert_that(&p.bin("b"), is_not(existing_file()));
    assert_that(&p.bin("examples/a"), existing_file());
    assert_that(&p.bin("examples/b"), existing_file());
}

#[test]
fn ignore_dotfile() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/bin/.a.rs", "")
        .file("src/bin/a.rs", "fn main() {}");
    p.build();

    assert_that(p.cargo("build"),
                execs().with_status(0));
}

#[test]
fn ignore_dotdirs() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/bin/a.rs", "fn main() {}")
        .file(".git/Cargo.toml", "")
        .file(".pc/dummy-fix.patch/Cargo.toml", "");
    p.build();

    assert_that(p.cargo("build"),
                execs().with_status(0));
}

#[test]
fn dotdir_root() {
    let p = ProjectBuilder::new("foo", root().join(".foo"))
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/bin/a.rs", "fn main() {}");
    p.build();
    assert_that(p.cargo("build"),
                execs().with_status(0));
}


#[test]
fn custom_target_dir() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/main.rs", "fn main() {}");
    p.build();

    let exe_name = format!("foo{}", env::consts::EXE_SUFFIX);

    assert_that(p.cargo("build").env("CARGO_TARGET_DIR", "foo/target"),
                execs().with_status(0));
    assert_that(&p.root().join("foo/target/debug").join(&exe_name),
                existing_file());
    assert_that(&p.root().join("target/debug").join(&exe_name),
                is_not(existing_file()));

    assert_that(p.cargo("build"),
                execs().with_status(0));
    assert_that(&p.root().join("foo/target/debug").join(&exe_name),
                existing_file());
    assert_that(&p.root().join("target/debug").join(&exe_name),
                existing_file());

    fs::create_dir(p.root().join(".cargo")).unwrap();
    File::create(p.root().join(".cargo/config")).unwrap().write_all(br#"
        [build]
        target-dir = "foo/target"
    "#).unwrap();
    assert_that(p.cargo("build").env("CARGO_TARGET_DIR", "bar/target"),
                execs().with_status(0));
    assert_that(&p.root().join("bar/target/debug").join(&exe_name),
                existing_file());
    assert_that(&p.root().join("foo/target/debug").join(&exe_name),
                existing_file());
    assert_that(&p.root().join("target/debug").join(&exe_name),
                existing_file());
}

#[test]
fn rustc_no_trans() {
    if !is_nightly() { return }

    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/main.rs", "fn main() {}");
    p.build();

    assert_that(p.cargo("rustc").arg("-v").arg("--").arg("-Zno-trans"),
                execs().with_status(0));
}

#[test]
fn build_multiple_packages() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.d1]
                path = "d1"
            [dependencies.d2]
                path = "d2"

            [[bin]]
                name = "foo"
        "#)
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]))
        .file("d1/Cargo.toml", r#"
            [package]
            name = "d1"
            version = "0.0.1"
            authors = []

            [[bin]]
                name = "d1"
        "#)
        .file("d1/src/lib.rs", "")
        .file("d1/src/main.rs", "fn main() { println!(\"d1\"); }")
        .file("d2/Cargo.toml", r#"
            [package]
            name = "d2"
            version = "0.0.1"
            authors = []

            [[bin]]
                name = "d2"
                doctest = false
        "#)
        .file("d2/src/main.rs", "fn main() { println!(\"d2\"); }");

    assert_that(p.cargo_process("build").arg("-p").arg("d1").arg("-p").arg("d2")
                                        .arg("-p").arg("foo"),
                execs().with_status(0));

    assert_that(&p.bin("foo"), existing_file());
    assert_that(process(&p.bin("foo")),
                execs().with_status(0).with_stdout("i am foo\n"));

    let d1_path = &p.build_dir().join("debug")
                                .join(format!("d1{}", env::consts::EXE_SUFFIX));
    let d2_path = &p.build_dir().join("debug")
                                .join(format!("d2{}", env::consts::EXE_SUFFIX));

    assert_that(d1_path, existing_file());
    assert_that(process(d1_path), execs().with_status(0).with_stdout("d1"));

    assert_that(d2_path, existing_file());
    assert_that(process(d2_path),
                execs().with_status(0).with_stdout("d2"));
}

#[test]
fn invalid_spec() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.d1]
                path = "d1"

            [[bin]]
                name = "foo"
        "#)
        .file("src/bin/foo.rs", &main_file(r#""i am foo""#, &[]))
        .file("d1/Cargo.toml", r#"
            [package]
            name = "d1"
            version = "0.0.1"
            authors = []

            [[bin]]
                name = "d1"
        "#)
        .file("d1/src/lib.rs", "")
        .file("d1/src/main.rs", "fn main() { println!(\"d1\"); }");
    p.build();

    assert_that(p.cargo("build").arg("-p").arg("notAValidDep"),
                execs().with_status(101).with_stderr("\
[ERROR] package id specification `notAValidDep` matched no packages
"));

    assert_that(p.cargo("build").arg("-p").arg("d1").arg("-p").arg("notAValidDep"),
                execs().with_status(101).with_stderr("\
[ERROR] package id specification `notAValidDep` matched no packages
"));
}

#[test]
fn manifest_with_bom_is_ok() {
    let p = project("foo")
        .file("Cargo.toml", "\u{FEFF}
            [package]
            name = \"foo\"
            version = \"0.0.1\"
            authors = []
        ")
        .file("src/lib.rs", "");
    assert_that(p.cargo_process("build").arg("-v"),
                execs().with_status(0));
}

#[test]
fn panic_abort_compiles_with_panic_abort() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [profile.dev]
            panic = 'abort'
        "#)
        .file("src/lib.rs", "");
    assert_that(p.cargo_process("build").arg("-v"),
                execs().with_status(0)
                       .with_stderr_contains("[..] -C panic=abort [..]"));
}

#[test]
fn explicit_color_config_is_propagated_to_rustc() {
    let p = project("foo")
        .file("Cargo.toml", r#"
                [package]

                name = "test"
                version = "0.0.0"
                authors = []
            "#)
        .file("src/lib.rs", "");
    p.build();
    assert_that(p.cargo("build").arg("-v").arg("--color").arg("always"),
                execs().with_status(0).with_stderr_contains(
                    "[..]rustc [..] src[/]lib.rs --color always[..]"));

    assert_that(p.cargo("clean"), execs().with_status(0));

    assert_that(p.cargo("build").arg("-v").arg("--color").arg("never"),
                execs().with_status(0).with_stderr("\
[COMPILING] test v0.0.0 ([..])
[RUNNING] `rustc [..] --color never [..]`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
"));
}

#[test]
fn compiler_json_error_format() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dependencies.bar]
            path = "bar"
        "#)
        .file("src/main.rs", "fn main() { let unused = 92; }")
        .file("bar/Cargo.toml", r#"
            [project]

            name = "bar"
            version = "0.5.0"
            authors = ["wycats@example.com"]
        "#)
        .file("bar/src/lib.rs", r#"fn dead() {}"#);
    p.build();

    assert_that(p.cargo("build").arg("-v")
                    .arg("--message-format").arg("json"),
                execs().with_status(0).with_json(r#"
    {
        "reason":"compiler-message",
        "package_id":"bar 0.5.0 ([..])",
        "target":{
            "kind":["lib"],
            "crate_types":["lib"],
            "name":"bar",
            "src_path":"[..]lib.rs"
        },
        "message":"{...}"
    }

    {
        "reason":"compiler-artifact",
        "profile": {
            "debug_assertions": true,
            "debuginfo": 2,
            "opt_level": "0",
            "overflow_checks": true,
            "test": false
        },
        "features": [],
        "package_id":"bar 0.5.0 ([..])",
        "target":{
            "kind":["lib"],
            "crate_types":["lib"],
            "name":"bar",
            "src_path":"[..]lib.rs"
        },
        "filenames":["[..].rlib"],
        "fresh": false
    }

    {
        "reason":"compiler-message",
        "package_id":"foo 0.5.0 ([..])",
        "target":{
            "kind":["bin"],
            "crate_types":["bin"],
            "name":"foo",
            "src_path":"[..]main.rs"
        },
        "message":"{...}"
    }

    {
        "reason":"compiler-artifact",
        "package_id":"foo 0.5.0 ([..])",
        "target":{
            "kind":["bin"],
            "crate_types":["bin"],
            "name":"foo",
            "src_path":"[..]main.rs"
        },
        "profile": {
            "debug_assertions": true,
            "debuginfo": 2,
            "opt_level": "0",
            "overflow_checks": true,
            "test": false
        },
        "features": [],
        "filenames": ["[..]"],
        "fresh": false
    }
"#));

    // With fresh build, we should repeat the artifacts,
    // but omit compiler warnings.
    assert_that(p.cargo("build").arg("-v")
                    .arg("--message-format").arg("json"),
                execs().with_status(0).with_json(r#"
    {
        "reason":"compiler-artifact",
        "profile": {
            "debug_assertions": true,
            "debuginfo": 2,
            "opt_level": "0",
            "overflow_checks": true,
            "test": false
        },
        "features": [],
        "package_id":"bar 0.5.0 ([..])",
        "target":{
            "kind":["lib"],
            "crate_types":["lib"],
            "name":"bar",
            "src_path":"[..]lib.rs"
        },
        "filenames":["[..].rlib"],
        "fresh": true
    }

    {
        "reason":"compiler-artifact",
        "package_id":"foo 0.5.0 ([..])",
        "target":{
            "kind":["bin"],
            "crate_types":["bin"],
            "name":"foo",
            "src_path":"[..]main.rs"
        },
        "profile": {
            "debug_assertions": true,
            "debuginfo": 2,
            "opt_level": "0",
            "overflow_checks": true,
            "test": false
        },
        "features": [],
        "filenames": ["[..]"],
        "fresh": true
    }
"#));
}

#[test]
fn wrong_message_format_option() {
    let p = project("foo")
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", "fn main() {}");

    assert_that(p.cargo_process("build").arg("--message-format").arg("XML"),
                execs().with_status(1)
                       .with_stderr_contains(
r#"[ERROR] Could not match 'xml' with any of the allowed variants: ["Human", "Json"]"#));
}

#[test]
fn message_format_json_forward_stderr() {
    let p = project("foo")
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", "fn main() { let unused = 0; }");

    assert_that(p.cargo_process("rustc").arg("--bin").arg("foo")
                .arg("--message-format").arg("JSON"),
                execs().with_status(0)
                .with_json(r#"
    {
        "reason":"compiler-message",
        "package_id":"foo 0.5.0 ([..])",
        "target":{
            "kind":["bin"],
            "crate_types":["bin"],
            "name":"foo",
            "src_path":"[..]"
        },
        "message":"{...}"
    }

    {
        "reason":"compiler-artifact",
        "package_id":"foo 0.5.0 ([..])",
        "target":{
            "kind":["bin"],
            "crate_types":["bin"],
            "name":"foo",
            "src_path":"[..]"
        },
        "profile":{
            "debug_assertions":true,
            "debuginfo":2,
            "opt_level":"0",
            "overflow_checks": true,
            "test":false
        },
        "features":[],
        "filenames":["[..]"],
        "fresh": false
    }
"#));
}

#[test]
fn no_warn_about_package_metadata() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []

            [package.metadata]
            foo = "bar"
            a = true
            b = 3

            [package.metadata.another]
            bar = 3
        "#)
        .file("src/lib.rs", "");
    assert_that(p.cargo_process("build"),
                execs().with_status(0)
                       .with_stderr("[..] foo v0.0.1 ([..])\n\
                       [FINISHED] dev [unoptimized + debuginfo] target(s) in [..]\n"));
}

#[test]
fn cargo_build_empty_target() {
    let p = project("foo")
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/main.rs", "fn main() {}");

    assert_that(p.cargo_process("build").arg("--target").arg(""),
                execs().with_status(101)
                .with_stderr_contains("[..] target was empty"));
}

#[test]
fn build_all_workspace() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.1.0"

            [dependencies]
            bar = { path = "bar" }

            [workspace]
        "#)
        .file("src/main.rs", r#"
            fn main() {}
        "#)
        .file("bar/Cargo.toml", r#"
            [project]
            name = "bar"
            version = "0.1.0"
        "#)
        .file("bar/src/lib.rs", r#"
            pub fn bar() {}
        "#);

    assert_that(p.cargo_process("build")
                 .arg("--all"),
                execs().with_status(0)
                       .with_stderr("[..] Compiling bar v0.1.0 ([..])\n\
                       [..] Compiling foo v0.1.0 ([..])\n\
                       [..] Finished dev [unoptimized + debuginfo] target(s) in [..]\n"));
}

#[test]
fn build_all_exclude() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.1.0"

            [workspace]
            members = ["bar", "baz"]
        "#)
        .file("src/main.rs", r#"
            fn main() {}
        "#)
        .file("bar/Cargo.toml", r#"
            [project]
            name = "bar"
            version = "0.1.0"
        "#)
        .file("bar/src/lib.rs", r#"
            pub fn bar() {}
        "#)
        .file("baz/Cargo.toml", r#"
            [project]
            name = "baz"
            version = "0.1.0"
        "#)
        .file("baz/src/lib.rs", r#"
            pub fn baz() {
                break_the_build();
            }
        "#);

    assert_that(p.cargo_process("build")
                 .arg("--all")
                 .arg("--exclude")
                 .arg("baz"),
                execs().with_status(0)
                       .with_stderr_contains("[..]Compiling foo v0.1.0 [..]")
                       .with_stderr_contains("[..]Compiling bar v0.1.0 [..]")
                       .with_stderr_does_not_contain("[..]Compiling baz v0.1.0 [..]"));
}

#[test]
fn build_all_workspace_implicit_examples() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.1.0"

            [dependencies]
            bar = { path = "bar" }

            [workspace]
        "#)
        .file("src/lib.rs", "")
        .file("src/bin/a.rs", "fn main() {}")
        .file("src/bin/b.rs", "fn main() {}")
        .file("examples/c.rs", "fn main() {}")
        .file("examples/d.rs", "fn main() {}")
        .file("bar/Cargo.toml", r#"
            [project]
            name = "bar"
            version = "0.1.0"
        "#)
        .file("bar/src/lib.rs", "")
        .file("bar/src/bin/e.rs", "fn main() {}")
        .file("bar/src/bin/f.rs", "fn main() {}")
        .file("bar/examples/g.rs", "fn main() {}")
        .file("bar/examples/h.rs", "fn main() {}");

    assert_that(p.cargo_process("build")
                 .arg("--all").arg("--examples"),
                execs().with_status(0)
                       .with_stderr("[..] Compiling bar v0.1.0 ([..])\n\
                       [..] Compiling foo v0.1.0 ([..])\n\
                       [..] Finished dev [unoptimized + debuginfo] target(s) in [..]\n"));
    assert_that(&p.bin("a"), is_not(existing_file()));
    assert_that(&p.bin("b"), is_not(existing_file()));
    assert_that(&p.bin("examples/c"), existing_file());
    assert_that(&p.bin("examples/d"), existing_file());
    assert_that(&p.bin("e"), is_not(existing_file()));
    assert_that(&p.bin("f"), is_not(existing_file()));
    assert_that(&p.bin("examples/g"), existing_file());
    assert_that(&p.bin("examples/h"), existing_file());
}

#[test]
fn build_all_virtual_manifest() {
    let p = project("workspace")
        .file("Cargo.toml", r#"
            [workspace]
            members = ["foo", "bar"]
        "#)
        .file("foo/Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.1.0"
        "#)
        .file("foo/src/lib.rs", r#"
            pub fn foo() {}
        "#)
        .file("bar/Cargo.toml", r#"
            [project]
            name = "bar"
            version = "0.1.0"
        "#)
        .file("bar/src/lib.rs", r#"
            pub fn bar() {}
        "#);

    // The order in which foo and bar are built is not guaranteed
    assert_that(p.cargo_process("build")
                 .arg("--all"),
                execs().with_status(0)
                       .with_stderr_contains("[..] Compiling bar v0.1.0 ([..])")
                       .with_stderr_contains("[..] Compiling foo v0.1.0 ([..])")
                       .with_stderr("[..] Compiling [..] v0.1.0 ([..])\n\
                       [..] Compiling [..] v0.1.0 ([..])\n\
                       [..] Finished dev [unoptimized + debuginfo] target(s) in [..]\n"));
}

#[test]
fn build_all_virtual_manifest_implicit_examples() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [workspace]
            members = ["foo", "bar"]
        "#)
        .file("foo/Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.1.0"
        "#)
        .file("foo/src/lib.rs", "")
        .file("foo/src/bin/a.rs", "fn main() {}")
        .file("foo/src/bin/b.rs", "fn main() {}")
        .file("foo/examples/c.rs", "fn main() {}")
        .file("foo/examples/d.rs", "fn main() {}")
        .file("bar/Cargo.toml", r#"
            [project]
            name = "bar"
            version = "0.1.0"
        "#)
        .file("bar/src/lib.rs", "")
        .file("bar/src/bin/e.rs", "fn main() {}")
        .file("bar/src/bin/f.rs", "fn main() {}")
        .file("bar/examples/g.rs", "fn main() {}")
        .file("bar/examples/h.rs", "fn main() {}");

    // The order in which foo and bar are built is not guaranteed
    assert_that(p.cargo_process("build")
                 .arg("--all").arg("--examples"),
                execs().with_status(0)
                       .with_stderr_contains("[..] Compiling bar v0.1.0 ([..])")
                       .with_stderr_contains("[..] Compiling foo v0.1.0 ([..])")
                       .with_stderr("[..] Compiling [..] v0.1.0 ([..])\n\
                       [..] Compiling [..] v0.1.0 ([..])\n\
                       [..] Finished dev [unoptimized + debuginfo] target(s) in [..]\n"));
    assert_that(&p.bin("a"), is_not(existing_file()));
    assert_that(&p.bin("b"), is_not(existing_file()));
    assert_that(&p.bin("examples/c"), existing_file());
    assert_that(&p.bin("examples/d"), existing_file());
    assert_that(&p.bin("e"), is_not(existing_file()));
    assert_that(&p.bin("f"), is_not(existing_file()));
    assert_that(&p.bin("examples/g"), existing_file());
    assert_that(&p.bin("examples/h"), existing_file());
}

#[test]
fn build_all_member_dependency_same_name() {
    let p = project("workspace")
        .file("Cargo.toml", r#"
            [workspace]
            members = ["a"]
        "#)
        .file("a/Cargo.toml", r#"
            [project]
            name = "a"
            version = "0.1.0"

            [dependencies]
            a = "0.1.0"
        "#)
        .file("a/src/lib.rs", r#"
            pub fn a() {}
        "#);

    Package::new("a", "0.1.0").publish();

    assert_that(p.cargo_process("build")
                 .arg("--all"),
                execs().with_status(0)
                       .with_stderr("[..] Updating registry `[..]`\n\
                       [..] Downloading a v0.1.0 ([..])\n\
                       [..] Compiling a v0.1.0\n\
                       [..] Compiling a v0.1.0 ([..])\n\
                       [..] Finished dev [unoptimized + debuginfo] target(s) in [..]\n"));
}

#[test]
fn run_proper_binary() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            authors = []
            version = "0.0.0"
            [[bin]]
            name = "main"
            [[bin]]
            name = "other"
        "#)
        .file("src/lib.rs", "")
        .file("src/bin/main.rs", r#"
            fn main() {
                panic!("This should never be run.");
            }
        "#)
        .file("src/bin/other.rs", r#"
            fn main() {
            }
        "#);

    assert_that(p.cargo_process("run").arg("--bin").arg("other"),
                execs().with_status(0));
}

#[test]
fn run_proper_binary_main_rs() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            authors = []
            version = "0.0.0"
            [[bin]]
            name = "foo"
        "#)
        .file("src/lib.rs", "")
        .file("src/bin/main.rs", r#"
            fn main() {
            }
        "#);

    assert_that(p.cargo_process("run").arg("--bin").arg("foo"),
                execs().with_status(0));
}

#[test]
fn run_proper_alias_binary_from_src() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            authors = []
            version = "0.0.0"
            [[bin]]
            name = "foo"
            [[bin]]
            name = "bar"
        "#)
        .file("src/foo.rs", r#"
            fn main() {
              println!("foo");
            }
        "#).file("src/bar.rs", r#"
            fn main() {
              println!("bar");
            }
        "#);

    assert_that(p.cargo_process("build")
                 .arg("--all"),
                execs().with_status(0)
                );
    assert_that(process(&p.bin("foo")),
                execs().with_status(0).with_stdout("foo\n"));
    assert_that(process(&p.bin("bar")),
                execs().with_status(0).with_stdout("bar\n"));
}

#[test]
fn run_proper_alias_binary_main_rs() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            authors = []
            version = "0.0.0"
            [[bin]]
            name = "foo"
            [[bin]]
            name = "bar"
        "#)
        .file("src/main.rs", r#"
            fn main() {
              println!("main");
            }
        "#);

    assert_that(p.cargo_process("build")
                 .arg("--all"),
                execs().with_status(0)
                );
    assert_that(process(&p.bin("foo")),
                execs().with_status(0).with_stdout("main\n"));
    assert_that(process(&p.bin("bar")),
                execs().with_status(0).with_stdout("main\n"));
}

#[test]
fn run_proper_binary_main_rs_as_foo() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            authors = []
            version = "0.0.0"
            [[bin]]
            name = "foo"
        "#)
        .file("src/foo.rs", r#"
            fn main() {
                panic!("This should never be run.");
            }
        "#)
        .file("src/main.rs", r#"
            fn main() {
            }
        "#);

    assert_that(p.cargo_process("run").arg("--bin").arg("foo"),
                execs().with_status(0));
}

#[test]
fn rustc_wrapper() {
    // We don't have /usr/bin/env on Windows.
    if cfg!(windows) { return }

    let p = project("foo")
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]));

    assert_that(p.cargo_process("build").arg("-v").env("RUSTC_WRAPPER", "/usr/bin/env"),
                execs().with_stderr_contains(
                    "[RUNNING] `/usr/bin/env rustc --crate-name foo [..]")
                .with_status(0));
}

#[test]
fn cdylib_not_lifted() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            authors = []
            version = "0.1.0"

            [lib]
            crate-type = ["cdylib"]
        "#)
        .file("src/lib.rs", "");

    assert_that(p.cargo_process("build"), execs().with_status(0));

    let files = if cfg!(windows) {
        vec!["foo.dll.lib", "foo.dll.exp", "foo.dll"]
    } else if cfg!(target_os = "macos") {
        vec!["libfoo.dylib"]
    } else {
        vec!["libfoo.so"]
    };

    for file in files {
        println!("checking: {}", file);
        assert_that(&p.root().join("target/debug/deps").join(&file),
                    existing_file());
    }
}

#[test]
fn deterministic_cfg_flags() {
    // This bug is non-deterministic

    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.1.0"
            authors = []
            build = "build.rs"

            [features]
            default = ["f_a", "f_b", "f_c", "f_d"]
            f_a = []
            f_b = []
            f_c = []
            f_d = []
        "#)
        .file("build.rs", r#"
                fn main() {
                    println!("cargo:rustc-cfg=cfg_a");
                    println!("cargo:rustc-cfg=cfg_b");
                    println!("cargo:rustc-cfg=cfg_c");
                    println!("cargo:rustc-cfg=cfg_d");
                    println!("cargo:rustc-cfg=cfg_e");
                }
            "#)
        .file("src/main.rs", r#"
            fn main() {}
        "#);

    assert_that(p.cargo_process("build").arg("-v"),
                execs().with_status(0)
                    .with_stderr("\
[COMPILING] foo v0.1.0 [..]
[RUNNING] [..]
[RUNNING] [..]
[RUNNING] `rustc --crate-name foo [..] \
--cfg[..]default[..]--cfg[..]f_a[..]--cfg[..]f_b[..]\
--cfg[..]f_c[..]--cfg[..]f_d[..] \
--cfg cfg_a --cfg cfg_b --cfg cfg_c --cfg cfg_d --cfg cfg_e`
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]"));
}

#[test]
fn explicit_bins_without_paths() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []

            [[bin]]
            name = "foo"

            [[bin]]
            name = "bar"
        "#)
        .file("src/lib.rs", "")
        .file("src/main.rs", "fn main() {}")
        .file("src/bin/bar.rs", "fn main() {}");

    assert_that(p.cargo_process("build"), execs().with_status(0));
}

#[test]
fn no_bin_in_src_with_lib() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []

            [[bin]]
            name = "foo"
        "#)
        .file("src/lib.rs", "")
        .file("src/foo.rs", "fn main() {}");

    assert_that(p.cargo_process("build"),
                execs().with_status(101)
                       .with_stderr_contains("\
[ERROR] failed to parse manifest at `[..]`

Caused by:
  can't find `foo` bin, specify bin.path"));
}


#[test]
fn dirs_in_bin_dir_with_main_rs() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []
        "#)
        .file("src/main.rs", "fn main() {}")
        .file("src/bin/bar.rs", "fn main() {}")
        .file("src/bin/bar2.rs", "fn main() {}")
        .file("src/bin/bar3/main.rs", "fn main() {}")
        .file("src/bin/bar4/main.rs", "fn main() {}");

    assert_that(p.cargo_process("build"), execs().with_status(0));
    assert_that(&p.bin("foo"), existing_file());
    assert_that(&p.bin("bar"), existing_file());
    assert_that(&p.bin("bar2"), existing_file());
    assert_that(&p.bin("bar3"), existing_file());
    assert_that(&p.bin("bar4"), existing_file());
}

#[test]
fn dir_and_file_with_same_name_in_bin() {
    // this should fail, because we have two binaries with the same name
    let p = project("bar")
        .file("Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.1.0"
            authors = []
        "#)
        .file("src/main.rs", "fn main() {}")
        .file("src/bin/foo.rs", "fn main() {}")
        .file("src/bin/foo/main.rs", "fn main() {}");

    assert_that(p.cargo_process("build"), 
                execs().with_status(101)
                       .with_stderr_contains("\
[..]found duplicate binary name foo, but all binary targets must have a unique name[..]
"));
}

#[test]
fn inferred_path_in_src_bin_foo() {
    let p = project("foo")
        .file("Cargo.toml", r#"
        [package]
        name = "foo"
        version = "0.1.0"
        authors = []

        [[bin]]
        name = "bar"
        # Note, no `path` key!
        "#)
        .file("src/bin/bar/main.rs", "fn main() {}");

    assert_that(p.cargo_process("build"), execs().with_status(0));
    assert_that(&p.bin("bar"), existing_file());
}
