//! Tests for the warnings issued when packages are available in multiple
//! registries without being disambiguated.

use cargo_test_support::{
    basic_manifest, git, project,
    registry::{self, Package},
};

#[cargo_test]
fn same_version_in_two_registries() {
    // The basic test: what happens if a package is available in two registries?
    // Note that both registries have to actually be used in the dependencies
    // for the warning to fire.

    registry::alt_init();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies]
                bar = "0.1.2"
                in-alternative = { version = "0.1.0", registry = "alternative" }
                in-default = "0.1.1"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    Package::new("in-alternative", "0.1.0")
        .alternative(true)
        .publish();
    Package::new("in-default", "0.1.1").publish();
    Package::new("bar", "0.1.2").alternative(true).publish();
    Package::new("bar", "0.1.2").publish();

    // Note one small piece of weirdness here: if a registry isn't defined in
    // the dependency, the default is `crates-io` even if `crates-io` has been
    // replaced by another source.
    p.cargo("check")
        .with_stderr_unordered(
            "\
[UPDATING] `dummy-registry` index
[UPDATING] `alternative` index
[DOWNLOADING] crates ...
[DOWNLOADED] in-default v0.1.1 (registry `dummy-registry`)
[DOWNLOADED] in-alternative v0.1.0 (registry `alternative`)
[DOWNLOADED] bar v0.1.2 (registry `dummy-registry`)
[WARNING] package `bar v0.1.2` from registry `crates-io` is also defined in registry `alternative`
[NOTE] To handle this warning, specify the exact registry in use for the
`bar v0.1.2` dependency in Cargo.toml, eg:

bar = { version = \"0.1.2\", registry = \"crates-io\" }

[CHECKING] bar v0.1.2
[CHECKING] in-alternative v0.1.0 (registry `alternative`)
[CHECKING] in-default v0.1.1
[CHECKING] foo v0.0.1 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
",
        )
        .run();
}

#[cargo_test]
fn different_versions_in_two_registries() {
    // Checks should take place on package names only, not versions.

    registry::alt_init();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies]
                bar = "0.1.2"
                in-alternative = { version = "0.1.0", registry = "alternative" }
                in-default = "0.1.1"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    Package::new("in-alternative", "0.1.0")
        .alternative(true)
        .publish();
    Package::new("in-default", "0.1.1").publish();
    Package::new("bar", "1.2.3").alternative(true).publish();
    Package::new("bar", "0.1.2").publish();

    p.cargo("check")
        .with_stderr_unordered(
            "\
[UPDATING] `dummy-registry` index
[UPDATING] `alternative` index
[DOWNLOADING] crates ...
[DOWNLOADED] in-default v0.1.1 (registry `dummy-registry`)
[DOWNLOADED] in-alternative v0.1.0 (registry `alternative`)
[DOWNLOADED] bar v0.1.2 (registry `dummy-registry`)
[WARNING] package `bar v0.1.2` from registry `crates-io` is also defined in registry `alternative`
[NOTE] To handle this warning, specify the exact registry in use for the
`bar v0.1.2` dependency in Cargo.toml, eg:

bar = { version = \"0.1.2\", registry = \"crates-io\" }

[CHECKING] bar v0.1.2
[CHECKING] in-alternative v0.1.0 (registry `alternative`)
[CHECKING] in-default v0.1.1
[CHECKING] foo v0.0.1 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
",
        )
        .run();
}

#[cargo_test]
fn explicit_dep_registry() {
    // Dependencies with explicit registries should not generate warnings.

    registry::alt_init();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies]
                bar = { version = "0.1.2", registry = "alternative" }
                in-alternative = { version = "0.1.0", registry = "alternative" }
                in-default = "0.1.1"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    Package::new("in-alternative", "0.1.0")
        .alternative(true)
        .publish();
    Package::new("in-default", "0.1.1").publish();
    Package::new("bar", "0.1.2").alternative(true).publish();
    Package::new("bar", "0.1.2").publish();

    p.cargo("check")
        .with_stderr_unordered(
            "\
[UPDATING] `dummy-registry` index
[UPDATING] `alternative` index
[DOWNLOADING] crates ...
[DOWNLOADED] in-default v0.1.1 (registry `dummy-registry`)
[DOWNLOADED] in-alternative v0.1.0 (registry `alternative`)
[DOWNLOADED] bar v0.1.2 (registry `alternative`)
[CHECKING] bar v0.1.2 (registry `alternative`)
[CHECKING] in-alternative v0.1.0 (registry `alternative`)
[CHECKING] in-default v0.1.1
[CHECKING] foo v0.0.1 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
",
        )
        .run();
}

#[cargo_test]
fn path_dep() {
    // Dependencies defined as path deps should never warn.

    registry::alt_init();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies]
                bar = { path = "bar" }
                in-alternative = { version = "0.1.0", registry = "alternative" }
                in-default = "0.1.1"
            "#,
        )
        .file("src/lib.rs", "")
        .file("bar/Cargo.toml", &basic_manifest("bar", "1.1.2"))
        .file("bar/src/lib.rs", "")
        .build();

    Package::new("in-alternative", "0.1.0")
        .alternative(true)
        .publish();
    Package::new("in-default", "0.1.1").publish();
    Package::new("bar", "0.1.2").alternative(true).publish();
    Package::new("bar", "0.1.2").publish();

    p.cargo("check")
        .with_stderr_unordered(
            "\
[UPDATING] `dummy-registry` index
[UPDATING] `alternative` index
[DOWNLOADING] crates ...
[DOWNLOADED] in-default v0.1.1 (registry `dummy-registry`)
[DOWNLOADED] in-alternative v0.1.0 (registry `alternative`)
[CHECKING] bar v1.1.2 ([CWD]/bar)
[CHECKING] in-alternative v0.1.0 (registry `alternative`)
[CHECKING] in-default v0.1.1
[CHECKING] foo v0.0.1 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
",
        )
        .run();
}

#[cargo_test]
fn git_dep() {
    // Dependencies defined as Git deps should never warn.

    let (dep_project, _dep_repo) = git::new_repo("bar", |p| {
        p.file("Cargo.toml", &basic_manifest("bar", "1.1.2"))
            .file("src/lib.rs", "")
    });

    registry::alt_init();
    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"
                    version = "0.0.1"
                    authors = []

                    [dependencies]
                    bar = {{ git = "{}" }}
                    in-alternative = {{ version = "0.1.0", registry = "alternative" }}
                    in-default = "0.1.1"
                "#,
                dep_project.url()
            ),
        )
        .file("src/lib.rs", "")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.2"))
        .file("bar/src/lib.rs", "")
        .build();

    Package::new("in-alternative", "0.1.0")
        .alternative(true)
        .publish();
    Package::new("in-default", "0.1.1").publish();
    Package::new("bar", "0.1.2").alternative(true).publish();
    Package::new("bar", "0.1.2").publish();

    p.cargo("check")
        .with_stderr_unordered(
            "\
[UPDATING] `dummy-registry` index
[UPDATING] `alternative` index
[DOWNLOADING] crates ...
[DOWNLOADED] in-default v0.1.1 (registry `dummy-registry`)
[DOWNLOADED] in-alternative v0.1.0 (registry `alternative`)
[UPDATING] git repository `[..]/bar`
[CHECKING] bar v1.1.2 ([..]/bar#[..])
[CHECKING] in-alternative v0.1.0 (registry `alternative`)
[CHECKING] in-default v0.1.1
[CHECKING] foo v0.0.1 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
",
        )
        .run();
}
