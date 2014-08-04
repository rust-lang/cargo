use std::io::File;

use support::{project, execs, cargo_dir, ResultTest};
use support::paths::PathExt;
use hamcrest::assert_that;

fn setup() {}

test!(ignores_carriage_return {
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
        .file("src/a.rs", "");

    assert_that(p.cargo_process("cargo-build"),
                execs().with_status(0));

    let lockfile = p.root().join("Cargo.lock");
    let lock = File::open(&lockfile).read_to_string();
    let lock = lock.assert();
    let lock = lock.as_slice().replace("\n", "\r\n");
    File::create(&lockfile).write_str(lock.as_slice()).assert();
    lockfile.move_into_the_past().assert();
    let mtime = lockfile.stat().assert().modified;
    assert_that(p.process(cargo_dir().join("cargo-build")),
                execs().with_status(0));
    assert_eq!(lockfile.stat().assert().modified, mtime);
})
