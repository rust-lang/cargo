use support::ChannelChanger;
use support::git;
use support::paths;
use support::registry::Package;
use support::{execs, project};
use support::hamcrest::assert_that;

#[test]
fn gated() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            bar = { package = "foo", version = "0.1" }
        "#,
        )
        .file("src/lib.rs", "")
        .build();

    assert_that(
        p.cargo("build").masquerade_as_nightly_cargo(),
        execs().with_status(101).with_stderr(
            "\
error: failed to parse manifest at `[..]`

Caused by:
  feature `rename-dependency` is required

consider adding `cargo-features = [\"rename-dependency\"]` to the manifest
",
        ),
    );

    let p = project().at("bar")
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            bar = { version = "0.1", package = "baz" }
        "#,
        )
        .file("src/lib.rs", "")
        .build();

    assert_that(
        p.cargo("build").masquerade_as_nightly_cargo(),
        execs().with_status(101).with_stderr(
            "\
error: failed to parse manifest at `[..]`

Caused by:
  feature `rename-dependency` is required

consider adding `cargo-features = [\"rename-dependency\"]` to the manifest
",
        ),
    );
}

#[test]
fn rename_dependency() {
    Package::new("bar", "0.1.0").publish();
    Package::new("bar", "0.2.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            cargo-features = ["rename-dependency"]

            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            bar = { version = "0.1.0" }
            baz = { version = "0.2.0", package = "bar" }
        "#,
        )
        .file(
            "src/lib.rs",
            "
            extern crate bar;
            extern crate baz;
        ",
        )
        .build();

    assert_that(
        p.cargo("build").masquerade_as_nightly_cargo(),
        execs().with_status(0),
    );
}

#[test]
fn rename_with_different_names() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            cargo-features = ["rename-dependency"]

            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies]
            baz = { path = "bar", package = "bar" }
        "#,
        )
        .file(
            "src/lib.rs",
            "
            extern crate baz;
        ",
        )
        .file(
            "bar/Cargo.toml",
            r#"
            [project]
            name = "bar"
            version = "0.0.1"
            authors = []

            [lib]
            name = "random_name"
        "#,
        )
        .file("bar/src/lib.rs", "")
        .build();

    assert_that(
        p.cargo("build").masquerade_as_nightly_cargo(),
        execs().with_status(0),
    );
}

#[test]
fn lots_of_names() {
    Package::new("foo", "0.1.0")
        .file("src/lib.rs", "pub fn foo1() {}")
        .publish();
    Package::new("foo", "0.2.0")
        .file("src/lib.rs", "pub fn foo() {}")
        .publish();
    Package::new("foo", "0.1.0")
        .file("src/lib.rs", "pub fn foo2() {}")
        .alternative(true)
        .publish();

    let g = git::repo(&paths::root().join("another"))
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                authors = []
            "#,
        )
        .file("src/lib.rs", "pub fn foo3() {}")
        .build();

    let p = project()
        .file(
            "Cargo.toml",
            &format!(r#"
                cargo-features = ["alternative-registries", "rename-dependency"]

                [package]
                name = "test"
                version = "0.1.0"
                authors = []

                [dependencies]
                foo = "0.2"
                foo1 = {{ version = "0.1", package = "foo" }}
                foo2 = {{ version = "0.1", registry = "alternative", package = "foo" }}
                foo3 = {{ git = '{}', package = "foo" }}
                foo4 = {{ path = "foo", package = "foo" }}
            "#,
            g.url())
        )
        .file(
            "src/lib.rs",
            "
                extern crate foo;
                extern crate foo1;
                extern crate foo2;
                extern crate foo3;
                extern crate foo4;

                pub fn foo() {
                    foo::foo();
                    foo1::foo1();
                    foo2::foo2();
                    foo3::foo3();
                    foo4::foo4();
                }
            ",
        )
        .file(
            "foo/Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.1.0"
                authors = []
            "#,
        )
        .file("foo/src/lib.rs", "pub fn foo4() {}")
        .build();

    assert_that(
        p.cargo("build -v").masquerade_as_nightly_cargo(),
        execs().with_status(0),
    );
}

#[test]
fn rename_and_patch() {
    Package::new("foo", "0.1.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["rename-dependency"]

                [package]
                name = "test"
                version = "0.1.0"
                authors = []

                [dependencies]
                bar = { version = "0.1", package = "foo" }

                [patch.crates-io]
                foo = { path = "foo" }
            "#,
        )
        .file(
            "src/lib.rs",
            "
                extern crate bar;

                pub fn foo() {
                    bar::foo();
                }
            ",
        )
        .file(
            "foo/Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.1.0"
                authors = []
            "#,
        )
        .file("foo/src/lib.rs", "pub fn foo() {}")
        .build();

    assert_that(
        p.cargo("build -v").masquerade_as_nightly_cargo(),
        execs().with_status(0),
    );
}

#[test]
fn rename_twice() {
    Package::new("foo", "0.1.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["rename-dependency"]

                [package]
                name = "test"
                version = "0.1.0"
                authors = []

                [dependencies]
                bar = { version = "0.1", package = "foo" }
                [build-dependencies]
                foo = { version = "0.1" }
            "#,
        )
        .file("src/lib.rs", "",)
        .build();

    assert_that(
        p.cargo("build -v").masquerade_as_nightly_cargo(),
        execs().with_status(101)
            .with_stderr("\
[UPDATING] registry `[..]`
[DOWNLOADING] foo v0.1.0 (registry [..])
error: multiple dependencies listed for the same crate must all have the same \
name, but the dependency on `foo v0.1.0` is listed as having different names
")
    );
}

#[test]
fn rename_affects_fingerprint() {
    Package::new("foo", "0.1.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["rename-dependency"]

                [package]
                name = "test"
                version = "0.1.0"
                authors = []

                [dependencies]
                foo = { version = "0.1", package = "foo" }
            "#,
        )
        .file("src/lib.rs", "extern crate foo;")
        .build();

    assert_that(
        p.cargo("build -v").masquerade_as_nightly_cargo(),
        execs().with_status(0),
    );

    p.change_file(
        "Cargo.toml",
        r#"
                cargo-features = ["rename-dependency"]

                [package]
                name = "test"
                version = "0.1.0"
                authors = []

                [dependencies]
                bar = { version = "0.1", package = "foo" }
        "#,
    );

    assert_that(
        p.cargo("build -v").masquerade_as_nightly_cargo(),
        execs().with_status(101),
    );
}
