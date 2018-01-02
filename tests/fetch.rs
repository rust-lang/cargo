extern crate cargotest;
#[macro_use]
extern crate hamcrest;

use cargotest::support::{project, execs};
use hamcrest::prelude::*;

#[test]
fn no_deps() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            authors = []
            version = "0.0.1"
        "#)
        .file("src/main.rs", r#"
            mod a; fn main() {}
        "#)
        .file("src/a.rs", "")
        .build();

    assert_that!(p.cargo("fetch"),
                execs().with_status(0).with_stdout(""));
}
