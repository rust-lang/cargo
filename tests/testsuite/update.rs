use std::fs::File;
use std::io::prelude::*;

use cargotest::support::{execs, project};
use cargotest::support::registry::Package;
use hamcrest::assert_that;

#[test]
fn minor_update_two_places() {
    Package::new("log", "0.1.0").publish();
    let p = project("foo")
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.0.1"
                authors = []

                [dependencies]
                log = "0.1"
                foo = { path = "foo" }
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "foo/Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies]
                log = "0.1"
            "#,
        )
        .file("foo/src/lib.rs", "")
        .build();

    assert_that(p.cargo("build"), execs().with_status(0));
    Package::new("log", "0.1.1").publish();

    File::create(p.root().join("foo/Cargo.toml"))
        .unwrap()
        .write_all(
            br#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies]
                log = "0.1.1"
            "#,
        )
        .unwrap();

    assert_that(p.cargo("build"), execs().with_status(0));
}

#[test]
fn transitive_minor_update() {
    Package::new("log", "0.1.0").publish();
    Package::new("serde", "0.1.0").dep("log", "0.1").publish();

    let p = project("foo")
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.0.1"
                authors = []

                [dependencies]
                serde = "0.1"
                log = "0.1"
                foo = { path = "foo" }
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "foo/Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies]
                serde = "0.1"
            "#,
        )
        .file("foo/src/lib.rs", "")
        .build();

    assert_that(p.cargo("build"), execs().with_status(0));

    Package::new("log", "0.1.1").publish();
    Package::new("serde", "0.1.1").dep("log", "0.1.1").publish();

    // Note that `serde` isn't actually updated here! The default behavior for
    // `update` right now is to as conservatively as possible attempt to satisfy
    // an update. In this case we previously locked the dependency graph to `log
    // 0.1.0`, but nothing on the command line says we're allowed to update
    // that. As a result the update of `serde` here shouldn't update to `serde
    // 0.1.1` as that would also force an update to `log 0.1.1`.
    //
    // Also note that this is probably counterintuitive and weird. We may wish
    // to change this one day.
    assert_that(
        p.cargo("update").arg("-p").arg("serde"),
        execs().with_status(0).with_stderr(
            "\
[UPDATING] registry `[..]`
",
        ),
    );
}

#[test]
fn conservative() {
    Package::new("log", "0.1.0").publish();
    Package::new("serde", "0.1.0").dep("log", "0.1").publish();

    let p = project("foo")
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.0.1"
                authors = []

                [dependencies]
                serde = "0.1"
                log = "0.1"
                foo = { path = "foo" }
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "foo/Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies]
                serde = "0.1"
            "#,
        )
        .file("foo/src/lib.rs", "")
        .build();

    assert_that(p.cargo("build"), execs().with_status(0));

    Package::new("log", "0.1.1").publish();
    Package::new("serde", "0.1.1").dep("log", "0.1").publish();

    assert_that(
        p.cargo("update").arg("-p").arg("serde"),
        execs().with_status(0).with_stderr(
            "\
[UPDATING] registry `[..]`
[UPDATING] serde v0.1.0 -> v0.1.1
",
        ),
    );
}

#[test]
fn update_via_new_dep() {
    Package::new("log", "0.1.0").publish();
    let p = project("foo")
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.0.1"
                authors = []

                [dependencies]
                log = "0.1"
                # foo = { path = "foo" }
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "foo/Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies]
                log = "0.1.1"
            "#,
        )
        .file("foo/src/lib.rs", "")
        .build();

    assert_that(p.cargo("build"), execs().with_status(0));
    Package::new("log", "0.1.1").publish();

    p.uncomment_root_manifest();
    assert_that(
        p.cargo("build").env("RUST_LOG", "cargo=trace"),
        execs().with_status(0),
    );
}

#[test]
fn update_via_new_member() {
    Package::new("log", "0.1.0").publish();
    let p = project("foo")
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.0.1"
                authors = []

                [workspace]
                # members = [ "foo" ]

                [dependencies]
                log = "0.1"
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "foo/Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies]
                log = "0.1.1"
            "#,
        )
        .file("foo/src/lib.rs", "")
        .build();

    assert_that(p.cargo("build"), execs().with_status(0));
    Package::new("log", "0.1.1").publish();

    p.uncomment_root_manifest();
    assert_that(p.cargo("build"), execs().with_status(0));
}

#[test]
fn add_dep_deep_new_requirement() {
    Package::new("log", "0.1.0").publish();
    let p = project("foo")
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.0.1"
                authors = []

                [dependencies]
                log = "0.1"
                # bar = "0.1"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    assert_that(p.cargo("build"), execs().with_status(0));

    Package::new("log", "0.1.1").publish();
    Package::new("bar", "0.1.0").dep("log", "0.1.1").publish();

    p.uncomment_root_manifest();
    assert_that(p.cargo("build"), execs().with_status(0));
}

#[test]
fn everything_real_deep() {
    Package::new("log", "0.1.0").publish();
    Package::new("foo", "0.1.0").dep("log", "0.1").publish();
    let p = project("foo")
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.0.1"
                authors = []

                [dependencies]
                foo = "0.1"
                # bar = "0.1"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    assert_that(p.cargo("build"), execs().with_status(0));

    Package::new("log", "0.1.1").publish();
    Package::new("bar", "0.1.0").dep("log", "0.1.1").publish();

    p.uncomment_root_manifest();
    assert_that(p.cargo("build"), execs().with_status(0));
}
