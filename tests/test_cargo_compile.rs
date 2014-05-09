use support::{ResultTest,project,execs};
use hamcrest::{assert_that,existing_file};
use cargo;
use cargo::util::process;

fn setup() {
}

test!(cargo_compile {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [[bin]]

            name = "foo"
        "#)
        .file("src/foo.rs", main_file(r#""i am foo""#, []));

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

test!(cargo_compile_with_nested_deps {
    let mut p = project("foo");
    let bar = p.root().join("bar");
    let baz = p.root().join("baz");

    p = p
        .file(".cargo/config", format!(r#"
            paths = ["{}", "{}"]
        "#, bar.display(), baz.display()))
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
        .file("src/foo.rs", main_file(r#""{}", bar::gimme()"#, ["bar"]))
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

            pub fn gimme() -> ~str {
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
            pub fn gimme() -> ~str {
                "test passed".to_owned()
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

fn main_file(println: &str, deps: &[&str]) -> ~str {
    let mut buf = StrBuf::new();

    for dep in deps.iter() {
        buf.push_str(format!("extern crate {};\n", dep));
    }

    buf.push_str("fn main() { println!(");
    buf.push_str(println);
    buf.push_str("); }\n");

    buf.to_owned()
}

// test!(compiling_project_with_invalid_manifest)
