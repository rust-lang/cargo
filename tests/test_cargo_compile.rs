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

    println!("~~~~~~~");
    p.cargo_process("cargo")
      .args(["compile".to_owned(), "--manifest-path".to_owned(), "Cargo.toml".to_owned()])
      .exec()
      .unwrap();
    println!("~~~~~~~");

    assert_that(&p.root().join("target/foo"), existing_file());

    assert_that(
      cargo::util::process("foo").extra_path(p.root().join("target")),
      execs().with_stdout("i am foo\n"));
})

// test!(compiling_project_with_invalid_manifest)
