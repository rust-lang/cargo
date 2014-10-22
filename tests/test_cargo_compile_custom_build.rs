use support::{project, execs};
use hamcrest::{assert_that};

fn setup() {
}

test!(custom_build_compiled {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]

            name = "foo"
            version = "0.5.0"
            authors = ["wycats@example.com"]
            build = 'build.rs'
        "#)
        .file("src/main.rs", r#"
            fn main() {}
        "#)
        .file("build.rs", r#"
        	invalid rust file, should trigger a build error
        "#);
    assert_that(p.cargo_process("build"),
                execs().with_status(101));
})
