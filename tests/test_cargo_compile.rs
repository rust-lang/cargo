use std::io::fs;
use std::os;

use support::{ResultTest,project,execs,main_file};
use hamcrest::{assert_that,existing_file};
use cargo;
use cargo::util::{process,realpath};

fn setup() {
}

static COMPILING: &'static str = "   Compiling";

fn basic_bin_manifest(name: &str) -> String {
    format!(r#"
        [project]

        name = "{}"
        version = "0.5.0"
        authors = ["wycats@example.com"]

        [[bin]]

        name = "{}"
    "#, name, name)
}

test!(cargo_compile_simple {
    let p = project("foo")
        .file("Cargo.toml", basic_bin_manifest("foo").as_slice())
        .file("src/foo.rs", main_file(r#""i am foo""#, []).as_slice());

    assert_that(p.cargo_process("cargo-compile"), execs());
    assert_that(&p.root().join("target/foo"), existing_file());

    let target = p.root().join("target");

    assert_that(
      process("foo").extra_path(target),
      execs().with_stdout("i am foo\n"));
})

test!(cargo_compile_with_invalid_manifest {
    let p = project("foo")
        .file("Cargo.toml", "");

    assert_that(p.cargo_process("cargo-compile"),
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

    assert_that(p.cargo_process("cargo-compile"),
        execs()
        .with_status(101)
        .with_stderr("could not parse input TOML\n\
                      Cargo.toml:3:19-3:20 expected a value\n"))
})

test!(cargo_compile_without_manifest {
    let p = project("foo");

    assert_that(p.cargo_process("cargo-compile"),
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

    assert_that(p.cargo_process("cargo-compile"),
        execs()
        .with_status(101)
        .with_stderr(format!("\
src/foo.rs:1:1: 1:8 error: expected item but found `invalid`
src/foo.rs:1 invalid rust code!
             ^~~~~~~
Could not execute process \
`rustc src/foo.rs --crate-type bin --out-dir {} -L {} -L {}` (status=101)\n",
            target.display(),
            target.display(),
            target.join("deps").display()).as_slice()));
})

test!(cargo_compile_with_warnings_in_the_root_package {
    let p = project("foo")
        .file("Cargo.toml", basic_bin_manifest("foo").as_slice())
        .file("src/foo.rs", "fn main() {} fn dead() {}");

    assert_that(p.cargo_process("cargo-compile"),
        execs()
        .with_stderr("\
src/foo.rs:1:14: 1:26 warning: code is never used: `dead`, #[warn(dead_code)] \
on by default
src/foo.rs:1 fn main() {} fn dead() {}
                          ^~~~~~~~~~~~
"));
})

test!(cargo_compile_with_warnings_in_a_dep_package {
    let mut p = project("foo");
    let bar = p.root().join("bar");

    p = p
        .file(".cargo/config", format!(r#"
            paths = ["{}"]
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

            [[lib]]

            name = "bar"
        "#)
        .file("bar/src/bar.rs", r#"
            pub fn gimme() -> String {
                "test passed".to_str()
            }

            fn dead() {}
        "#);

    let bar = realpath(&p.root().join("bar")).assert();
    let main = realpath(&p.root()).assert();

    assert_that(p.cargo_process("cargo-compile"),
        execs()
        .with_stdout(format!("{} bar v0.5.0 (file:{})\n\
                              {} foo v0.5.0 (file:{})\n",
                             COMPILING, bar.display(),
                             COMPILING, main.display()))
        .with_stderr(""));

    assert_that(&p.root().join("target/foo"), existing_file());

    assert_that(
      cargo::util::process("foo").extra_path(p.root().join("target")),
      execs().with_stdout("test passed\n"));
})

test!(cargo_compile_with_nested_deps_shorthand {
    let mut p = project("foo");
    let bar = p.root().join("bar");
    let baz = p.root().join("baz");

    p = p
        .file(".cargo/config", format!(r#"
            paths = ["{}", "{}"]
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
                "test passed".to_str()
            }
        "#);

    p.cargo_process("cargo-compile")
        .exec_with_output()
        .assert();

    assert_that(&p.root().join("target/foo"), existing_file());

    assert_that(
      cargo::util::process("foo").extra_path(p.root().join("target")),
      execs().with_stdout("test passed\n"));
})

test!(cargo_compile_with_nested_deps_longhand {
    let mut p = project("foo");
    let bar = p.root().join("bar");
    let baz = p.root().join("baz");

    p = p
        .file(".cargo/config", format!(r#"
            paths = ["{}", "{}"]
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
                "test passed".to_str()
            }
        "#);

    assert_that(p.cargo_process("cargo-compile"), execs());

    assert_that(&p.root().join("target/foo"), existing_file());

    assert_that(
      cargo::util::process("foo").extra_path(p.root().join("target")),
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
    assert_that(build.cargo_process("cargo-compile"),
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
        "#, build.root().join("target/foo").display()))
        .file("src/foo.rs", r#"
            fn main() {}
        "#);
    assert_that(p.cargo_process("cargo-compile"),
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

            [[bin]] name = "foo"
        "#)
        .file("src/foo.rs", r#"
            fn main() { fail!("nope") }
        "#);
    assert_that(build.cargo_process("cargo-compile"), execs().with_status(0));


    let mut p = project("foo");
    p = p
        .file("Cargo.toml", format!(r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]
            build = "{}"

            [[bin]] name = "foo"
        "#, build.root().join("target/foo").display()))
        .file("src/foo.rs", r#"
            fn main() {}
        "#);
    assert_that(p.cargo_process("cargo-compile"),
                execs().with_status(101).with_stderr(format!("\
Could not execute process `{}` (status=101)
--- stderr
task '<main>' failed at 'nope', src/foo.rs:2

", build.root().join("target/foo").display())));
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

            [[bin]] name = "foo"
        "#)
        .file("src/foo.rs", format!(r#"
            use std::os;
            fn main() {{
                assert_eq!(os::getenv("OUT_DIR").unwrap(), "{}".to_str());
                assert_eq!(os::getenv("DEPS_DIR").unwrap(), "{}".to_str());
            }}
        "#,
        p.root().join("target").display(),
        p.root().join("target/deps").display()));
    assert_that(build.cargo_process("cargo-compile"), execs().with_status(0));


    p = p
        .file("Cargo.toml", format!(r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]
            build = "{}"

            [[bin]] name = "foo"
        "#, build.root().join("target/foo").display()))
        .file("src/foo.rs", r#"
            fn main() {}
        "#);
    assert_that(p.cargo_process("cargo-compile"), execs().with_status(0));
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

            [[bin]] name = "foo"
        "#)
        .file("src/foo.rs", format!(r#"
            use std::os;
            fn main() {{
                assert_eq!(os::getenv("OUT_DIR").unwrap(), "{}".to_str());
                assert_eq!(os::getenv("DEPS_DIR").unwrap(), "{}".to_str());
            }}
        "#,
        p.root().join("target/deps").display(),
        p.root().join("target/deps").display()));
    assert_that(build.cargo_process("cargo-compile"), execs().with_status(0));


    p = p
        .file(".cargo/config", format!(r#"
            paths = ["{}"]
        "#, bar.display()).as_slice())
        .file("Cargo.toml", r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [[bin]] name = "foo"
            [dependencies] bar = "0.5.0"
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

            [[lib]] name = "bar"
        "#, build.root().join("target/foo").display()))
        .file("bar/src/bar.rs", r#"
            pub fn bar() {}
        "#);
    assert_that(p.cargo_process("cargo-compile"),
                execs().with_status(0));
})

test!(many_crate_types {
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
    assert_that(p.cargo_process("cargo-compile"),
                execs().with_status(0));

    let files = fs::readdir(&p.root().join("target")).assert();
    let mut files: Vec<String> = files.iter().filter_map(|f| {
        match f.filename_str().unwrap() {
            "deps" => None,
            s if !s.starts_with("lib") => None,
            s => Some(s.to_str())
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
