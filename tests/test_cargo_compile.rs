use support::{ResultTest,project,execs,realpath};
use hamcrest::{assert_that,existing_file};
use cargo;
use cargo::util::process;

fn setup() {
}

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

test!(cargo_compile {
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
        .with_stderr("Cargo.toml is not a valid Cargo manifest"));
})

test!(cargo_compile_without_manifest {
    let p = project("foo");

    assert_that(p.cargo_process("cargo-compile"),
        execs()
        .with_status(102)
        .with_stderr("Could not find Cargo.toml in this directory or any parent directory"));
})

test!(cargo_compile_with_invalid_code {
    let p = project("foo")
        .file("Cargo.toml", basic_bin_manifest("foo").as_slice())
        .file("src/foo.rs", "invalid rust code!");

    let target = realpath(&p.root().join("target")).assert();

    assert_that(p.cargo_process("cargo-compile"),
        execs()
        .with_status(101)
        .with_stderr(format!("src/foo.rs:1:1: 1:8 error: expected item but found `invalid`\nsrc/foo.rs:1 invalid rust code!\n             ^~~~~~~\nfailed to execute: `rustc src/foo.rs --crate-type bin --out-dir {} -L {}`", target.display(), target.join("deps").display()).as_slice()));
})

test!(cargo_compile_with_warnings_in_the_root_package {
    let p = project("foo")
        .file("Cargo.toml", basic_bin_manifest("foo").as_slice())
        .file("src/foo.rs", "fn main() {} fn dead() {}");

    assert_that(p.cargo_process("cargo-compile"),
        execs()
        .with_stderr("src/foo.rs:1:14: 1:26 warning: code is never used: `dead`, #[warn(dead_code)] on by default\nsrc/foo.rs:1 fn main() {} fn dead() {}\n                          ^~~~~~~~~~~~\n"));
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
        .file("src/foo.rs", main_file(r#""{}", bar::gimme()"#, ["bar"]).as_slice())
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

    assert_that(p.cargo_process("cargo-compile"),
        execs()
        .with_stdout("Compiling bar v0.5.0\nCompiling foo v0.5.0\n")
        .with_stderr(""));

    assert_that(&p.root().join("target/foo"), existing_file());

    assert_that(
      cargo::util::process("foo").extra_path(p.root().join("target")),
      execs().with_stdout("test passed\n"));
})

test!(cargo_compile_with_nested_deps {
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
        .file("src/foo.rs", main_file(r#""{}", bar::gimme()"#, ["bar"]).as_slice())
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

            [dependencies.bar]

            version = "0.5.0"

            [[bin]]

            name = "foo"
        "#)
        .file("src/foo.rs", main_file(r#""{}", bar::gimme()"#, ["bar"]).as_slice())
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

fn main_file(println: &str, deps: &[&str]) -> String {
    let mut buf = String::new();

    for dep in deps.iter() {
        buf.push_str(format!("extern crate {};\n", dep).as_slice());
    }

    buf.push_str("fn main() { println!(");
    buf.push_str(println);
    buf.push_str("); }\n");

    buf.to_str()
}

// test!(compiling_project_with_invalid_manifest)
