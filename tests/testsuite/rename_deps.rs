use support::git;
use support::paths;
use support::registry::Package;
use support::{basic_manifest, project};

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
        ).file("src/lib.rs", "")
        .build();

    p.cargo("build")
        .masquerade_as_nightly_cargo()
        .with_status(101)
        .with_stderr(
            "\
error: failed to parse manifest at `[..]`

Caused by:
  feature `rename-dependency` is required

consider adding `cargo-features = [\"rename-dependency\"]` to the manifest
",
        ).run();

    let p = project()
        .at("bar")
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
        ).file("src/lib.rs", "")
        .build();

    p.cargo("build")
        .masquerade_as_nightly_cargo()
        .with_status(101)
        .with_stderr(
            "\
error: failed to parse manifest at `[..]`

Caused by:
  feature `rename-dependency` is required

consider adding `cargo-features = [\"rename-dependency\"]` to the manifest
",
        ).run();
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
        ).file("src/lib.rs", "extern crate bar; extern crate baz;")
        .build();

    p.cargo("build").masquerade_as_nightly_cargo().run();
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
        ).file("src/lib.rs", "extern crate baz;")
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
        ).file("bar/src/lib.rs", "")
        .build();

    p.cargo("build").masquerade_as_nightly_cargo().run();
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
        .file("Cargo.toml", &basic_manifest("foo", "0.1.0"))
        .file("src/lib.rs", "pub fn foo3() {}")
        .build();

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
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
                g.url()
            ),
        ).file(
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
        ).file("foo/Cargo.toml", &basic_manifest("foo", "0.1.0"))
        .file("foo/src/lib.rs", "pub fn foo4() {}")
        .build();

    p.cargo("build -v").masquerade_as_nightly_cargo().run();
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
        ).file(
            "src/lib.rs",
            "extern crate bar; pub fn foo() { bar::foo(); }",
        ).file("foo/Cargo.toml", &basic_manifest("foo", "0.1.0"))
        .file("foo/src/lib.rs", "pub fn foo() {}")
        .build();

    p.cargo("build -v").masquerade_as_nightly_cargo().run();
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
        ).file("src/lib.rs", "")
        .build();

    p.cargo("build -v")
        .masquerade_as_nightly_cargo()
        .with_status(101)
        .with_stderr(
            "\
[UPDATING] `[..]` index
[DOWNLOADING] crates ...
[DOWNLOADED] foo v0.1.0 (registry [..])
error: multiple dependencies listed for the same crate must all have the same \
name, but the dependency on `foo v0.1.0` is listed as having different names
",
        ).run();
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
        ).file("src/lib.rs", "extern crate foo;")
        .build();

    p.cargo("build -v").masquerade_as_nightly_cargo().run();

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

    p.cargo("build -v")
        .masquerade_as_nightly_cargo()
        .with_status(101)
        .run();
}

#[test]
fn can_run_doc_tests() {
    Package::new("bar", "0.1.0").publish();
    Package::new("bar", "0.2.0").publish();

    let foo = project()
        .file(
            "Cargo.toml",
            r#"
            cargo-features = ["rename-dependency"]

            [project]
            name = "foo"
            version = "0.0.1"

            [dependencies]
            bar = { version = "0.1.0" }
            baz = { version = "0.2.0", package = "bar" }
        "#,
        ).file(
            "src/lib.rs",
            "
            extern crate bar;
            extern crate baz;
        ",
        ).build();

    foo.cargo("test -v")
        .masquerade_as_nightly_cargo()
        .with_stderr_contains(
            "\
[DOCTEST] foo
[RUNNING] `rustdoc --test [CWD]/src/lib.rs \
        [..] \
        --extern baz=[CWD]/target/debug/deps/libbar-[..].rlib \
        --extern bar=[CWD]/target/debug/deps/libbar-[..].rlib \
        [..]`
",
        ).run();
}

#[test]
fn features_still_work() {
    Package::new("foo", "0.1.0").publish();
    Package::new("bar", "0.1.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "test"
                version = "0.1.0"
                authors = []

                [dependencies]
                p1 = { path = 'a', features = ['b'] }
                p2 = { path = 'b' }
            "#,
        ).file("src/lib.rs", "")
        .file(
            "a/Cargo.toml",
            r#"
                cargo-features = ["rename-dependency"]

                [package]
                name = "p1"
                version = "0.1.0"
                authors = []

                [dependencies]
                b = { version = "0.1", package = "foo", optional = true }
            "#,
        ).file("a/src/lib.rs", "extern crate b;")
        .file(
            "b/Cargo.toml",
            r#"
                cargo-features = ["rename-dependency"]

                [package]
                name = "p2"
                version = "0.1.0"
                authors = []

                [dependencies]
                b = { version = "0.1", package = "bar", optional = true }

                [features]
                default = ['b']
            "#,
        ).file("b/src/lib.rs", "extern crate b;")
        .build();

    p.cargo("build -v").masquerade_as_nightly_cargo().run();
}

#[test]
fn features_not_working() {
    Package::new("foo", "0.1.0").publish();
    Package::new("bar", "0.1.0").publish();

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
                a = { path = 'a', package = 'p1', optional = true }

                [features]
                default = ['p1']
            "#,
        ).file("src/lib.rs", "")
        .file("a/Cargo.toml", &basic_manifest("p1", "0.1.0"))
        .build();

    p.cargo("build -v")
        .masquerade_as_nightly_cargo()
        .with_status(101)
        .with_stderr(
            "\
error: failed to parse manifest at `[..]`

Caused by:
  Feature `default` includes `p1` which is neither a dependency nor another feature
",
        ).run();
}

#[test]
fn rename_with_dash() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                cargo-features = ["rename-dependency"]
                [package]
                name = "qwerty"
                version = "0.1.0"

                [dependencies]
                foo-bar = { path = 'a', package = 'a' }
            "#,
        )
        .file("src/lib.rs", "extern crate foo_bar;")
        .file("a/Cargo.toml", &basic_manifest("a", "0.1.0"))
        .file("a/src/lib.rs", "")
        .build();

    p.cargo("build")
        .masquerade_as_nightly_cargo()
        .run();
}
