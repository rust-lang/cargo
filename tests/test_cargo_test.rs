use support::{project, execs, basic_bin_manifest, COMPILING, cargo_dir};
use hamcrest::{assert_that, existing_file};
use cargo::util::process;

fn setup() {}

test!(cargo_test_simple {
    let p = project("foo")
        .file("Cargo.toml", basic_bin_manifest("foo").as_slice())
        .file("src/foo.rs", r#"
            fn hello() -> &'static str {
                "hello"
            }

            pub fn main() {
                println!("{}", hello())
            }

            #[test]
            fn test_hello() {
                assert_eq!(hello(), "hello")
            }"#);

    assert_that(p.cargo_process("cargo-build"), execs());
    assert_that(&p.bin("foo"), existing_file());

    assert_that(
        process(p.bin("foo")),
        execs().with_stdout("hello\n"));

    assert_that(p.process(cargo_dir().join("cargo-test")),
        execs().with_stdout(format!("{} foo v0.5.0 (file:{})\n\n\
                                    running 1 test\n\
                                    test test_hello ... ok\n\n\
                                    test result: ok. 1 passed; 0 failed; \
                                    0 ignored; 0 measured\n\n",
                                    COMPILING, p.root().display())));

    assert_that(&p.bin("test/foo"), existing_file());
})

test!(test_with_lib_dep {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/lib.rs", "pub fn foo(){}")
        .file("src/main.rs", "
            extern crate foo;
            fn main() {}
        ");

    assert_that(p.cargo_process("cargo-test"), execs().with_status(0));
})

test!(test_with_deep_lib_dep {
    let p = project("bar")
        .file("Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.0.1"
            authors = []

            [dependencies.foo]
            path = "../foo"
        "#)
        .file("src/lib.rs", "
            extern crate foo;
            #[test]
            fn bar_test() {
                foo::foo();
            }
        ");
    let p2 = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
        "#)
        .file("src/lib.rs", "
            pub fn foo() {}

            #[test]
            fn foo_test() {}
        ");

    p2.build();
    assert_that(p.cargo_process("cargo-test"),
                execs().with_status(0)
                       .with_stdout(format!("\
{compiling} foo v0.0.1 (file:{dir})
{compiling} bar v0.0.1 (file:{dir})

running 1 test
test bar_test ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured\n\n\
                       ",
                       compiling = COMPILING,
                       dir = p.root().display()).as_slice()));
})
