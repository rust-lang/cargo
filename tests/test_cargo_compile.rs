use std::env;
use std::fs::{self, File};
use std::io::prelude::*;
use tempdir::TempDir;

use support::{project, execs, main_file, basic_bin_manifest};
use support::{COMPILING, RUNNING, ProjectBuilder};
use hamcrest::{assert_that, existing_file};
use support::paths::CargoPathExt;
use cargo::util::process;

fn setup() {
}

test!(cargo_compile_simple {
    let p = project("foo")
        .file("Cargo.toml", &basic_bin_manifest("foo").as_slice())
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]).as_slice());

    assert_that(p.cargo_process("build"), execs());
    assert_that(&p.bin("foo"), existing_file());

    assert_that(process(&p.bin("foo")).unwrap(),
                execs().with_stdout("i am foo\n"));
});

test!(cargo_compile_manifest_path {
    let p = project("foo")
        .file("Cargo.toml", &basic_bin_manifest("foo").as_slice())
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]).as_slice());

    assert_that(p.cargo_process("build")
                 .arg("--manifest-path").arg("foo/Cargo.toml")
                 .cwd(p.root().parent().unwrap()),
                execs().with_status(0));
    assert_that(&p.bin("foo"), existing_file());
});

test!(cargo_compile_with_invalid_manifest {
    let p = project("foo")
        .file("Cargo.toml", "");

    assert_that(p.cargo_process("build"),
        execs()
        .with_status(101)
        .with_stderr("\
failed to parse manifest at `[..]`

Caused by:
  No `package` or `project` section found.
"))
});

test!(cargo_compile_with_invalid_manifest2 {
    let p = project("foo")
        .file("Cargo.toml", r"
            [project]
            foo = bar
        ");

    assert_that(p.cargo_process("build"),
        execs()
        .with_status(101)
        .with_stderr("\
failed to parse manifest at `[..]`

Caused by:
  could not parse input as TOML
Cargo.toml:3:19-3:20 expected a value

"))
});

test!(cargo_compile_with_invalid_manifest3 {
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
failed to parse manifest at `[..]`

Caused by:
  could not parse input as TOML\n\
src[..]Cargo.toml:1:5-1:6 expected a value\n\n"))
});

test!(cargo_compile_with_invalid_version {
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
failed to parse manifest at `[..]`

Caused by:
  cannot parse '1.0' as a semver for the key `project.version`
"))

});

test!(cargo_compile_without_manifest {
    let tmpdir = TempDir::new("cargo").unwrap();
    let p = ProjectBuilder::new("foo", tmpdir.path().to_path_buf());

    assert_that(p.cargo_process("build"),
                execs().with_status(101)
                       .with_stderr("\
Could not find `Cargo.toml` in `[..]` or any parent directory
"));
});

test!(cargo_compile_with_invalid_code {
    let p = project("foo")
        .file("Cargo.toml", basic_bin_manifest("foo").as_slice())
        .file("src/foo.rs", "invalid rust code!");

    assert_that(p.cargo_process("build"),
        execs()
        .with_status(101)
        .with_stderr("\
src[..]foo.rs:1:1: 1:8 error: expected item[..]found `invalid`
src[..]foo.rs:1 invalid rust code!
             ^~~~~~~
Could not compile `foo`.

To learn more, run the command again with --verbose.\n"));
    assert_that(&p.root().join("Cargo.lock"), existing_file());
});

test!(cargo_compile_with_invalid_code_in_deps {
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
        .file("Cargo.toml", basic_bin_manifest("bar").as_slice())
        .file("src/lib.rs", "invalid rust code!");
    let baz = project("baz")
        .file("Cargo.toml", basic_bin_manifest("baz").as_slice())
        .file("src/lib.rs", "invalid rust code!");
    bar.build();
    baz.build();
    assert_that(p.cargo_process("build"), execs().with_status(101));
});

test!(cargo_compile_with_warnings_in_the_root_package {
    let p = project("foo")
        .file("Cargo.toml", basic_bin_manifest("foo").as_slice())
        .file("src/foo.rs", "fn main() {} fn dead() {}");

    assert_that(p.cargo_process("build"),
        execs()
        .with_stderr("\
src[..]foo.rs:1:14: 1:26 warning: function is never used: `dead`, \
    #[warn(dead_code)] on by default
src[..]foo.rs:1 fn main() {} fn dead() {}
[..]                         ^~~~~~~~~~~~
"));
});

test!(cargo_compile_with_warnings_in_a_dep_package {
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
              main_file(r#""{}", bar::gimme()"#, &["bar"]).as_slice())
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
        execs()
        .with_stdout(format!("{} bar v0.5.0 ({})\n\
                              {} foo v0.5.0 ({})\n",
                             COMPILING, p.url(),
                             COMPILING, p.url()))
        .with_stderr("\
[..]warning: function is never used: `dead`[..]
[..]fn dead() {}
[..]^~~~~~~~~~~~
"));

    assert_that(&p.bin("foo"), existing_file());

    assert_that(
      process(&p.bin("foo")).unwrap(),
      execs().with_stdout("test passed\n"));
});

test!(cargo_compile_with_nested_deps_inferred {
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
              main_file(r#""{}", bar::gimme()"#, &["bar"]).as_slice())
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

    assert_that(
      process(&p.bin("foo")).unwrap(),
      execs().with_stdout("test passed\n"));
});

test!(cargo_compile_with_nested_deps_correct_bin {
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
              main_file(r#""{}", bar::gimme()"#, &["bar"]).as_slice())
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

    assert_that(
      process(&p.bin("foo")).unwrap(),
      execs().with_stdout("test passed\n"));
});

test!(cargo_compile_with_nested_deps_shorthand {
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
        .file("src/foo.rs",
              main_file(r#""{}", bar::gimme()"#, &["bar"]).as_slice())
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

    assert_that(
      process(&p.bin("foo")).unwrap(),
      execs().with_stdout("test passed\n"));
});

test!(cargo_compile_with_nested_deps_longhand {
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
              main_file(r#""{}", bar::gimme()"#, &["bar"]).as_slice())
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

    assert_that(process(&p.bin("foo")).unwrap(),
                execs().with_stdout("test passed\n"));
});

// Check that Cargo gives a sensible error if a dependency can't be found
// because of a name mismatch.
test!(cargo_compile_with_dep_name_mismatch {
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
        .file("src/foo.rs", main_file(r#""i am foo""#, &["bar"]).as_slice())
        .file("bar/Cargo.toml", basic_bin_manifest("bar").as_slice())
        .file("bar/src/bar.rs", main_file(r#""i am bar""#, &[]).as_slice());

    assert_that(p.cargo_process("build"),
                execs().with_status(101).with_stderr(format!(
r#"no matching package named `notquitebar` found (required by `foo`)
location searched: {proj_dir}
version required: *
"#, proj_dir = p.url())));
});

test!(compile_path_dep_then_change_version {
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
no matching package named `bar` found (required by `foo`)
location searched: [..]
version required: = 0.0.1
versions found: 0.0.2
consider running `cargo update` to update a path dependency's locked version
"));
});

// test!(compiling_project_with_invalid_manifest)

test!(crate_version_env_vars {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.5.1-alpha.1"
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

            fn main() {
                let s = format!("{}-{}-{} @ {} in {}", VERSION_MAJOR,
                                VERSION_MINOR, VERSION_PATCH, VERSION_PRE,
                                CARGO_MANIFEST_DIR);
                assert_eq!(s, foo::version());
                println!("{}", s);
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

    assert_that(p.cargo_process("build"), execs().with_status(0));

    assert_that(process(&p.bin("foo")).unwrap(),
                execs().with_stdout(format!("0-5-1 @ alpha.1 in {}\n",
                                            p.root().display()).as_slice()));

    assert_that(p.cargo("test"),
                execs().with_status(0));
});

// this is testing that src/<pkg-name>.rs still works (for now)
test!(many_crate_types_old_style_lib_location {
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
    assert_that(p.cargo_process("build"),
                execs().with_status(0));

    let files = fs::read_dir(&p.root().join("target/debug")).unwrap();
    let mut files: Vec<String> = files.map(|e| e.unwrap().path()).filter_map(|f| {
        match f.file_name().unwrap().to_str().unwrap() {
            "build" | "examples" | "deps" => None,
            s if s.contains("fingerprint") || s.contains("dSYM") => None,
            s => Some(s.to_string())
        }
    }).collect();
    files.sort();
    let file0 = files[0].as_slice();
    let file1 = files[1].as_slice();
    println!("{} {}", file0, file1);
    assert!(file0.ends_with(".rlib") || file1.ends_with(".rlib"));
    assert!(file0.ends_with(env::consts::DLL_SUFFIX) ||
            file1.ends_with(env::consts::DLL_SUFFIX));
});

test!(many_crate_types_correct {
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

    let files = fs::read_dir(&p.root().join("target/debug")).unwrap();
    let mut files: Vec<String> = files.map(|f| f.unwrap().path()).filter_map(|f| {
        match f.file_name().unwrap().to_str().unwrap() {
            "build" | "examples" | "deps" => None,
            s if s.contains("fingerprint") || s.contains("dSYM") => None,
            s => Some(s.to_string())
        }
    }).collect();
    files.sort();
    let file0 = files[0].as_slice();
    let file1 = files[1].as_slice();
    println!("{} {}", file0, file1);
    assert!(file0.ends_with(".rlib") || file1.ends_with(".rlib"));
    assert!(file0.ends_with(env::consts::DLL_SUFFIX) ||
            file1.ends_with(env::consts::DLL_SUFFIX));
});

test!(unused_keys {
    let mut p = project("foo");
    p = p
        .file("Cargo.toml", r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]
            bulid = "foo"

            [lib]

            name = "foo"
        "#)
        .file("src/foo.rs", r#"
            pub fn foo() {}
        "#);
    assert_that(p.cargo_process("build"),
                execs().with_status(0)
                       .with_stderr("unused manifest key: project.bulid\n"));

    let mut p = project("bar");
    p = p
        .file("Cargo.toml", r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [lib]

            name = "foo"
            build = "foo"
        "#)
        .file("src/foo.rs", r#"
            pub fn foo() {}
        "#);
    assert_that(p.cargo_process("build"),
                execs().with_status(0)
                       .with_stderr("unused manifest key: lib.build\n"));
});

test!(self_dependency {
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
        "#)
        .file("src/test.rs", "fn main() {}");
    assert_that(p.cargo_process("build"),
                execs().with_status(101)
                       .with_stderr("\
cyclic package dependency: package `test v0.0.0 ([..])` depends on itself
"));
});

test!(ignore_broken_symlinks {
    // windows and symlinks don't currently agree that well
    if cfg!(windows) { return }

    let p = project("foo")
        .file("Cargo.toml", basic_bin_manifest("foo").as_slice())
        .file("src/foo.rs", main_file(r#""i am foo""#, &[]).as_slice())
        .symlink("Notafile", "bar");

    assert_that(p.cargo_process("build"), execs());
    assert_that(&p.bin("foo"), existing_file());

    assert_that(process(&p.bin("foo")).unwrap(),
                execs().with_stdout("i am foo\n"));
});

test!(missing_lib_and_bin {
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
failed to parse manifest at `[..]Cargo.toml`

Caused by:
  either a [lib] or [[bin]] section must be present\n"));
});

test!(lto_build {
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
                execs().with_status(0).with_stdout(format!("\
{compiling} test v0.0.0 ({url})
{running} `rustc src[..]main.rs --crate-name test --crate-type bin \
        -C opt-level=3 \
        -C lto \
        --cfg ndebug \
        --out-dir {dir}[..]target[..]release \
        --emit=dep-info,link \
        -L dependency={dir}[..]target[..]release \
        -L dependency={dir}[..]target[..]release[..]deps`
",
running = RUNNING, compiling = COMPILING,
dir = p.root().display(),
url = p.url(),
)));
});

test!(verbose_build {
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
                execs().with_status(0).with_stdout(format!("\
{compiling} test v0.0.0 ({url})
{running} `rustc src[..]lib.rs --crate-name test --crate-type lib -g \
        -C metadata=[..] \
        -C extra-filename=-[..] \
        --out-dir {dir}[..]target[..]debug \
        --emit=dep-info,link \
        -L dependency={dir}[..]target[..]debug \
        -L dependency={dir}[..]target[..]debug[..]deps`
",
running = RUNNING, compiling = COMPILING,
dir = p.root().display(),
url = p.url(),
)));
});

test!(verbose_release_build {
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
                execs().with_status(0).with_stdout(format!("\
{compiling} test v0.0.0 ({url})
{running} `rustc src[..]lib.rs --crate-name test --crate-type lib \
        -C opt-level=3 \
        --cfg ndebug \
        -C metadata=[..] \
        -C extra-filename=-[..] \
        --out-dir {dir}[..]target[..]release \
        --emit=dep-info,link \
        -L dependency={dir}[..]target[..]release \
        -L dependency={dir}[..]target[..]release[..]deps`
",
running = RUNNING, compiling = COMPILING,
dir = p.root().display(),
url = p.url(),
)));
});

test!(verbose_release_build_deps {
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
                execs().with_status(0).with_stdout(format!("\
{compiling} foo v0.0.0 ({url})
{running} `rustc foo[..]src[..]lib.rs --crate-name foo \
        --crate-type dylib --crate-type rlib -C prefer-dynamic \
        -C opt-level=3 \
        --cfg ndebug \
        -C metadata=[..] \
        -C extra-filename=-[..] \
        --out-dir {dir}[..]target[..]release[..]deps \
        --emit=dep-info,link \
        -L dependency={dir}[..]target[..]release[..]deps \
        -L dependency={dir}[..]target[..]release[..]deps`
{compiling} test v0.0.0 ({url})
{running} `rustc src[..]lib.rs --crate-name test --crate-type lib \
        -C opt-level=3 \
        --cfg ndebug \
        -C metadata=[..] \
        -C extra-filename=-[..] \
        --out-dir {dir}[..]target[..]release \
        --emit=dep-info,link \
        -L dependency={dir}[..]target[..]release \
        -L dependency={dir}[..]target[..]release[..]deps \
        --extern foo={dir}[..]target[..]release[..]deps[..]\
                     {prefix}foo-[..]{suffix} \
        --extern foo={dir}[..]target[..]release[..]deps[..]libfoo-[..].rlib`
",
                    running = RUNNING,
                    compiling = COMPILING,
                    dir = p.root().display(),
                    url = p.url(),
                    prefix = env::consts::DLL_PREFIX,
                    suffix = env::consts::DLL_SUFFIX).as_slice()));
});

test!(explicit_examples {
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

    assert_that(p.cargo_process("test"), execs().with_status(0));
    assert_that(process(&p.bin("examples/hello")).unwrap(),
                        execs().with_stdout("Hello, World!\n"));
    assert_that(process(&p.bin("examples/goodbye")).unwrap(),
                        execs().with_stdout("Goodbye, World!\n"));
});

test!(implicit_examples {
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
    assert_that(process(&p.bin("examples/hello")).unwrap(),
                execs().with_stdout("Hello, World!\n"));
    assert_that(process(&p.bin("examples/goodbye")).unwrap(),
                execs().with_stdout("Goodbye, World!\n"));
});

test!(standard_build_no_ndebug {
    let p = project("world")
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", r#"
            fn main() {
                if cfg!(ndebug) {
                    println!("fast")
                } else {
                    println!("slow")
                }
            }
        "#);

    assert_that(p.cargo_process("build"), execs().with_status(0));
    assert_that(process(&p.bin("foo")).unwrap(),
                execs().with_stdout("slow\n"));
});

test!(release_build_ndebug {
    let p = project("world")
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", r#"
            fn main() {
                if cfg!(ndebug) {
                    println!("fast")
                } else {
                    println!("slow")
                }
            }
        "#);

    assert_that(p.cargo_process("build").arg("--release"),
                execs().with_status(0));
    assert_that(process(&p.release_bin("foo")).unwrap(),
                execs().with_stdout("fast\n"));
});

test!(inferred_main_bin {
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
    assert_that(process(&p.bin("foo")).unwrap(), execs().with_status(0));
});

test!(deletion_causes_failure {
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

    assert_that(p.cargo_process("build"), execs().with_status(0));
    let p = p.file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#);
    assert_that(p.cargo_process("build"), execs().with_status(101));
});

test!(bad_cargo_toml_in_target_dir {
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
    assert_that(process(&p.bin("foo")).unwrap(), execs().with_status(0));
});

test!(lib_with_standard_name {
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
                       .with_stdout(format!("\
{compiling} syntax v0.0.1 ({dir})
",
                       compiling = COMPILING,
                       dir = p.url()).as_slice()));
});

test!(simple_staticlib {
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
});

test!(staticlib_rlib_and_bin {
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
});

test!(opt_out_of_lib {
    let p = project("foo")
        .file("Cargo.toml", r#"
              lib = []

              [package]
              name = "foo"
              authors = []
              version = "0.0.1"
        "#)
        .file("src/lib.rs", "bad syntax")
        .file("src/main.rs", "fn main() {}");
    assert_that(p.cargo_process("build"), execs().with_status(0));
});

test!(opt_out_of_bin {
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
});

test!(single_lib {
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
});

test!(deprecated_lib {
    let p = project("foo")
        .file("Cargo.toml", r#"
              [package]
              name = "foo"
              authors = []
              version = "0.0.1"

              [[lib]]
              name = "foo"
        "#)
        .file("src/foo.rs", "");
    assert_that(p.cargo_process("build"),
                execs().with_status(0)
                       .with_stderr("\
the [[lib]] section has been deprecated in favor of [lib]\n"));
});

test!(freshness_ignores_excluded {
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
    foo.root().move_into_the_past().unwrap();

    assert_that(foo.cargo("build"),
                execs().with_status(0)
                       .with_stdout(format!("\
{compiling} foo v0.0.0 ({url})
", compiling = COMPILING, url = foo.url())));

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
});

test!(rebuild_preserves_out_dir {
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
            use std::old_io::File;
            use std::old_path::{Path, GenericPath};

            fn main() {
                let path = Path::new(env::var("OUT_DIR").unwrap()).join("foo");
                if env::var_os("FIRST").is_some() {
                    File::create(&path).unwrap();
                } else {
                    File::create(&path).unwrap();
                }
            }
        "#)
        .file("src/lib.rs", "pub fn bar() -> i32 { 1 }");
    foo.build();
    foo.root().move_into_the_past().unwrap();

    assert_that(foo.cargo("build").env("FIRST", "1"),
                execs().with_status(0)
                       .with_stdout(format!("\
{compiling} foo v0.0.0 ({url})
", compiling = COMPILING, url = foo.url())));

    File::create(&foo.root().join("src/bar.rs")).unwrap();
    assert_that(foo.cargo("build"),
                execs().with_status(0)
                       .with_stdout(format!("\
{compiling} foo v0.0.0 ({url})
", compiling = COMPILING, url = foo.url())));
});

test!(dep_no_libs {
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
                execs().with_status(101)
                       .with_stderr("\
Package `bar v0.0.0 ([..])` has no library targets"));
});

test!(recompile_space_in_name {
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
    foo.root().move_into_the_past().unwrap();
    assert_that(foo.cargo("build"),
                execs().with_status(0).with_stdout(""));
});

#[cfg(unix)]
test!(ignore_bad_directories {
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
});

test!(bad_cargo_config {
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
Couldn't load Cargo configuration

Caused by:
  could not parse TOML configuration in `[..]`

Caused by:
  could not parse input as TOML
[..].cargo[..]config:2:20-2:21 expected `=`, but found `i`

"));
});

#[cfg(all(any(target_arch = "x86", target_arch = "x86_64"), target_os = "linux"))]
test!(cargo_platform_specific_dependency {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [target.i686-unknown-linux-gnu.dependencies.bar]
            path = "bar"
            [target.x86_64-unknown-linux-gnu.dependencies.bar]
            path = "bar"
        "#)
        .file("src/main.rs",
              main_file(r#""{}", bar::gimme()"#, &["bar"]).as_slice())
        .file("bar/Cargo.toml", r#"
            [project]

            name = "bar"
            version = "0.5.0"
            authors = ["wycats@example.com"]
        "#)
        .file("bar/src/lib.rs", r#"
            pub fn gimme() -> String {
                "test passed".to_string()
            }
        "#);

    p.cargo_process("build")
        .exec_with_output()
        .unwrap();

    assert_that(&p.bin("foo"), existing_file());

    assert_that(process(&p.bin("foo")).unwrap(),
                execs().with_stdout("test passed\n"));
});

#[cfg(not(all(any(target_arch = "x86", target_arch = "x86_64"), target_os = "linux")))]
test!(cargo_platform_specific_dependency {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [target.i686-unknown-linux-gnu.dependencies.bar]
            path = "bar"
            [target.x86_64-unknown-linux-gnu.dependencies.bar]
            path = "bar"
        "#)
        .file("src/main.rs",
              main_file(r#""{}", bar::gimme()"#, &["bar"]).as_slice())
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
});

test!(cargo_platform_specific_dependency_wrong_platform {
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
    assert_that(process(&p.bin("foo")).unwrap(),
                execs());

    let loc = p.root().join("Cargo.lock");
    let mut lockfile = String::new();
    File::open(&loc).unwrap().read_to_string(&mut lockfile).unwrap();
    assert!(lockfile.contains("bar"))
});

test!(example_bin_same_name {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/main.rs", "fn main() {}")
        .file("examples/foo.rs", "fn main() {}");

    p.cargo_process("test").arg("--no-run")
        .exec_with_output()
        .unwrap();

    assert_that(&p.bin("foo"), existing_file());
    assert_that(&p.bin("examples/foo"), existing_file());

    p.cargo("test").arg("--no-run")
        .exec_with_output()
        .unwrap();

    assert_that(&p.bin("foo"), existing_file());
    assert_that(&p.bin("examples/foo"), existing_file());
});

test!(compile_then_delete {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/main.rs", "fn main() {}");

    assert_that(p.cargo_process("run"), execs().with_status(0));
    assert_that(&p.bin("foo"), existing_file());
    if cfg!(windows) {
        // On windows unlinking immediately after running often fails, so sleep
        use std::time::duration::Duration;
        ::sleep(Duration::milliseconds(100));
    }
    fs::remove_file(&p.bin("foo")).unwrap();
    assert_that(p.cargo("run"),
                execs().with_status(0));
});

test!(transitive_dependencies_not_available {
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
                       .with_stderr("\
[..] can't find crate for `bbbbb`
[..] extern crate bbbbb; [..]
[..]
error: aborting due to previous error
Could not compile `foo`.

Caused by:
  [..]
"));
});

test!(cyclic_deps_rejected {
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
cyclic package dependency: package `foo v0.0.1 ([..])` depends on itself
"));
});
