use std::fs::File;
use std::io::prelude::*;

use support::{basic_manifest, execs, project};
use support::registry::Package;
use support::hamcrest::assert_that;

#[test]
fn minor_update_two_places() {
    Package::new("log", "0.1.0").publish();
    let p = project()
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

    assert_that(p.cargo("build"), execs());
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

    assert_that(p.cargo("build"), execs());
}

#[test]
fn transitive_minor_update() {
    Package::new("log", "0.1.0").publish();
    Package::new("serde", "0.1.0").dep("log", "0.1").publish();

    let p = project()
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

    assert_that(p.cargo("build"), execs());

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
        p.cargo("update -p serde"),
        execs().with_stderr(
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

    let p = project()
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

    assert_that(p.cargo("build"), execs());

    Package::new("log", "0.1.1").publish();
    Package::new("serde", "0.1.1").dep("log", "0.1").publish();

    assert_that(
        p.cargo("update -p serde"),
        execs().with_stderr(
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
    let p = project()
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

    assert_that(p.cargo("build"), execs());
    Package::new("log", "0.1.1").publish();

    p.uncomment_root_manifest();
    assert_that(
        p.cargo("build").env("RUST_LOG", "cargo=trace"),
        execs(),
    );
}

#[test]
fn update_via_new_member() {
    Package::new("log", "0.1.0").publish();
    let p = project()
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

    assert_that(p.cargo("build"), execs());
    Package::new("log", "0.1.1").publish();

    p.uncomment_root_manifest();
    assert_that(p.cargo("build"), execs());
}

#[test]
fn add_dep_deep_new_requirement() {
    Package::new("log", "0.1.0").publish();
    let p = project()
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

    assert_that(p.cargo("build"), execs());

    Package::new("log", "0.1.1").publish();
    Package::new("bar", "0.1.0").dep("log", "0.1.1").publish();

    p.uncomment_root_manifest();
    assert_that(p.cargo("build"), execs());
}

#[test]
fn everything_real_deep() {
    Package::new("log", "0.1.0").publish();
    Package::new("foo", "0.1.0").dep("log", "0.1").publish();
    let p = project()
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

    assert_that(p.cargo("build"), execs());

    Package::new("log", "0.1.1").publish();
    Package::new("bar", "0.1.0").dep("log", "0.1.1").publish();

    p.uncomment_root_manifest();
    assert_that(p.cargo("build"), execs());
}

#[test]
fn change_package_version() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "a-foo"
                version = "0.2.0-alpha"
                authors = []

                [dependencies]
                bar = { path = "bar", version = "0.2.0-alpha" }
            "#,
        )
        .file("src/lib.rs", "")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.2.0-alpha"))
        .file("bar/src/lib.rs", "")
        .file(
            "Cargo.lock",
            r#"
                [[package]]
                name = "foo"
                version = "0.2.0"
                dependencies = ["bar 0.2.0"]

                [[package]]
                name = "bar"
                version = "0.2.0"
            "#,
        )
        .build();

    assert_that(p.cargo("build"), execs());
}

#[test]
fn update_precise() {
    Package::new("log", "0.1.0").publish();
    Package::new("serde", "0.1.0").publish();
    Package::new("serde", "0.2.1").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.0.1"
                authors = []

                [dependencies]
                serde = "0.2"
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

    assert_that(p.cargo("build"), execs());

    Package::new("serde", "0.2.0").publish();

    assert_that(
        p.cargo("update -p serde:0.2.1 --precise 0.2.0"),
        execs().with_stderr(
            "\
[UPDATING] registry `[..]`
[UPDATING] serde v0.2.1 -> v0.2.0
",
        ),
    );
}
