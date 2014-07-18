use support::{project, execs};
use support::{PACKAGING};
use hamcrest::{assert_that, existing_file};

fn setup() {
}

test!(simple {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/main.rs", r#"
            fn main() { println!("hello"); }
        "#);

    assert_that(p.cargo_process("package"),
                execs().with_status(0).with_stdout(format!("\
{packaging} foo v0.0.1 ({dir})
",
        packaging = PACKAGING,
        dir = p.url()).as_slice()));
    assert_that(&p.root().join("foo-0.0.1.tar.gz"), existing_file());
})
