use std;
use support::project;
use hamcrest::{SelfDescribing,Description,Matcher,assert_that};
use cargo;

#[deriving(Clone,Eq)]
pub struct ExistingFile;

impl SelfDescribing for ExistingFile {
  fn describe_to(&self, desc: &mut Description) {
    desc.append_text("an existing file");
  }
}

impl Matcher<Path> for ExistingFile {
  fn matches(&self, actual: &Path) -> bool {
    actual.exists()
  }

  fn describe_mismatch(&self, actual: &Path, desc: &mut Description) {
    desc.append_text(format!("`{}` was missing", actual.display()));
  }
}

pub fn existing_file() -> ExistingFile {
  ExistingFile
}

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

    let output = cargo::util::process("cargo-compile")
      .args([~"--manifest-path", ~"Cargo.toml"])
      .extra_path(target_path())
      .cwd(p.root())
      .exec_with_output();

    match output {
      Ok(out) => {
        println!("out:\n{}\n", std::str::from_utf8(out.output));
        println!("err:\n{}\n", std::str::from_utf8(out.error));
      },
      Err(e) => println!("err: {}", e)
    }

    assert_that(p.root().join("target/foo/bar"), existing_file());
    assert!(p.root().join("target/foo").exists(), "the executable exists");

    let o = cargo::util::process("foo")
      .extra_path(format!("{}", p.root().join("target").display()))
      .exec_with_output()
      .unwrap();

    assert_eq!(std::str::from_utf8(o.output).unwrap(), "i am foo\n");
})

// test!(compiling_project_with_invalid_manifest)

fn target_path() -> ~str {
  std::os::getenv("CARGO_BIN_PATH").unwrap_or_else(|| {
    fail!("CARGO_BIN_PATH wasn't set. Cannot continue running test")
  })
}
