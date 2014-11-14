use support::{project, execs};
use hamcrest::{assert_that, is, is_not, existing_file};

fn setup() {
}

test!(build_bin_only {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "proj"
            version = "0.0.1"
            authors = ["wycats@example.com"]

            [[bin]]
            name = "foo"
            path = "src/foo.rs"

            [[bin]]
            name = "bar"
            path = "src/bar.rs"
        "#)
        .file("src/foo.rs", r#"
            extern crate proj;
            fn main() {}
        "#)
        .file("src/bar.rs", r#"
            incorrect() program! should[] not<> be? built.
        "#)
        .file("src/lib.rs", r#" "#);
    assert_that(p.cargo_process("build").arg("--bin").arg("foo"),
    execs().with_status(0));
    assert_that(&p.bin("foo"), is(existing_file()));
    assert_that(&p.bin("bar"), is_not(existing_file()));
})

test!(build_bin_nonexistent {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "proj"
            version = "0.0.1"
            authors = ["wycats@example.com"]

            [[bin]]
            name = "foo"
            path = "src/foo.rs"
        "#)
        .file("src/foo.rs", r#"
            extern crate proj;
            fn main() {}
        "#)
        .file("src/lib.rs", r#" "#);
    assert_that(p.cargo_process("build").arg("--bin").arg("noexistent"),
    execs().with_status(101).with_stderr("\
    unknown bin target: noexistent
"));
    assert_that(&p.bin("foo"), is_not(existing_file()));
})
