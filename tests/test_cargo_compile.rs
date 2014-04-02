use std;
use support::{project,execs};
use hamcrest::{assert_that,existing_file};
use cargo;

fn setup() {
}

test!(cargo_compile_with_explicit_manifest_path {
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
            }"#)
        .build();

    let output = p.cargo_process("cargo-compile")
      .args([~"--manifest-path", ~"Cargo.toml"])
      .exec_with_output();

    match output {
      Ok(out) => {
        println!("out:\n{}\n", std::str::from_utf8(out.output));
        println!("err:\n{}\n", std::str::from_utf8(out.error));
      },
      Err(e) => println!("err: {}", e)
    }

    assert_that(&p.root().join("target/foo"), existing_file());

    assert_that(
      &cargo::util::process("foo").extra_path(p.root().join("target")),
      execs().with_stdout("i am foo\n"));
})

// test!(compiling_project_with_invalid_manifest)
