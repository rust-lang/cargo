use std::io::{fs, TempDir, File};
use std::os;
use std::path;

use support::{ResultTest, project, execs, main_file, basic_bin_manifest};
use support::{COMPILING, RUNNING, cargo_dir, ProjectBuilder, path2url};
use hamcrest::{assert_that, existing_file};
use support::paths::PathExt;
use cargo;
use cargo::util::{process, realpath};

fn setup() {
}

test!(cargo_compile_simple {
    let p = project("foo")
        .file("Cargo.toml", basic_bin_manifest("foo").as_slice())
        .file("src/foo.rs", main_file(r#""i am foo""#, []).as_slice());

    assert_that(p.cargo_process("build"), execs());
    assert_that(&p.bin("foo"), existing_file());

    assert_that(
      process(p.bin("foo")),
      execs().with_stdout("i am foo\n"));
})

test!(cargo_compile_manifest_path {
    let p = project("foo")
        .file("Cargo.toml", basic_bin_manifest("foo").as_slice())
        .file("src/foo.rs", main_file(r#""i am foo""#, []).as_slice());

    assert_that(p.cargo_process("build")
                 .arg("--manifest-path").arg("foo/Cargo.toml")
                 .cwd(p.root().dir_path()),
                execs().with_status(0));
    assert_that(&p.bin("foo"), existing_file());
})

test!(cargo_compile_with_invalid_manifest {
    let p = project("foo")
        .file("Cargo.toml", "");

    assert_that(p.cargo_process("build"),
        execs()
        .with_status(101)
        .with_stderr("Cargo.toml is not a valid manifest\n\n\
                      No `package` or `project` section found.\n"))
})

test!(cargo_compile_with_invalid_manifest2 {
    let p = project("foo")
        .file("Cargo.toml", r"
            [project]
            foo = bar
        ");

    assert_that(p.cargo_process("build"),
        execs()
        .with_status(101)
        .with_stderr("could not parse input TOML\n\
                      Cargo.toml:3:19-3:20 expected a value\n\n"))
})

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
        .with_stderr("could not parse input TOML\n\
                      src[..]Cargo.toml:1:5-1:6 expected a value\n\n"))
})

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
                .with_stderr("Cargo.toml is not a valid manifest\n\n\
                              cannot parse '1.0' as a semver for the key \
                              `project.version`\n"))

})

test!(cargo_compile_without_manifest {
    let tmpdir = TempDir::new("cargo").unwrap();
    let p = ProjectBuilder::new("foo", tmpdir.path().clone());

    assert_that(p.cargo_process("build"),
        execs()
        .with_status(102)
        .with_stderr("Could not find Cargo.toml in this directory or any \
                      parent directory\n"));
})

test!(cargo_compile_with_invalid_code {
    let p = project("foo")
        .file("Cargo.toml", basic_bin_manifest("foo").as_slice())
        .file("src/foo.rs", "invalid rust code!");

    assert_that(p.cargo_process("build"),
        execs()
        .with_status(101)
        .with_stderr(format!("\
{filename}:1:1: 1:8 error: expected item[..]found `invalid`
{filename}:1 invalid rust code!
             ^~~~~~~
Could not compile `foo`.

To learn more, run the command again with --verbose.\n",
            filename = format!("src{}foo.rs", path::SEP)).as_slice()));
    assert_that(&p.root().join("Cargo.lock"), existing_file());
})

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
})

test!(cargo_compile_with_warnings_in_the_root_package {
    let p = project("foo")
        .file("Cargo.toml", basic_bin_manifest("foo").as_slice())
        .file("src/foo.rs", "fn main() {} fn dead() {}");

    assert_that(p.cargo_process("build"),
        execs()
        .with_stderr(format!("\
{filename}:1:14: 1:26 warning: code is never used: `dead`, #[warn(dead_code)] \
on by default
{filename}:1 fn main() {{}} fn dead() {{}}
                          ^~~~~~~~~~~~
", filename = format!("src{}foo.rs", path::SEP).as_slice())));
})

test!(cargo_compile_with_warnings_in_a_dep_package {
    let mut p = project("foo");
    let bar = p.root().join("bar");

    p = p
        .file(".cargo/config", format!(r#"
            paths = ['{}']
        "#, bar.display()).as_slice())
        .file("Cargo.toml", r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dependencies]

            bar = "0.5.0"

            [[bin]]

            name = "foo"
        "#)
        .file("src/foo.rs",
              main_file(r#""{}", bar::gimme()"#, ["bar"]).as_slice())
        .file("bar/Cargo.toml", r#"
            [project]

            name = "bar"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [lib]

            name = "bar"
        "#)
        .file("bar/src/bar.rs", r#"
            pub fn gimme() -> String {
                "test passed".to_string()
            }

            fn dead() {}
        "#);

    let bar = realpath(&p.root().join("bar")).assert();
    let main = realpath(&p.root()).assert();

    assert_that(p.cargo_process("build"),
        execs()
        .with_stdout(format!("{} bar v0.5.0 ({})\n\
                              {} foo v0.5.0 ({})\n",
                             COMPILING, path2url(bar),
                             COMPILING, path2url(main)))
        .with_stderr(""));

    assert_that(&p.bin("foo"), existing_file());

    assert_that(
      cargo::util::process(p.bin("foo")),
      execs().with_stdout("test passed\n"));
})

test!(cargo_compile_with_nested_deps_inferred {
    let mut p = project("foo");
    let bar = p.root().join("bar");
    let baz = p.root().join("baz");

    p = p
        .file(".cargo/config", format!(r#"
            paths = ['{}', '{}']
        "#, bar.display(), baz.display()).as_slice())
        .file("Cargo.toml", r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dependencies]

            bar = "0.5.0"

            [[bin]]

            name = "foo"
        "#)
        .file("src/foo.rs",
              main_file(r#""{}", bar::gimme()"#, ["bar"]).as_slice())
        .file("bar/Cargo.toml", r#"
            [project]

            name = "bar"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dependencies]

            baz = "0.5.0"
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
        .assert();

    assert_that(&p.bin("foo"), existing_file());

    assert_that(
      cargo::util::process(p.bin("foo")),
      execs().with_stdout("test passed\n"));
})

test!(cargo_compile_with_nested_deps_correct_bin {
    let mut p = project("foo");
    let bar = p.root().join("bar");
    let baz = p.root().join("baz");

    p = p
        .file(".cargo/config", format!(r#"
            paths = ['{}', '{}']
        "#, bar.display(), baz.display()).as_slice())
        .file("Cargo.toml", r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dependencies]

            bar = "0.5.0"

            [[bin]]

            name = "foo"
        "#)
        .file("src/main.rs",
              main_file(r#""{}", bar::gimme()"#, ["bar"]).as_slice())
        .file("bar/Cargo.toml", r#"
            [project]

            name = "bar"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dependencies]

            baz = "0.5.0"
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
        .assert();

    assert_that(&p.bin("foo"), existing_file());

    assert_that(
      cargo::util::process(p.bin("foo")),
      execs().with_stdout("test passed\n"));
})

test!(cargo_compile_with_nested_deps_shorthand {
    let mut p = project("foo");
    let bar = p.root().join("bar");
    let baz = p.root().join("baz");

    p = p
        .file(".cargo/config", format!(r#"
            paths = ['{}', '{}']
        "#, bar.display(), baz.display()).as_slice())
        .file("Cargo.toml", r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dependencies]

            bar = "0.5.0"

            [[bin]]

            name = "foo"
        "#)
        .file("src/foo.rs",
              main_file(r#""{}", bar::gimme()"#, ["bar"]).as_slice())
        .file("bar/Cargo.toml", r#"
            [project]

            name = "bar"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dependencies]

            baz = "0.5.0"

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
        .assert();

    assert_that(&p.bin("foo"), existing_file());

    assert_that(
      cargo::util::process(p.bin("foo")),
      execs().with_stdout("test passed\n"));
})

test!(cargo_compile_with_nested_deps_longhand {
    let mut p = project("foo");
    let bar = p.root().join("bar");
    let baz = p.root().join("baz");

    p = p
        .file(".cargo/config", format!(r#"
            paths = ['{}', '{}']
        "#, bar.display(), baz.display()).as_slice())
        .file("Cargo.toml", r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dependencies]

            bar = "0.5.0"

            [[bin]]

            name = "foo"
        "#)
        .file("src/foo.rs",
              main_file(r#""{}", bar::gimme()"#, ["bar"]).as_slice())
        .file("bar/Cargo.toml", r#"
            [project]

            name = "bar"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [dependencies.baz]

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

    assert_that(
      cargo::util::process(p.bin("foo")),
      execs().with_stdout("test passed\n"));
})

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
        .file("src/foo.rs", main_file(r#""i am foo""#, ["bar"]).as_slice())
        .file("bar/Cargo.toml", basic_bin_manifest("bar").as_slice())
        .file("bar/src/bar.rs", main_file(r#""i am bar""#, []).as_slice());

    assert_that(p.cargo_process("build"),
                execs().with_status(101).with_stderr(format!(
r#"No package named `notquitebar` found (required by `foo`).
Location searched: {proj_dir}
Version required: *
"#, proj_dir = p.url())));
})

// test!(compiling_project_with_invalid_manifest)

test!(custom_build {
    let mut build = project("builder");
    build = build
        .file("Cargo.toml", r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [[bin]] name = "foo"
        "#)
        .file("src/foo.rs", r#"
            fn main() { println!("Hello!"); }
        "#);
    assert_that(build.cargo_process("build"),
                execs().with_status(0));


    let mut p = project("foo");
    p = p
        .file("Cargo.toml", format!(r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]
            build = '{}'

            [[bin]] name = "foo"
        "#, build.bin("foo").display()))
        .file("src/foo.rs", r#"
            fn main() {}
        "#);
    assert_that(p.cargo_process("build"),
                execs().with_status(0)
                       .with_stdout(format!("   Compiling foo v0.5.0 ({})\n",
                                            p.url()))
                       .with_stderr(""));
})

test!(custom_multiple_build {
    let mut build1 = project("builder1");
    build1 = build1
        .file("Cargo.toml", r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [[bin]] name = "foo"
        "#)
        .file("src/foo.rs", r#"
            fn main() {
                let args = ::std::os::args();
                assert_eq!(args.get(1), &"hello".to_string());
                assert_eq!(args.get(2), &"world".to_string());
            }
        "#);
    assert_that(build1.cargo_process("build"),
                execs().with_status(0));

    let mut build2 = project("builder2");
    build2 = build2
        .file("Cargo.toml", r#"
            [project]

            name = "bar"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [[bin]] name = "bar"
        "#)
        .file("src/bar.rs", r#"
            fn main() {
                let args = ::std::os::args();
                assert_eq!(args.get(1), &"cargo".to_string());
            }
        "#);
    assert_that(build2.cargo_process("build"),
                execs().with_status(0));

    let mut p = project("foo");
    p = p
        .file("Cargo.toml", format!(r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]
            build = [ '{} hello world', '{} cargo' ]

            [[bin]] name = "foo"
        "#, build1.bin("foo").display(), build2.bin("bar").display()))
        .file("src/foo.rs", r#"
            fn main() {}
        "#);
    assert_that(p.cargo_process("build"),
                execs().with_status(0)
                       .with_stdout(format!("   Compiling foo v0.5.0 ({})\n",
                                            p.url()))
                       .with_stderr(""));
})

test!(custom_build_failure {
    let mut build = project("builder");
    build = build
        .file("Cargo.toml", r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [[bin]]
            name = "foo"
        "#)
        .file("src/foo.rs", r#"
            fn main() { fail!("nope") }
        "#);
    assert_that(build.cargo_process("build"), execs().with_status(0));


    let mut p = project("foo");
    p = p
        .file("Cargo.toml", format!(r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]
            build = '{}'

            [[bin]]
            name = "foo"
        "#, build.bin("foo").display()))
        .file("src/foo.rs", r#"
            fn main() {}
        "#);
    assert_that(p.cargo_process("build"),
                execs().with_status(101).with_stderr(format!("\
Failed to run custom build command for `foo v0.5.0 ({dir})
Process didn't exit successfully: `{}` (status=101)\n\
--- stderr\n\
task '<main>' failed at 'nope', {filename}:2\n\
\n\
", build.bin("foo").display(), filename = format!("src{}foo.rs", path::SEP),
   dir = p.url())));
})

test!(custom_second_build_failure {
    let mut build1 = project("builder1");
    build1 = build1
        .file("Cargo.toml", r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [[bin]] name = "foo"
        "#)
        .file("src/foo.rs", r#"
            fn main() { println!("Hello!"); }
        "#);
    assert_that(build1.cargo_process("build"),
                execs().with_status(0));

    let mut build2 = project("builder2");
    build2 = build2
        .file("Cargo.toml", r#"
            [project]

            name = "bar"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [[bin]]
            name = "bar"
        "#)
        .file("src/bar.rs", r#"
            fn main() { fail!("nope") }
        "#);
    assert_that(build2.cargo_process("build"), execs().with_status(0));


    let mut p = project("foo");
    p = p
        .file("Cargo.toml", format!(r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]
            build = [ '{}', '{}' ]

            [[bin]]
            name = "foo"
        "#, build1.bin("foo").display(), build2.bin("bar").display()))
        .file("src/foo.rs", r#"
            fn main() {}
        "#);
    assert_that(p.cargo_process("build"),
                execs().with_status(101).with_stderr(format!("\
Failed to run custom build command for `foo v0.5.0 ({dir})
Process didn't exit successfully: `{}` (status=101)\n\
--- stderr\n\
task '<main>' failed at 'nope', {filename}:2\n\
\n\
", build2.bin("bar").display(), filename = format!("src{}bar.rs", path::SEP),
   dir = p.url())));
})

test!(custom_build_env_vars {
    let bar = project("bar")
        .file("Cargo.toml", r#"
            [package]
            name = "bar-bar"
            version = "0.0.1"
            authors = []
            build = "true"
        "#)
        .file("src/lib.rs", "");
    bar.build();

    let mut p = project("foo");
    let mut build = project("builder");
    build = build
        .file("Cargo.toml", r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [[bin]]
            name = "foo"
        "#)
        .file("src/foo.rs", format!(r#"
            use std::os;
            fn main() {{
                let _ncpus = os::getenv("NUM_JOBS").unwrap();
                let out = os::getenv("OUT_DIR").unwrap();
                assert!(out.as_slice().starts_with(r"{0}"));
                assert!(Path::new(out).is_dir());

                let out = os::getenv("DEP_BAR_BAR_OUT_DIR").unwrap();
                assert!(out.as_slice().starts_with(r"{0}"));
                assert!(Path::new(out).is_dir());
            }}
        "#,
        p.root().join("target").join("native").display()));
    assert_that(build.cargo_process("build"), execs().with_status(0));


    p = p
        .file("Cargo.toml", format!(r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]
            build = '{}'

            [[bin]]
            name = "foo"

            [dependencies.bar-bar]
            path = '{}'
        "#, build.bin("foo").display(), bar.root().display()))
        .file("src/foo.rs", r#"
            fn main() {}
        "#);
    assert_that(p.cargo_process("build"), execs().with_status(0));
})

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
            static CARGO_MANIFEST_DIR: &'static str = env!("CARGO_MANIFEST_DIR");

            fn main() {
                let s = format!("{}-{}-{} @ {} in {}", VERSION_MAJOR,
                                VERSION_MINOR, VERSION_PATCH, VERSION_PRE,
                                CARGO_MANIFEST_DIR);
                assert_eq!(s, foo::version());
                println!("{}", s);
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

    assert_that(
      process(p.bin("foo")),
      execs().with_stdout(format!("0-5-1 @ alpha.1 in {}\n",
                                  p.root().display()).as_slice()));

    assert_that(p.process(cargo_dir().join("cargo")).arg("test"),
                execs().with_status(0));
})

test!(custom_build_in_dependency {
    let mut p = project("foo");
    let bar = p.root().join("bar");
    let mut build = project("builder");
    build = build
        .file("Cargo.toml", r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [[bin]]
            name = "foo"
        "#)
        .file("src/foo.rs", format!(r#"
            use std::os;
            fn main() {{
                assert!(os::getenv("OUT_DIR").unwrap().as_slice()
                           .starts_with(r"{}"));
            }}
        "#,
        p.root().join("target/native/bar-").display()));
    assert_that(build.cargo_process("build"), execs().with_status(0));


    p = p
        .file(".cargo/config", format!(r#"
            paths = ['{}']
        "#, bar.display()).as_slice())
        .file("Cargo.toml", r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [[bin]]
            name = "foo"
            [dependencies]
            bar = "0.5.0"
        "#)
        .file("src/foo.rs", r#"
            extern crate bar;
            fn main() { bar::bar() }
        "#)
        .file("bar/Cargo.toml", format!(r#"
            [project]

            name = "bar"
            version = "0.5.0"
            authors = ["wycats@example.com"]
            build = '{}'
        "#, build.bin("foo").display()))
        .file("bar/src/lib.rs", r#"
            pub fn bar() {}
        "#);
    assert_that(p.cargo_process("build"),
                execs().with_status(0));
})

// tests that custom build in dep can be built twice in a row - issue 227
test!(custom_build_in_dependency_twice {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [[bin]]
            name = "foo"
            [dependencies.bar]
            path = "./bar"
            "#)
        .file("src/foo.rs", r#"
            extern crate bar;
            fn main() { bar::bar() }
            "#)
        .file("bar/Cargo.toml", format!(r#"
            [project]

            name = "bar"
            version = "0.0.1"
            authors = ["wycats@example.com"]
            build = '{}'
        "#, "echo test"))
        .file("bar/src/lib.rs", r#"
            pub fn bar() {}
        "#);
    assert_that(p.cargo_process("build"),
                execs().with_status(0));
    assert_that(p.process(cargo_dir().join("cargo")).arg("build"),
                execs().with_status(0));
})

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

    let files = fs::readdir(&p.root().join("target")).assert();
    let mut files: Vec<String> = files.iter().filter_map(|f| {
        match f.filename_str().unwrap() {
            "deps" => None,
            s if s.contains("fingerprint") || s.contains("dSYM") => None,
            s => Some(s.to_string())
        }
    }).collect();
    files.sort();
    let file0 = files[0].as_slice();
    let file1 = files[1].as_slice();
    println!("{} {}", file0, file1);
    assert!(file0.ends_with(".rlib") || file1.ends_with(".rlib"));
    assert!(file0.ends_with(os::consts::DLL_SUFFIX) ||
            file1.ends_with(os::consts::DLL_SUFFIX));
})

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

    let files = fs::readdir(&p.root().join("target")).assert();
    let mut files: Vec<String> = files.iter().filter_map(|f| {
        match f.filename_str().unwrap() {
            "deps" => None,
            s if s.contains("fingerprint") || s.contains("dSYM") => None,
            s => Some(s.to_string())
        }
    }).collect();
    files.sort();
    let file0 = files[0].as_slice();
    let file1 = files[1].as_slice();
    println!("{} {}", file0, file1);
    assert!(file0.ends_with(".rlib") || file1.ends_with(".rlib"));
    assert!(file0.ends_with(os::consts::DLL_SUFFIX) ||
            file1.ends_with(os::consts::DLL_SUFFIX));
})

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
})

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
                execs().with_status(0));
})

#[cfg(not(windows))]
test!(ignore_broken_symlinks {
    let p = project("foo")
        .file("Cargo.toml", basic_bin_manifest("foo").as_slice())
        .file("src/foo.rs", main_file(r#""i am foo""#, []).as_slice())
        .symlink("Notafile", "bar");

    assert_that(p.cargo_process("build"), execs());
    assert_that(&p.bin("foo"), existing_file());

    assert_that(
      process(p.bin("foo")),
      execs().with_stdout("i am foo\n"));
})

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
                       .with_stderr("either a [lib] or [[bin]] section \
                                     must be present\n"));
})

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
{running} `rustc {dir}{sep}src{sep}lib.rs --crate-name test --crate-type lib -g \
        -C metadata=[..] \
        -C extra-filename=-[..] \
        --out-dir {dir}{sep}target \
        --dep-info [..] \
        -L {dir}{sep}target \
        -L {dir}{sep}target{sep}deps`
",
running = RUNNING, compiling = COMPILING, sep = path::SEP,
dir = p.root().display(),
url = p.url(),
)));
})

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
{running} `rustc {dir}{sep}src{sep}lib.rs --crate-name test --crate-type lib \
        --opt-level 3 \
        --cfg ndebug \
        -C metadata=[..] \
        -C extra-filename=-[..] \
        --out-dir {dir}{sep}target{sep}release \
        --dep-info [..] \
        -L {dir}{sep}target{sep}release \
        -L {dir}{sep}target{sep}release{sep}deps`
",
running = RUNNING, compiling = COMPILING, sep = path::SEP,
dir = p.root().display(),
url = p.url(),
)));
})

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
{running} `rustc {dir}{sep}foo{sep}src{sep}lib.rs --crate-name foo \
        --crate-type dylib --crate-type rlib \
        --opt-level 3 \
        --cfg ndebug \
        -C metadata=[..] \
        -C extra-filename=-[..] \
        --out-dir {dir}{sep}target{sep}release{sep}deps \
        --dep-info [..] \
        -L {dir}{sep}target{sep}release{sep}deps \
        -L {dir}{sep}target{sep}release{sep}deps`
{compiling} test v0.0.0 ({url})
{running} `rustc {dir}{sep}src{sep}lib.rs --crate-name test --crate-type lib \
        --opt-level 3 \
        --cfg ndebug \
        -C metadata=[..] \
        -C extra-filename=-[..] \
        --out-dir {dir}{sep}target{sep}release \
        --dep-info [..] \
        -L {dir}{sep}target{sep}release \
        -L {dir}{sep}target{sep}release{sep}deps \
        --extern foo={dir}{sep}target{sep}release{sep}deps/\
                     {prefix}foo-[..]{suffix} \
        --extern foo={dir}{sep}target{sep}release{sep}deps/libfoo-[..].rlib`
",
                    running = RUNNING,
                    compiling = COMPILING,
                    dir = p.root().display(),
                    url = p.url(),
                    sep = path::SEP,
                    prefix = os::consts::DLL_PREFIX,
                    suffix = os::consts::DLL_SUFFIX).as_slice()));
})

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

    assert_that(p.cargo_process("test"), execs());
    assert_that(process(p.bin("hello")), execs().with_stdout("Hello, World!\n"));
    assert_that(process(p.bin("goodbye")), execs().with_stdout("Goodbye, World!\n"));
})

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
            fn main() { println!("{}, {}!", world::get_hello(), world::get_world()); }
        "#)
        .file("examples/goodbye.rs", r#"
            extern crate world;
            fn main() { println!("{}, {}!", world::get_goodbye(), world::get_world()); }
        "#);

    assert_that(p.cargo_process("test"), execs().with_status(0));
    assert_that(process(p.bin("hello")), execs().with_stdout("Hello, World!\n"));
    assert_that(process(p.bin("goodbye")), execs().with_stdout("Goodbye, World!\n"));
})

test!(standard_build_no_ndebug {
    let p = project("world")
        .file("Cargo.toml", basic_bin_manifest("foo"))
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
    assert_that(process(p.bin("foo")), execs().with_stdout("slow\n"));
})

test!(release_build_ndebug {
    let p = project("world")
        .file("Cargo.toml", basic_bin_manifest("foo"))
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
    assert_that(process(p.bin("release/foo")), execs().with_stdout("fast\n"));
})

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
    assert_that(process(p.bin("foo")), execs().with_status(0));
})

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
})

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
    assert_that(process(p.bin("foo")), execs().with_status(0));
})

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
})

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

    assert_that(p.cargo_process("build"), execs().with_status(0));
})

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

    assert_that(p.cargo_process("build"), execs().with_status(0));
})

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
})

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
})

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
})

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
})

test!(freshness_ignores_excluded {
    let foo = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.0"
            authors = []
            build = "true"
            exclude = ["src/b*.rs"]
        "#)
        .file("src/lib.rs", "pub fn bar() -> int { 1 }");
    foo.build();
    foo.root().move_into_the_past().assert();

    assert_that(foo.process(cargo_dir().join("cargo")).arg("build"),
                execs().with_status(0)
                       .with_stdout(format!("\
{compiling} foo v0.0.0 ({url})
", compiling = COMPILING, url = foo.url())));

    // Smoke test to make sure it doesn't compile again
    println!("first pass");
    assert_that(foo.process(cargo_dir().join("cargo")).arg("build"),
                execs().with_status(0)
                       .with_stdout(""));

    // Modify an ignored file and make sure we don't rebuild
    println!("second pass");
    File::create(&foo.root().join("src/bar.rs")).assert();
    assert_that(foo.process(cargo_dir().join("cargo")).arg("build"),
                execs().with_status(0)
                       .with_stdout(""));
})

test!(rebuild_preserves_out_dir {
    let mut build = project("builder");
    build = build
        .file("Cargo.toml", r#"
            [package]
            name = "build"
            version = "0.5.0"
            authors = ["wycats@example.com"]
        "#)
        .file("src/main.rs", r#"
            use std::os;
            use std::io::File;

            fn main() {{
                let path = Path::new(os::getenv("OUT_DIR").unwrap()).join("foo");
                if os::getenv("FIRST").is_some() {
                    File::create(&path).unwrap();
                } else {
                    File::create(&path).unwrap();
                }
            }}
        "#);
    assert_that(build.cargo_process("build"), execs().with_status(0));

    let foo = project("foo")
        .file("Cargo.toml", format!(r#"
            [package]
            name = "foo"
            version = "0.0.0"
            authors = []
            build = '{}'
        "#, build.bin("build").display()).as_slice())
        .file("src/lib.rs", "pub fn bar() -> int { 1 }");
    foo.build();
    foo.root().move_into_the_past().assert();

    assert_that(foo.process(cargo_dir().join("cargo")).arg("build")
                   .env("FIRST", Some("1")),
                execs().with_status(0)
                       .with_stdout(format!("\
{compiling} foo v0.0.0 ({url})
", compiling = COMPILING, url = foo.url())));

    File::create(&foo.root().join("src/bar.rs")).assert();
    assert_that(foo.process(cargo_dir().join("cargo")).arg("build"),
                execs().with_status(0)
                       .with_stdout(format!("\
{compiling} foo v0.0.0 ({url})
", compiling = COMPILING, url = foo.url())));
})

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
        .file("src/lib.rs", "pub fn bar() -> int { 1 }")
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
})
