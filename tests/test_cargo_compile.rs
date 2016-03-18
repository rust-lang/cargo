use std::env;
use std::fs::{self, File};
use std::io::prelude::*;
use tempdir::TempDir;

use support::{project, execs, main_file, basic_bin_manifest};
use support::{COMPILING, RUNNING, ProjectBuilder, ERROR};
use hamcrest::{assert_that, existing_file, is_not};
use support::paths::{CargoPathExt,root};
use cargo::util::process;

fn setup() {
}

test!(cargo_compile_simple {
    let p = project("foo")
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]));

    assert_that(p.cargo_process("build"), execs());
    assert_that(&p.bin("foo"), existing_file());

    assert_that(process(&p.bin("foo")),
                execs().with_stdout("i am foo\n"));
});

test!(cargo_compile_manifest_path {
    let p = project("foo")
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]));

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
        .with_stderr(&format!("\
{error} failed to parse manifest at `[..]`

Caused by:
  no `package` or `project` section found.
",
error = ERROR)))
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
        .with_stderr(&format!("\
{error} failed to parse manifest at `[..]`

Caused by:
  could not parse input as TOML
Cargo.toml:3:19-3:20 expected a value

",
error = ERROR)))
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
        .with_stderr(&format!("\
{error} failed to parse manifest at `[..]`

Caused by:
  could not parse input as TOML\n\
src[..]Cargo.toml:1:5-1:6 expected a value\n\n",
error = ERROR)))
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
                .with_stderr(&format!("\
{error} failed to parse manifest at `[..]`

Caused by:
  cannot parse '1.0' as a semver for the key `project.version`
",
error = ERROR)))

});

test!(cargo_compile_with_invalid_package_name {
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
                .with_stderr(&format!("\
{error} failed to parse manifest at `[..]`

Caused by:
  package name cannot be an empty string.
",
error = ERROR)))
});

test!(cargo_compile_with_invalid_bin_target_name {
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
                .with_stderr(&format!("\
{error} failed to parse manifest at `[..]`

Caused by:
  binary target names cannot be empty.
",
error = ERROR)))
});

test!(cargo_compile_with_forbidden_bin_target_name {
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
                .with_stderr(&format!("\
{error} failed to parse manifest at `[..]`

Caused by:
  the binary target name `build` is forbidden
",
error = ERROR)))
});

test!(cargo_compile_with_invalid_lib_target_name {
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
                .with_stderr(&format!("\
{error} failed to parse manifest at `[..]`

Caused by:
  library target names cannot be empty.
",
error = ERROR)))
});

test!(cargo_compile_without_manifest {
    let tmpdir = TempDir::new("cargo").unwrap();
    let p = ProjectBuilder::new("foo", tmpdir.path().to_path_buf());

    assert_that(p.cargo_process("build"),
                execs().with_status(101)
                       .with_stderr(&format!("\
{error} could not find `Cargo.toml` in `[..]` or any parent directory
",
error = ERROR)));
});

test!(cargo_compile_with_invalid_code {
    let p = project("foo")
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", "invalid rust code!");

    assert_that(p.cargo_process("build"),
        execs()
        .with_status(101)
        .with_stderr_contains("\
src[..]foo.rs:1:1: 1:8 error: expected item[..]found `invalid`
src[..]foo.rs:1 invalid rust code!
             ^~~~~~~
")
        .with_stderr_contains(format!("\
{error} Could not compile `foo`.

To learn more, run the command again with --verbose.\n", error = ERROR)));
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
        .file("Cargo.toml", &basic_bin_manifest("bar"))
        .file("src/lib.rs", "invalid rust code!");
    let baz = project("baz")
        .file("Cargo.toml", &basic_bin_manifest("baz"))
        .file("src/lib.rs", "invalid rust code!");
    bar.build();
    baz.build();
    assert_that(p.cargo_process("build"), execs().with_status(101));
});

test!(cargo_compile_with_warnings_in_the_root_package {
    let p = project("foo")
        .file("Cargo.toml", &basic_bin_manifest("foo"))
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
        execs()
        .with_stdout(&format!("{} bar v0.5.0 ({}/bar)\n\
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
      process(&p.bin("foo")),
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

    assert_that(
      process(&p.bin("foo")),
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

    assert_that(
      process(&p.bin("foo")),
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

    assert_that(
      process(&p.bin("foo")),
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

    assert_that(process(&p.bin("foo")),
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
        .file("src/foo.rs", &main_file(r#""i am foo""#, &["bar"]))
        .file("bar/Cargo.toml", &basic_bin_manifest("bar"))
        .file("bar/src/bar.rs", &main_file(r#""i am bar""#, &[]));

    assert_that(p.cargo_process("build"),
                execs().with_status(101).with_stderr(&format!(
r#"{error} no matching package named `notquitebar` found (required by `foo`)
location searched: {proj_dir}/bar
version required: *
"#, error = ERROR, proj_dir = p.url())));
});

test!(cargo_compile_with_filename{
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

    assert_that(p.cargo_process("build").arg("--bin").arg("bin.rs"),
                execs().with_status(101).with_stderr(&format!("\
{error} no bin target named `bin.rs`", error = ERROR)));

    assert_that(p.cargo_process("build").arg("--bin").arg("a.rs"),
                execs().with_status(101).with_stderr(&format!("\
{error} no bin target named `a.rs`

Did you mean `a`?", error = ERROR)));

    assert_that(p.cargo_process("build").arg("--example").arg("example.rs"),
                execs().with_status(101).with_stderr(&format!("\
{error} no example target named `example.rs`", error = ERROR)));

    assert_that(p.cargo_process("build").arg("--example").arg("a.rs"),
                execs().with_status(101).with_stderr(&format!("\
{error} no example target named `a.rs`

Did you mean `a`?", error = ERROR)));
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
                execs().with_status(101).with_stderr(&format!("\
{error} no matching package named `bar` found (required by `foo`)
location searched: [..]
version required: = 0.0.1
versions found: 0.0.2
consider running `cargo update` to update a path dependency's locked version
",
error = ERROR)));
});

test!(ignores_carriage_return_in_lockfile {
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
});

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

    println!("build");
    assert_that(p.cargo_process("build").arg("-v"), execs().with_status(0));

    println!("bin");
    assert_that(process(&p.bin("foo")),
                execs().with_stdout(&format!("0-5-1 @ alpha.1 in {}\n",
                                            p.root().display())));

    println!("test");
    assert_that(p.cargo("test").arg("-v"),
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
    assert_that(p.cargo_process("build"), execs().with_status(0));

    assert_that(&p.root().join("target/debug/libfoo.rlib"), existing_file());
    let fname = format!("{}foo{}", env::consts::DLL_PREFIX,
                        env::consts::DLL_SUFFIX);
    assert_that(&p.root().join("target/debug").join(&fname), existing_file());
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

    assert_that(&p.root().join("target/debug/libfoo.rlib"), existing_file());
    let fname = format!("{}foo{}", env::consts::DLL_PREFIX,
                        env::consts::DLL_SUFFIX);
    assert_that(&p.root().join("target/debug").join(&fname), existing_file());
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
                       .with_stderr("\
warning: unused manifest key: project.bulid
"));

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
                       .with_stderr("\
warning: unused manifest key: lib.build
"));
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
                       .with_stderr(&format!("\
{error} cyclic package dependency: package `test v0.0.0 ([..])` depends on itself
",
error = ERROR)));
});

test!(ignore_broken_symlinks {
    // windows and symlinks don't currently agree that well
    if cfg!(windows) { return }

    let p = project("foo")
        .file("Cargo.toml", &basic_bin_manifest("foo"))
        .file("src/foo.rs", &main_file(r#""i am foo""#, &[]))
        .symlink("Notafile", "bar");

    assert_that(p.cargo_process("build"), execs());
    assert_that(&p.bin("foo"), existing_file());

    assert_that(process(&p.bin("foo")),
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
                       .with_stderr(&format!("\
{error} failed to parse manifest at `[..]Cargo.toml`

Caused by:
  no targets specified in the manifest
  either src/lib.rs, src/main.rs, a [lib] section, or [[bin]] section must be present\n",
error = ERROR)));
});

test!(lto_build {
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
                execs().with_status(0).with_stdout(&format!("\
{compiling} test v0.0.0 ({url})
{running} `rustc src[..]main.rs --crate-name test --crate-type bin \
        -C opt-level=3 \
        -C lto \
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
                execs().with_status(0).with_stdout(&format!("\
{compiling} test v0.0.0 ({url})
{running} `rustc src[..]lib.rs --crate-name test --crate-type lib -g \
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
                execs().with_status(0).with_stdout(&format!("\
{compiling} test v0.0.0 ({url})
{running} `rustc src[..]lib.rs --crate-name test --crate-type lib \
        -C opt-level=3 \
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
                execs().with_status(0).with_stdout(&format!("\
{compiling} foo v0.0.0 ({url}/foo)
{running} `rustc foo[..]src[..]lib.rs --crate-name foo \
        --crate-type dylib --crate-type rlib -C prefer-dynamic \
        -C opt-level=3 \
        -C metadata=[..] \
        -C extra-filename=-[..] \
        --out-dir {dir}[..]target[..]release[..]deps \
        --emit=dep-info,link \
        -L dependency={dir}[..]target[..]release[..]deps \
        -L dependency={dir}[..]target[..]release[..]deps`
{compiling} test v0.0.0 ({url})
{running} `rustc src[..]lib.rs --crate-name test --crate-type lib \
        -C opt-level=3 \
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
                    suffix = env::consts::DLL_SUFFIX)));
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

    assert_that(p.cargo_process("test").arg("-v"), execs().with_status(0));
    assert_that(process(&p.bin("examples/hello")),
                        execs().with_stdout("Hello, World!\n"));
    assert_that(process(&p.bin("examples/goodbye")),
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
    assert_that(process(&p.bin("examples/hello")),
                execs().with_stdout("Hello, World!\n"));
    assert_that(process(&p.bin("examples/goodbye")),
                execs().with_stdout("Goodbye, World!\n"));
});

test!(standard_build_no_ndebug {
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
                execs().with_stdout("slow\n"));
});

test!(release_build_ndebug {
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
    assert_that(process(&p.bin("foo")), execs().with_status(0));
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
    assert_that(process(&p.bin("foo")), execs().with_status(0));
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
                       .with_stdout(&format!("\
{compiling} syntax v0.0.1 ({dir})
",
                       compiling = COMPILING,
                       dir = p.url())));
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
                       .with_stdout(&format!("\
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
    foo.root().move_into_the_past().unwrap();

    assert_that(foo.cargo("build").env("FIRST", "1"),
                execs().with_status(0)
                       .with_stdout(&format!("\
{compiling} foo v0.0.0 ({url})
", compiling = COMPILING, url = foo.url())));

    File::create(&foo.root().join("src/bar.rs")).unwrap();
    assert_that(foo.cargo("build"),
                execs().with_status(0)
                       .with_stdout(&format!("\
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
                execs().with_status(0));
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
                execs().with_status(101).with_stderr(&format!("\
{error} Couldn't load Cargo configuration

Caused by:
  could not parse TOML configuration in `[..]`

Caused by:
  could not parse input as TOML
[..].cargo[..]config:2:20-2:21 expected `=`, but found `i`

",
error = ERROR)));
});

test!(cargo_platform_specific_dependency {
    let host = ::rustc_host();
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
});

test!(bad_platform_specific_dependency {
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
    assert_that(process(&p.bin("foo")),
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

    p.cargo_process("test").arg("--no-run").arg("-v")
        .exec_with_output()
        .unwrap();

    assert_that(&p.bin("foo"), is_not(existing_file()));
    assert_that(&p.bin("examples/foo"), existing_file());

    p.cargo("test").arg("--no-run").arg("-v")
                   .exec_with_output()
                   .unwrap();

    assert_that(&p.bin("foo"), is_not(existing_file()));
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
        ::sleep_ms(100);
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
                       .with_stderr(format!("\
[..] can't find crate for `bbbbb`[..]
[..] extern crate bbbbb; [..]
[..]
error: aborting due to previous error
{error} Could not compile `foo`.

Caused by:
  [..]
",
error = ERROR)));
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
                       .with_stderr(&format!("\
{error} cyclic package dependency: package `foo v0.0.1 ([..])` depends on itself
",
error = ERROR)));
});

test!(predictable_filenames {
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
});

test!(dashes_to_underscores {
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
});

test!(dashes_in_crate_name_bad {
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
});

test!(rustc_env_var {
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
                       .with_stderr(&format!("\
{error} Could not execute process `rustc-that-does-not-exist -vV` ([..])

Caused by:
[..]
",
error = ERROR)));
    assert_that(&p.bin("a"), is_not(existing_file()));
});

test!(filtering {
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
});

test!(ignore_dotfile {
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
});

test!(ignore_dotdirs {
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
});

test!(dotdir_root {
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
});


test!(custom_target_dir {
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
});

test!(rustc_no_trans {
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
});

test!(build_multiple_packages {
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
    p.build();

    assert_that(p.cargo_process("build").arg("-p").arg("d1").arg("-p").arg("d2")
                                        .arg("-p").arg("foo"),
                execs());

    assert_that(&p.bin("foo"), existing_file());
    assert_that(process(&p.bin("foo")),
                execs().with_stdout("i am foo\n"));

    let d1_path = &p.build_dir().join("debug").join("deps")
                                .join(format!("d1{}", env::consts::EXE_SUFFIX));
    let d2_path = &p.build_dir().join("debug").join("deps")
                                .join(format!("d2{}", env::consts::EXE_SUFFIX));

    assert_that(d1_path, existing_file());
    assert_that(process(d1_path), execs().with_stdout("d1"));

    assert_that(d2_path, existing_file());
    assert_that(process(d2_path),
                execs().with_stdout("d2"));
});

test!(invalid_spec {
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
        .file("d1/src/main.rs", "fn main() { println!(\"d1\"); }");
    p.build();

    assert_that(p.cargo_process("build").arg("-p").arg("notAValidDep"),
                execs().with_status(101).with_stderr(&format!(
                    "{error} could not find package matching spec `notAValidDep`", error = ERROR)));

    assert_that(p.cargo_process("build").arg("-p").arg("d1").arg("-p").arg("notAValidDep"),
                execs().with_status(101).with_stderr(&format!(
                    "{error} could not find package matching spec `notAValidDep`", error = ERROR)));

});

test!(manifest_with_bom_is_ok {
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
});
