use std::path::MAIN_SEPARATOR as SEP;

use support::{project, execs};
use support::{COMPILING, RUNNING};
use hamcrest::assert_that;

fn setup() {
}

test!(basic_per_dependency_opt_levels {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.bar]
            path = "bar"
            opt_level = 2
        "#)
        .file("src/main.rs", r#"
            extern crate bar;
            fn main() {}
        "#)
        .file("bar/Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.0.1"
            authors = []
        "#)
        .file("bar/src/lib.rs", "pub fn bar() {}");

    assert_that(p.cargo_process("build").arg("-v"),
                execs().with_status(0).with_stdout(format!("\
{compiling} bar v0.0.1 ({url})
{running} `rustc bar{sep}src{sep}lib.rs --crate-name bar --crate-type lib \
        -C opt-level=2 -g [..]
{compiling} foo v0.0.1 ({url})
{running} `rustc src{sep}main.rs --crate-name foo --crate-type bin -g \
        --out-dir {dir}{sep}target{sep}debug [..]",
        running = RUNNING, compiling = COMPILING, sep = SEP,
        dir = p.root().display(), url = p.url())));
});

test!(highest_opt_level_wins_in_per_dependency_opt_levels {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.bar]
            path = "bar"

            [dependencies.baz]
            path = "baz"
            opt_level = 1
        "#)
        .file("src/main.rs", r#"
            extern crate bar;
            fn main() {}
        "#)
        .file("bar/Cargo.toml", r#"
            [package]
            name = "bar"
            version = "0.0.1"
            authors = []

            [dependencies.baz]
            path = "../baz"
            opt_level = 2
        "#)
        .file("bar/src/lib.rs", r#"
            extern crate baz;
            pub fn bar() {}
        "#)
        .file("baz/Cargo.toml", r#"
            [package]
            name = "baz"
            version = "0.0.1"
            authors = []
        "#)
        .file("baz/src/lib.rs", "pub fn baz() {}");

    assert_that(p.cargo_process("build").arg("-v"),
                execs().with_status(0).with_stdout(format!("\
{compiling} baz v0.0.1 ({url})
{running} `rustc baz{sep}src{sep}lib.rs --crate-name baz --crate-type lib \
        -C opt-level=2 -g [..]
{compiling} bar v0.0.1 ({url})
{running} `rustc bar{sep}src{sep}lib.rs --crate-name bar --crate-type lib \
        -g [..]
{compiling} foo v0.0.1 ({url})
{running} `rustc src{sep}main.rs --crate-name foo --crate-type bin -g \
        --out-dir {dir}{sep}target{sep}debug [..]",
        running = RUNNING, compiling = COMPILING, sep = SEP,
        dir = p.root().display(), url = p.url())));
});

