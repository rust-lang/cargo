use std::fs::File;
use std::io::prelude::*;

use support::{project, execs};
use hamcrest::assert_that;

fn setup() {}

test!(adding_and_removing_packages {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            authors = []
            version = "0.0.1"
        "#)
        .file("src/main.rs", "fn main() {}")
        .file("bar/Cargo.toml", r#"
            [package]
            name = "bar"
            authors = []
            version = "0.0.1"
        "#)
        .file("bar/src/lib.rs", "");

    assert_that(p.cargo_process("generate-lockfile"),
                execs().with_status(0));

    let lockfile = p.root().join("Cargo.lock");
    let toml = p.root().join("Cargo.toml");
    let mut lock1 = String::new();
    File::open(&lockfile).unwrap().read_to_string(&mut lock1).unwrap();

    // add a dep
    File::create(&toml).unwrap().write_all(br#"
        [package]
        name = "foo"
        authors = []
        version = "0.0.1"

        [dependencies.bar]
        path = "bar"
    "#).unwrap();
    assert_that(p.cargo("generate-lockfile"),
                execs().with_status(0));
    let mut lock2 = String::new();
    File::open(&lockfile).unwrap().read_to_string(&mut lock2).unwrap();
    assert!(lock1 != lock2);

    // change the dep
    File::create(&p.root().join("bar/Cargo.toml")).unwrap().write_all(br#"
        [package]
        name = "bar"
        authors = []
        version = "0.0.2"
    "#).unwrap();
    assert_that(p.cargo("generate-lockfile"),
                execs().with_status(0));
    let mut lock3 = String::new();
    File::open(&lockfile).unwrap().read_to_string(&mut lock3).unwrap();
    assert!(lock1 != lock3);
    assert!(lock2 != lock3);

    // remove the dep
    File::create(&toml).unwrap().write_all(br#"
        [package]
        name = "foo"
        authors = []
        version = "0.0.1"
    "#).unwrap();
    assert_that(p.cargo("generate-lockfile"),
                execs().with_status(0));
    let mut lock4 = String::new();
    File::open(&lockfile).unwrap().read_to_string(&mut lock4).unwrap();
    assert_eq!(lock1, lock4);
});

test!(preserve_metadata {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            authors = []
            version = "0.0.1"
        "#)
        .file("src/main.rs", "fn main() {}")
        .file("bar/Cargo.toml", r#"
            [package]
            name = "bar"
            authors = []
            version = "0.0.1"
        "#)
        .file("bar/src/lib.rs", "");

    assert_that(p.cargo_process("generate-lockfile"),
                execs().with_status(0));

    let metadata = r#"
[metadata]
bar = "baz"
foo = "bar"
"#;
    let lockfile = p.root().join("Cargo.lock");
    {
        let mut lock = String::new();
        File::open(&lockfile).unwrap().read_to_string(&mut lock).unwrap();
        let data = lock + metadata;
        File::create(&lockfile).unwrap().write_all(data.as_bytes()).unwrap();
    }

    // Build and make sure the metadata is still there
    assert_that(p.cargo("build"),
                execs().with_status(0));
    let mut lock = String::new();
    File::open(&lockfile).unwrap().read_to_string(&mut lock).unwrap();
    assert!(lock.contains(metadata.trim()), "{}", lock);

    // Update and make sure the metadata is still there
    assert_that(p.cargo("update"),
                execs().with_status(0));
    let mut lock = String::new();
    File::open(&lockfile).unwrap().read_to_string(&mut lock).unwrap();
    assert!(lock.contains(metadata.trim()), "{}", lock);
});
