use std::io::File;

use support::{project, execs, cargo_dir};
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

    assert_that(p.cargo_process("build"),
                execs().with_status(0));

    let lockfile = p.root().join("Cargo.lock");
    let lock = File::open(&lockfile).read_to_string();
    let lock = lock.unwrap();
    let lock = lock.as_slice().replace("\n", "\r\n");
    File::create(&lockfile).write_str(lock.as_slice()).unwrap();
    assert_that(p.process(cargo_dir().join("cargo")).arg("build"),
                execs().with_status(0));
});

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
    let lock1 = File::open(&lockfile).read_to_string().unwrap();

    // add a dep
    File::create(&toml).write_str(r#"
        [package]
        name = "foo"
        authors = []
        version = "0.0.1"

        [dependencies.bar]
        path = "bar"
    "#).unwrap();
    assert_that(p.process(cargo_dir().join("cargo")).arg("generate-lockfile"),
                execs().with_status(0));
    let lock2 = File::open(&lockfile).read_to_string().unwrap();
    assert!(lock1 != lock2);

    // change the dep
    File::create(&p.root().join("bar/Cargo.toml")).write_str(r#"
        [package]
        name = "bar"
        authors = []
        version = "0.0.2"
    "#).unwrap();
    assert_that(p.process(cargo_dir().join("cargo")).arg("generate-lockfile"),
                execs().with_status(0));
    let lock3 = File::open(&lockfile).read_to_string().unwrap();
    assert!(lock1 != lock3);
    assert!(lock2 != lock3);

    // remove the dep
    File::create(&toml).write_str(r#"
        [package]
        name = "foo"
        authors = []
        version = "0.0.1"
    "#).unwrap();
    assert_that(p.process(cargo_dir().join("cargo")).arg("generate-lockfile"),
                execs().with_status(0));
    let lock4 = File::open(&lockfile).read_to_string().unwrap();
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
        let lock = File::open(&lockfile).read_to_string().unwrap();
        File::create(&lockfile).write_str((lock + metadata).as_slice()).unwrap();
    }

    // Build and make sure the metadata is still there
    assert_that(p.process(cargo_dir().join("cargo")).arg("build"),
                execs().with_status(0));
    let lock = File::open(&lockfile).read_to_string().unwrap();
    assert!(lock.as_slice().contains(metadata.trim()), "{}", lock);

    // Update and make sure the metadata is still there
    assert_that(p.process(cargo_dir().join("cargo")).arg("update"),
                execs().with_status(0));
    let lock = File::open(&lockfile).read_to_string().unwrap();
    assert!(lock.as_slice().contains(metadata.trim()), "{}", lock);
});
