use support::{project, execs};
use hamcrest::{assert_that, existing_file, not};

fn setup() {
}

test!(compile_simple_feature_deps {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [features]
            default = ["a"]
            a = []

            [[bin]]
            name = "foo"
            features = ["a"]
        "#)
        .file("src/main.rs", "fn main() {}");

    assert_that(p.cargo_process("build"),
                execs().with_status(0));

    assert_that(&p.bin("foo"), existing_file());

    assert_that(p.cargo_process("build").arg("--no-default-features"),
                execs().with_status(0));

    assert_that(&p.bin("foo"), not(existing_file()));
});

test!(compile_simple_feature_deps_args {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [features]
            a = []

            [[bin]]
            name = "foo"
            features = ["a"]
        "#)
        .file("src/main.rs", "fn main() {}");

    assert_that(p.cargo_process("build").arg("--features").arg("a"),
                execs().with_status(0));

    assert_that(&p.bin("foo"), existing_file());
});

test!(compile_multiple_feature_deps {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [features]
            default = ["a", "b"]
            a = []
            b = ["a"]
            c = []

            [[bin]]
            name = "foo_1"
            path = "src/foo_1.rs"
            features = ["b", "c"]

            [[bin]]
            name = "foo_2"
            path = "src/foo_2.rs"
            features = ["a"]
        "#)
        .file("src/foo_1.rs", "fn main() {}")
        .file("src/foo_2.rs", "fn main() {}");

    assert_that(p.cargo_process("build"),
                execs().with_status(0));

    assert_that(&p.bin("foo_1"), not(existing_file()));
    assert_that(&p.bin("foo_2"), existing_file());

    assert_that(p.cargo_process("build").arg("--features").arg("c"),
                execs().with_status(0));

    assert_that(&p.bin("foo_1"), existing_file());
    assert_that(&p.bin("foo_2"), existing_file());

    assert_that(p.cargo_process("build").arg("--no-default-features"),
                execs().with_status(0));

    assert_that(&p.bin("foo_1"), not(existing_file()));
    assert_that(&p.bin("foo_2"), not(existing_file()));
});

