use support::project;

fn setup() {

}

test!(cargo_compile_with_explicit_manifest_path {
    project("foo")
        .file("Cargo.toml", r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]

            [[lib]]

            name = "foo"
        "#)
        .file("src/foo.rs", r#"
            fn main() {
                println!("i am foo");
            }"#)
        .build();

    fail!("not implemented");
    // 1) Setup project
    // 2) Run cargo-compile --manifest-path /tmp/bar/zomg
    // 3) assertThat(target/foo) exists assertThat("target/foo", isCompiledBin())
    // 4) Run target/foo, assert that output is ass expected (foo.rs == println!("i am foo"))
})

// test!(compiling_project_with_invalid_manifest)
