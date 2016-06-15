extern crate cargotest;
extern crate hamcrest;

use std::fs::{self, File};
use std::io::prelude::*;

use cargotest::support::{project, execs};
use cargotest::support::registry::{self, Package};
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
    println!("lock4");
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
        .file("bar/src/lib.rs", "");

    let lockfile = p.root().join("Cargo.lock");
    assert_that(p.cargo_process("generate-lockfile"),
                execs().with_status(0));
    assert_that(&lockfile,
                existing_file());
    assert_that(p.cargo("generate-lockfile"),
                execs().with_status(0));

    let mut lock0 = String::new();
    {
        File::open(&lockfile).unwrap().read_to_string(&mut lock0).unwrap();
    }

    assert!(lock0.starts_with("[root]\n"));

    let lock1 = lock0.replace("\n", "\r\n");
    {
        File::create(&lockfile).unwrap().write_all(lock1.as_bytes()).unwrap();
    }

    assert_that(p.cargo("generate-lockfile"),
                execs().with_status(0));

    let mut lock2 = String::new();
    {
        File::open(&lockfile).unwrap().read_to_string(&mut lock2).unwrap();
    }

    assert!(lock2.starts_with("[root]\r\n"));
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
        .file("src/main.rs", "fn main() {}");

    let lockfile = p.root().join("Cargo.lock");
    assert_that(&lockfile, is_not(existing_file()));
    assert_that(p.cargo_process("update"), execs().with_status(0).with_stdout(""));
    assert_that(&lockfile, existing_file());

    fs::remove_file(p.root().join("Cargo.lock")).unwrap();

    assert_that(&lockfile, is_not(existing_file()));
    assert_that(p.cargo("update"), execs().with_status(0).with_stdout(""));
    assert_that(&lockfile, existing_file());
}

#[test]
fn warn_about_multiple_versions_on_generate() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            bar = "0.0.1"
            baz = "*"
        "#)
        .file("src/main.rs", "fn main() {}");

    Package::new("bar", "0.0.1").publish();
    Package::new("bar", "0.0.2").publish();
    Package::new("baz", "0.0.1").dep("bar", "0.0.2").publish();

    assert_that(p.cargo_process("generate-lockfile"),
                execs().with_status(0)
                       .with_stderr_contains("\
[WARNING] using multiple versions of crate \"bar\"
versions: v0.0.1, v0.0.2
"));
}

#[test]
fn warn_about_multiple_versions_on_update() {
    let p = project("foo")
        .file("Cargo.toml", r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            bar = "0.0.1"
            baz = "*"
        "#)
        .file("src/main.rs", "fn main() {}");

    Package::new("bar", "0.0.1").publish();
    Package::new("bar", "0.0.2").publish();
    Package::new("baz", "0.0.1").dep("bar", "0.0.2").publish();

    assert_that(p.cargo_process("update"),
                execs().with_status(0)
                       .with_stderr_contains("\
[WARNING] using multiple versions of crate \"bar\"
versions: v0.0.1, v0.0.2
"));

    assert_that(p.cargo("update"),
                execs().with_status(0)
                .with_stderr(&format!("\
[UPDATING] registry `{}`", registry::registry())));
}
