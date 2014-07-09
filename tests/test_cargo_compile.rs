use std::io::fs;
use std::os;
use std::path;

use support::{ResultTest, project, execs, main_file, escape_path, basic_bin_manifest};
use support::COMPILING;
use hamcrest::{assert_that, existing_file};
use cargo;
use cargo::util::{process, realpath};

fn setup() {
}

test!(cargo_compile_simple {
    let p = project("foo")
        .file("Cargo.toml", basic_bin_manifest("foo").as_slice())
        .file("src/foo.rs", main_file(r#""i am foo""#, []).as_slice());

    assert_that(p.cargo_process("cargo-build"), execs());
    assert_that(&p.bin("foo"), existing_file());

    assert_that(
      process(p.bin("foo")),
      execs().with_stdout("i am foo\n"));
})

test!(cargo_compile_with_invalid_manifest {
    let p = project("foo")
        .file("Cargo.toml", "");

    assert_that(p.cargo_process("cargo-build"),
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

    assert_that(p.cargo_process("cargo-build"),
        execs()
        .with_status(101)
        .with_stderr("could not parse input TOML\n\
                      Cargo.toml:3:19-3:20 expected a value\n\n"))
})

test!(cargo_compile_with_invalid_version {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            authors = []
            version = "1.0"
        "#);

    assert_that(p.cargo_process("cargo-build"),
                execs()
                .with_status(101)
                .with_stderr("Cargo.toml is not a valid manifest\n\n\
                              invalid version: cannot parse '1.0' as a semver\n"))

})

test!(cargo_compile_without_manifest {
    let p = project("foo");

    assert_that(p.cargo_process("cargo-build"),
        execs()
        .with_status(102)
        .with_stderr("Could not find Cargo.toml in this directory or any \
                      parent directory\n"));
})

test!(cargo_compile_with_invalid_code {
    let p = project("foo")
        .file("Cargo.toml", basic_bin_manifest("foo").as_slice())
        .file("src/foo.rs", "invalid rust code!");

    let target = realpath(&p.root().join("target")).assert();

    assert_that(p.cargo_process("cargo-build"),
        execs()
        .with_status(101)
        .with_stderr(format!("\
{filename}:1:1: 1:8 error: expected item but found `invalid`
{filename}:1 invalid rust code!
             ^~~~~~~
Could not execute process \
`rustc {filename} --crate-name foo --crate-type bin -g -o {} -L {} -L {}` (status=101)\n",
            target.join("foo").display(),
            target.display(),
            target.join("deps").display(),
            filename = format!("src{}foo.rs", path::SEP)).as_slice()));
})

test!(cargo_compile_with_warnings_in_the_root_package {
    let p = project("foo")
        .file("Cargo.toml", basic_bin_manifest("foo").as_slice())
        .file("src/foo.rs", "fn main() {} fn dead() {}");

    assert_that(p.cargo_process("cargo-build"),
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
            paths = ["{}"]
        "#, escape_path(&bar)).as_slice())
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

            [[lib]]

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

    assert_that(p.cargo_process("cargo-build"),
        execs()
        .with_stdout(format!("{} bar v0.5.0 (file:{})\n\
                              {} foo v0.5.0 (file:{})\n",
                             COMPILING, bar.display(),
                             COMPILING, main.display()))
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
            paths = ["{}", "{}"]
        "#, escape_path(&bar), escape_path(&baz)).as_slice())
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

    p.cargo_process("cargo-build")
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
            paths = ["{}", "{}"]
        "#, escape_path(&bar), escape_path(&baz)).as_slice())
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

    p.cargo_process("cargo-build")
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
            paths = ["{}", "{}"]
        "#, escape_path(&bar), escape_path(&baz)).as_slice())
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

            [[lib]]

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

            [[lib]]

            name = "baz"
        "#)
        .file("baz/src/baz.rs", r#"
            pub fn gimme() -> String {
                "test passed".to_string()
            }
        "#);

    p.cargo_process("cargo-build")
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
            paths = ["{}", "{}"]
        "#, escape_path(&bar), escape_path(&baz)).as_slice())
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

            [[lib]]

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

            [[lib]]

            name = "baz"
        "#)
        .file("baz/src/baz.rs", r#"
            pub fn gimme() -> String {
                "test passed".to_string()
            }
        "#);

    assert_that(p.cargo_process("cargo-build"), execs());

    assert_that(&p.bin("foo"), existing_file());

    assert_that(
      cargo::util::process(p.bin("foo")),
      execs().with_stdout("test passed\n"));
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
    assert_that(build.cargo_process("cargo-build"),
                execs().with_status(0));


    let mut p = project("foo");
    p = p
        .file("Cargo.toml", format!(r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]
            build = "{}"

            [[bin]] name = "foo"
        "#, escape_path(&build.bin("foo"))))
        .file("src/foo.rs", r#"
            fn main() {}
        "#);
    assert_that(p.cargo_process("cargo-build"),
                execs().with_status(0)
                       .with_stdout(format!("   Compiling foo v0.5.0 (file:{})\n",
                                            p.root().display()))
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
    assert_that(build1.cargo_process("cargo-build"),
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
    assert_that(build2.cargo_process("cargo-build"),
                execs().with_status(0));

    let mut p = project("foo");
    p = p
        .file("Cargo.toml", format!(r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]
            build = [ "{} hello world", "{} cargo" ]

            [[bin]] name = "foo"
        "#, escape_path(&build1.bin("foo")), escape_path(&build2.bin("bar"))))
        .file("src/foo.rs", r#"
            fn main() {}
        "#);
    assert_that(p.cargo_process("cargo-build"),
                execs().with_status(0)
                       .with_stdout(format!("   Compiling foo v0.5.0 (file:{})\n",
                                            p.root().display()))
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
    assert_that(build.cargo_process("cargo-build"), execs().with_status(0));


    let mut p = project("foo");
    p = p
        .file("Cargo.toml", format!(r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]
            build = "{}"

            [[bin]]
            name = "foo"
        "#, escape_path(&build.bin("foo"))))
        .file("src/foo.rs", r#"
            fn main() {}
        "#);
    assert_that(p.cargo_process("cargo-build"),
                execs().with_status(101).with_stderr(format!("\
Could not execute process `{}` (status=101)\n\
--- stderr\n\
task '<main>' failed at 'nope', {filename}:2\n\
\n\
", build.bin("foo").display(), filename = format!("src{}foo.rs", path::SEP))));
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
    assert_that(build1.cargo_process("cargo-build"),
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
    assert_that(build2.cargo_process("cargo-build"), execs().with_status(0));


    let mut p = project("foo");
    p = p
        .file("Cargo.toml", format!(r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]
            build = [ "{}", "{}" ]

            [[bin]]
            name = "foo"
        "#, escape_path(&build1.bin("foo")), escape_path(&build2.bin("bar"))))
        .file("src/foo.rs", r#"
            fn main() {}
        "#);
    assert_that(p.cargo_process("cargo-build"),
                execs().with_status(101).with_stderr(format!("\
Could not execute process `{}` (status=101)\n\
--- stderr\n\
task '<main>' failed at 'nope', {filename}:2\n\
\n\
", build2.bin("bar").display(), filename = format!("src{}bar.rs", path::SEP))));
})

test!(custom_build_env_vars {
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
                assert_eq!(os::getenv("OUT_DIR").unwrap(), "{}".to_string());
                assert_eq!(os::getenv("DEPS_DIR").unwrap(), "{}".to_string());
            }}
        "#,
        escape_path(&p.root().join("target")),
        escape_path(&p.root().join("target").join("deps"))));
    assert_that(build.cargo_process("cargo-build"), execs().with_status(0));


    p = p
        .file("Cargo.toml", format!(r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]
            build = "{}"

            [[bin]]
            name = "foo"
        "#, escape_path(&build.bin("foo"))))
        .file("src/foo.rs", r#"
            fn main() {}
        "#);
    assert_that(p.cargo_process("cargo-build"), execs().with_status(0));
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
                assert_eq!(os::getenv("OUT_DIR").unwrap(), "{}".to_string());
                assert_eq!(os::getenv("DEPS_DIR").unwrap(), "{}".to_string());
            }}
        "#,
        escape_path(&p.root().join("target/deps")),
        escape_path(&p.root().join("target/deps"))));
    assert_that(build.cargo_process("cargo-build"), execs().with_status(0));


    p = p
        .file(".cargo/config", format!(r#"
            paths = ["{}"]
        "#, escape_path(&bar)).as_slice())
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
            build = "{}"
        "#, escape_path(&build.bin("foo"))))
        .file("bar/src/lib.rs", r#"
            pub fn bar() {}
        "#);
    assert_that(p.cargo_process("cargo-build"),
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

            [[lib]]

            name = "foo"
            crate_type = ["rlib", "dylib"]
        "#)
        .file("src/foo.rs", r#"
            pub fn foo() {}
        "#);
    assert_that(p.cargo_process("cargo-build"),
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
    let file0 = files.get(0).as_slice();
    let file1 = files.get(1).as_slice();
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

            [[lib]]

            name = "foo"
            crate_type = ["rlib", "dylib"]
        "#)
        .file("src/lib.rs", r#"
            pub fn foo() {}
        "#);
    assert_that(p.cargo_process("cargo-build"),
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
    let file0 = files.get(0).as_slice();
    let file1 = files.get(1).as_slice();
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

            [[lib]]

            name = "foo"
        "#)
        .file("src/foo.rs", r#"
            pub fn foo() {}
        "#);
    assert_that(p.cargo_process("cargo-build"),
                execs().with_status(0)
                       .with_stderr("unused manifest key: project.bulid\n"));

    let mut p = project("bar");
    p = p
        .file("Cargo.toml", r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [[lib]]

            name = "foo"
            build = "foo"
        "#)
        .file("src/foo.rs", r#"
            pub fn foo() {}
        "#);
    assert_that(p.cargo_process("cargo-build"),
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

            [[lib]]

            name = "test"
        "#)
        .file("src/test.rs", "fn main() {}");
    assert_that(p.cargo_process("cargo-build"),
                execs().with_status(0));
})

#[cfg(not(windows))]
test!(ignore_broken_symlinks {
    let p = project("foo")
        .file("Cargo.toml", basic_bin_manifest("foo").as_slice())
        .file("src/foo.rs", main_file(r#""i am foo""#, []).as_slice())
        .symlink("Notafile", "bar");

    assert_that(p.cargo_process("cargo-build"), execs());
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
    assert_that(p.cargo_process("cargo-build"),
                execs().with_status(101)
                       .with_stderr("either a [[lib]] or [[bin]] section \
                                     must be present\n"));
})
