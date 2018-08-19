use std::fs::{self, File};
use std::io::prelude::*;

use support::registry::Package;
use support::{basic_manifest, execs, paths, project, ProjectBuilder};
use support::ChannelChanger;
use support::hamcrest::{assert_that, existing_file, is_not};

#[test]
fn adding_and_removing_packages() {
    let p = project()
        .file("src/main.rs", "fn main() {}")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file("bar/src/lib.rs", "")
        .build();

    assert_that(p.cargo("generate-lockfile"), execs());

    let toml = p.root().join("Cargo.toml");
    let lock1 = p.read_lockfile();

    // add a dep
    File::create(&toml)
        .unwrap()
        .write_all(
            br#"
        [package]
        name = "foo"
        authors = []
        version = "0.0.1"

        [dependencies.bar]
        path = "bar"
    "#,
        )
        .unwrap();
    assert_that(p.cargo("generate-lockfile"), execs());
    let lock2 = p.read_lockfile();
    assert_ne!(lock1, lock2);

    // change the dep
    File::create(&p.root().join("bar/Cargo.toml"))
        .unwrap()
        .write_all(basic_manifest("bar", "0.0.2").as_bytes())
        .unwrap();
    assert_that(p.cargo("generate-lockfile"), execs());
    let lock3 = p.read_lockfile();
    assert_ne!(lock1, lock3);
    assert_ne!(lock2, lock3);

    // remove the dep
    println!("lock4");
    File::create(&toml)
        .unwrap()
        .write_all(
            br#"
        [package]
        name = "foo"
        authors = []
        version = "0.0.1"
    "#,
        )
        .unwrap();
    assert_that(p.cargo("generate-lockfile"), execs());
    let lock4 = p.read_lockfile();
    assert_eq!(lock1, lock4);
}

#[test]
fn no_index_update() {
    Package::new("serde", "1.0.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            authors = []
            version = "0.0.1"

            [dependencies]
            serde = "1.0"
        "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    assert_that(
        p.cargo("generate-lockfile"),
        execs().with_stderr("[UPDATING] registry `[..]`"),
    );

    assert_that(
        p.cargo("generate-lockfile -Zno-index-update")
            .masquerade_as_nightly_cargo(),
        execs().with_stdout("").with_stderr(""),
    );
}

#[test]
fn preserve_metadata() {
    let p = project()
        .file("src/main.rs", "fn main() {}")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file("bar/src/lib.rs", "")
        .build();

    assert_that(p.cargo("generate-lockfile"), execs());

    let metadata = r#"
[metadata]
bar = "baz"
foo = "bar"
"#;
    let lockfile = p.root().join("Cargo.lock");
    let lock = p.read_lockfile();
    let data = lock + metadata;
    File::create(&lockfile)
        .unwrap()
        .write_all(data.as_bytes())
        .unwrap();

    // Build and make sure the metadata is still there
    assert_that(p.cargo("build"), execs());
    let lock = p.read_lockfile();
    assert!(lock.contains(metadata.trim()), "{}", lock);

    // Update and make sure the metadata is still there
    assert_that(p.cargo("update"), execs());
    let lock = p.read_lockfile();
    assert!(lock.contains(metadata.trim()), "{}", lock);
}

#[test]
fn preserve_line_endings_issue_2076() {
    let p = project()
        .file("src/main.rs", "fn main() {}")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file("bar/src/lib.rs", "")
        .build();

    let lockfile = p.root().join("Cargo.lock");
    assert_that(p.cargo("generate-lockfile"), execs());
    assert_that(&lockfile, existing_file());
    assert_that(p.cargo("generate-lockfile"), execs());

    let lock0 = p.read_lockfile();

    assert!(lock0.starts_with("[[package]]\n"));

    let lock1 = lock0.replace("\n", "\r\n");
    {
        File::create(&lockfile)
            .unwrap()
            .write_all(lock1.as_bytes())
            .unwrap();
    }

    assert_that(p.cargo("generate-lockfile"), execs());

    let lock2 = p.read_lockfile();

    assert!(lock2.starts_with("[[package]]\r\n"));
    assert_eq!(lock1, lock2);
}

#[test]
fn cargo_update_generate_lockfile() {
    let p = project()
        .file("src/main.rs", "fn main() {}")
        .build();

    let lockfile = p.root().join("Cargo.lock");
    assert_that(&lockfile, is_not(existing_file()));
    assert_that(p.cargo("update"), execs().with_stdout(""));
    assert_that(&lockfile, existing_file());

    fs::remove_file(p.root().join("Cargo.lock")).unwrap();

    assert_that(&lockfile, is_not(existing_file()));
    assert_that(p.cargo("update"), execs().with_stdout(""));
    assert_that(&lockfile, existing_file());
}

#[test]
fn duplicate_entries_in_lockfile() {
    let _a = ProjectBuilder::new(paths::root().join("a"))
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "a"
            authors = []
            version = "0.0.1"

            [dependencies]
            common = {path="common"}
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    let common_toml = &basic_manifest("common", "0.0.1");

    let _common_in_a = ProjectBuilder::new(paths::root().join("a/common"))
        .file("Cargo.toml", common_toml)
        .file("src/lib.rs", "")
        .build();

    let b = ProjectBuilder::new(paths::root().join("b"))
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "b"
            authors = []
            version = "0.0.1"

            [dependencies]
            common = {path="common"}
            a = {path="../a"}
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    let _common_in_b = ProjectBuilder::new(paths::root().join("b/common"))
        .file("Cargo.toml", common_toml)
        .file("src/lib.rs", "")
        .build();

    // should fail due to a duplicate package `common` in the lockfile
    assert_that(
        b.cargo("build"),
        execs().with_status(101).with_stderr_contains(
            "[..]package collision in the lockfile: packages common [..] and \
             common [..] are different, but only one can be written to \
             lockfile unambigiously",
        ),
    );
}
