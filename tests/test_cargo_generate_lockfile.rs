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

test!(adding_and_removing_packages {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            authors = []
            version = "0.0.1"
        "#)
        .file("src/main.rs", "fn main() {}");

    assert_that(p.cargo_process("cargo-generate-lockfile"),
                execs().with_status(0));

    let lockfile = p.root().join("Cargo.lock");
    let toml = p.root().join("Cargo.toml");
    let lock1 = File::open(&lockfile).read_to_string().assert();

    // add a dep
    File::create(&toml).write_str(r#"
        [package]
        name = "foo"
        authors = []
        version = "0.0.1"

        [dependencies]
        bar = "0.5.0"
    "#).assert();
    assert_that(p.process(cargo_dir().join("cargo-generate-lockfile")),
                execs().with_status(0));
    let lock2 = File::open(&lockfile).read_to_string().assert();
    assert!(lock1 != lock2);

    // change the dep
    File::create(&toml).write_str(r#"
        [package]
        name = "foo"
        authors = []
        version = "0.0.1"

        [dependencies]
        bar = "0.2.0"
    "#).assert();
    assert_that(p.process(cargo_dir().join("cargo-generate-lockfile")),
                execs().with_status(0));
    let lock3 = File::open(&lockfile).read_to_string().assert();
    assert!(lock1 != lock3);
    assert!(lock2 != lock3);

    // remove the dep
    File::create(&toml).write_str(r#"
        [package]
        name = "foo"
        authors = []
        version = "0.0.1"
    "#).assert();
    assert_that(p.process(cargo_dir().join("cargo-generate-lockfile")),
                execs().with_status(0));
    let lock4 = File::open(&lockfile).read_to_string().assert();
    assert_eq!(lock1, lock4);
})
