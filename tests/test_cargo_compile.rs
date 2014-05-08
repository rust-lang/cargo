use support::{ResultTest,project,execs};
use hamcrest::{assert_that,existing_file};
use cargo;

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
        .file("src/foo.rs", r#"
            fn main() {
                println!("i am foo");
            }
        "#)
        .build();

    p.cargo_process("cargo-compile")
      .args([])
      .exec_with_output()
      .assert();

    assert_that(&p.root().join("target/foo"), existing_file());

    assert_that(
      cargo::util::process("foo").extra_path(p.root().join("target")),
      execs().with_stdout("i am foo\n"));
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
        .file("src/foo.rs", r#"
            extern crate bar;

            fn main() {
                println!("{}", bar::gimme());
            }
        "#)
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
        "#)
        .build();

    p.cargo_process("cargo-compile")
        .exec_with_output()
        .assert();

    assert_that(&p.root().join("target/foo"), existing_file());

    assert_that(
      cargo::util::process("foo").extra_path(p.root().join("target")),
      execs().with_stdout("test passed\n"));
})

// test!(compiling_project_with_invalid_manifest)
