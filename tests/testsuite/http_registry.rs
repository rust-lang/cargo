//! Tests for HTTP registry sources.

use cargo_test_support::paths;
use cargo_test_support::registry::{
    registry_path, serve_registry, Package, RegistryServer, RegistryServerConfiguration,
};
use cargo_test_support::{project, t};
use std::fs;

fn setup(config: RegistryServerConfiguration) -> RegistryServer {
    let server = serve_registry(registry_path(), config);

    let root = paths::root();
    t!(fs::create_dir(&root.join(".cargo")));
    t!(fs::write(
        root.join(".cargo/config"),
        format!(
            "
            [source.crates-io]
            registry = 'https://wut'
            replace-with = 'my-awesome-http-registry'

            [source.my-awesome-http-registry]
            registry = 'rfc+http://{}'
        ",
            server.addr()
        )
    ));

    server
}

fn simple(config: RegistryServerConfiguration) {
    let server = setup(config);
    let url = format!("http://{}/", server.addr());
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies]
                bar = ">= 0.0.0"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("bar", "0.0.1").publish();

    p.cargo("build")
        .with_stderr(&format!(
            "\
[UPDATING] `{reg}` index
[DOWNLOADING] crates ...
[DOWNLOADED] bar v0.0.1 (http registry `{reg}`)
[COMPILING] bar v0.0.1
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
",
            reg = url
        ))
        .run();

    p.cargo("clean").run();

    // Don't download a second time
    p.cargo("build")
        .with_stderr(
            "\
[COMPILING] bar v0.0.1
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
",
        )
        .run();
}

#[cargo_test]
fn no_changelog_simple() {
    simple(RegistryServerConfiguration::NoChangelog);
}

#[cargo_test]
fn changelog_simple() {
    simple(RegistryServerConfiguration::WithChangelog);
}

fn deps(config: RegistryServerConfiguration) {
    let server = setup(config);
    let url = format!("http://{}/", server.addr());
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies]
                bar = ">= 0.0.0"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("baz", "0.0.1").publish();
    Package::new("bar", "0.0.1").dep("baz", "*").publish();

    p.cargo("build")
        .with_stderr(&format!(
            "\
[UPDATING] `{reg}` index
[DOWNLOADING] crates ...
[DOWNLOADED] [..] v0.0.1 (http registry `{reg}`)
[DOWNLOADED] [..] v0.0.1 (http registry `{reg}`)
[COMPILING] baz v0.0.1
[COMPILING] bar v0.0.1
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
",
            reg = url
        ))
        .run();
}

#[cargo_test]
fn no_changelog_deps() {
    deps(RegistryServerConfiguration::NoChangelog);
}

#[cargo_test]
fn changelog_deps() {
    deps(RegistryServerConfiguration::WithChangelog);
}

fn nonexistent(config: RegistryServerConfiguration) {
    let _server = setup(config);
    Package::new("init", "0.0.1").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies]
                nonexistent = ">= 0.0.0"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("build")
        .with_status(101)
        .with_stderr(
            "\
[UPDATING] [..] index
error: no matching package named `nonexistent` found
location searched: registry [..]
required by package `foo v0.0.1 ([..])`
",
        )
        .run();
}

#[cargo_test]
fn no_changelog_nonexistent() {
    nonexistent(RegistryServerConfiguration::NoChangelog);
}

#[cargo_test]
fn changelog_nonexistent() {
    nonexistent(RegistryServerConfiguration::WithChangelog);
}

fn update_registry(config: RegistryServerConfiguration) {
    let server = setup(config);
    let url = format!("http://{}/", server.addr());
    Package::new("init", "0.0.1").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies]
                notyet = ">= 0.0.0"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("build")
        .with_status(101)
        .with_stderr_contains(
            "\
error: no matching package named `notyet` found
location searched: registry `[..]`
required by package `foo v0.0.1 ([..])`
",
        )
        .run();

    Package::new("notyet", "0.0.1").publish();

    p.cargo("build")
        .with_stderr(format!(
            "\
[UPDATING] `{reg}` index
[DOWNLOADING] crates ...
[DOWNLOADED] notyet v0.0.1 (http registry `{reg}`)
[COMPILING] notyet v0.0.1
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
",
            reg = url
        ))
        .run();
}

#[cargo_test]
fn no_changelog_update_registry() {
    update_registry(RegistryServerConfiguration::NoChangelog);
}

#[cargo_test]
fn changelog_update_registry() {
    update_registry(RegistryServerConfiguration::WithChangelog);
}
