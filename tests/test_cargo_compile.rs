use support::{project,execs};
use hamcrest::{assert_that,existing_file};
use cargo;

fn setup() {
}

// Currently doesn't pass due to the code being broken
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
      .exec()
      .unwrap();

    assert_that(&p.root().join("target/foo"), existing_file());

    assert_that(
      cargo::util::process("foo").extra_path(p.root().join("target")),
      execs().with_stdout("i am foo\n"));
})

// test!(compiling_project_with_invalid_manifest)
