use std;
use support::project;
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

    assert!(p.root().join("target/foo").exists(), "the executable exists");

    let o = cargo::util::process("foo")
      .extra_path(format!("{}", p.root().join("target").display()))
      .exec_with_output()
      .unwrap();

    assert_eq!(std::str::from_utf8(o.output).unwrap(), "i am foo\n");

    // 1) Setup project
    // 2) Run cargo-compile --manifest-path /tmp/bar/zomg
    // 3) assertThat(target/foo) exists assertThat("target/foo", isCompiledBin())
    // 4) Run target/foo, assert that output is ass expected (foo.rs == println!("i am foo"))
})

// test!(compiling_project_with_invalid_manifest)

fn target_path() -> ~str {
  std::os::getenv("CARGO_BIN_PATH").unwrap_or_else(|| {
    fail!("CARGO_BIN_PATH wasn't set. Cannot continue running test")
  })
}
