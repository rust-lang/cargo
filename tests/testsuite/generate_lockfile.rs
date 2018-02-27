use std::fs::{self, File};
use std::io::prelude::*;

use cargotest::support::{project, execs};
use hamcrest::{assert_that, existing_file, is_not};

#[test]
fn adding_and_removing_packages() {
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
        .file("bar/src/lib.rs", "")
        .build();

    assert_that(p.cargo("generate-lockfile"),
                execs().with_status(0));

    let toml = p.root().join("Cargo.toml");
    let lock1 = p.read_lockfile();

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
    let lock2 = p.read_lockfile();
    assert_ne!(lock1, lock2);

    // change the dep
    File::create(&p.root().join("bar/Cargo.toml")).unwrap().write_all(br#"
        [package]
        name = "bar"
        authors = []
        version = "0.0.2"
    "#).unwrap();
    assert_that(p.cargo("generate-lockfile"),
                execs().with_status(0));
    let lock3 = p.read_lockfile();
    assert_ne!(lock1, lock3);
    assert_ne!(lock2, lock3);

    // remove the dep
    println!("lock4");
    File::create(&toml).unwrap().write_all(br#"
        [package]
        name = "foo"
        authors = []
        version = "0.0.1"
    "#).unwrap();
    assert_that(p.cargo("generate-lockfile"),
                execs().with_status(0));
    let lock4 = p.read_lockfile();
    assert_eq!(lock1, lock4);
}

#[test]
fn no_index_update() {
    use cargotest::ChannelChanger;
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            authors = []
            version = "0.0.1"

            [dependencies]
            serde = "1.0"
        "#)
        .file("src/main.rs", "fn main() {}")
        .build();

    assert_that(p.cargo("generate-lockfile"),
                execs().with_status(0).with_stdout("")
                    .with_stderr_contains("    Updating registry `https://github.com/rust-lang/crates.io-index`"));

    assert_that(p.cargo("generate-lockfile").masquerade_as_nightly_cargo().arg("-Zno-index-update"),
                execs().with_status(0).with_stdout("")
                    .with_stderr_does_not_contain("    Updating registry `https://github.com/rust-lang/crates.io-index`"));
}

#[test]
fn preserve_metadata() {
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
        .file("bar/src/lib.rs", "")
        .build();

    assert_that(p.cargo("generate-lockfile"),
                execs().with_status(0));

    let metadata = r#"
[metadata]
bar = "baz"
foo = "bar"
"#;
    let lockfile = p.root().join("Cargo.lock");
    let lock = p.read_lockfile();
    let data = lock + metadata;
    File::create(&lockfile).unwrap().write_all(data.as_bytes()).unwrap();

    // Build and make sure the metadata is still there
    assert_that(p.cargo("build"),
                execs().with_status(0));
    let lock = p.read_lockfile();
    assert!(lock.contains(metadata.trim()), "{}", lock);

    // Update and make sure the metadata is still there
    assert_that(p.cargo("update"),
                execs().with_status(0));
    let lock = p.read_lockfile();
    assert!(lock.contains(metadata.trim()), "{}", lock);
}

#[test]
fn preserve_line_endings_issue_2076() {
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
        .file("bar/src/lib.rs", "")
        .build();

    let lockfile = p.root().join("Cargo.lock");
    assert_that(p.cargo("generate-lockfile"),
                execs().with_status(0));
    assert_that(&lockfile,
                existing_file());
    assert_that(p.cargo("generate-lockfile"),
                execs().with_status(0));

    let lock0 = p.read_lockfile();

    assert!(lock0.starts_with("[[package]]\n"));

    let lock1 = lock0.replace("\n", "\r\n");
    {
        File::create(&lockfile).unwrap().write_all(lock1.as_bytes()).unwrap();
    }

    assert_that(p.cargo("generate-lockfile"),
                execs().with_status(0));

    let lock2 = p.read_lockfile();

    assert!(lock2.starts_with("[[package]]\r\n"));
    assert_eq!(lock1, lock2);
}

#[test]
fn cargo_update_generate_lockfile() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [package]
            name = "foo"
            authors = []
            version = "0.0.1"
        "#)
        .file("src/main.rs", "fn main() {}")
        .build();

    let lockfile = p.root().join("Cargo.lock");
    assert_that(&lockfile, is_not(existing_file()));
    assert_that(p.cargo("update"), execs().with_status(0).with_stdout(""));
    assert_that(&lockfile, existing_file());

    fs::remove_file(p.root().join("Cargo.lock")).unwrap();

    assert_that(&lockfile, is_not(existing_file()));
    assert_that(p.cargo("update"), execs().with_status(0).with_stdout(""));
    assert_that(&lockfile, existing_file());

}
