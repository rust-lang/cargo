//! Tests for HTTP registry sources.

use cargo_test_support::paths::{self, CargoPathExt};
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

fn update_publish_then_update(config: RegistryServerConfiguration) {
    let server = setup(config);
    let url = format!("http://{}/", server.addr());

    // First generate a Cargo.lock and a clone of the registry index at the
    // "head" of the current registry.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.5.0"
                authors = []

                [dependencies]
                a = "0.1.0"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();
    Package::new("a", "0.1.0").publish();
    p.cargo("build").run();

    // Next, publish a new package and back up the copy of the registry we just
    // created.
    Package::new("a", "0.1.1").publish();
    let registry = paths::home().join(".cargo/registry");
    let backup = paths::root().join("registry-backup");
    t!(fs::rename(&registry, &backup));

    // Generate a Cargo.lock with the newer version, and then move the old copy
    // of the registry back into place.
    let p2 = project()
        .at("foo2")
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.5.0"
                authors = []

                [dependencies]
                a = "0.1.1"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();
    p2.cargo("build").run();
    registry.rm_rf();
    t!(fs::rename(&backup, &registry));
    t!(fs::rename(
        p2.root().join("Cargo.lock"),
        p.root().join("Cargo.lock")
    ));

    // Finally, build the first project again (with our newer Cargo.lock) which
    // should force an update of the old registry, download the new crate, and
    // then build everything again.
    //
    // However, if the server does not support a changelog, the index file will be double-checked
    // with the backend when it is loaded, and will be updated at that time. There is no index
    // update.
    let updating = if matches!(config, RegistryServerConfiguration::NoChangelog) {
        ""
    } else {
        "[UPDATING] [..]\n"
    };
    p.cargo("build")
        .with_stderr(format!(
            "{u}\
[DOWNLOADING] crates ...
[DOWNLOADED] a v0.1.1 (http registry `{reg}`)
[COMPILING] a v0.1.1
[COMPILING] foo v0.5.0 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
",
            u = updating,
            reg = url
        ))
        .run();
}

#[cargo_test]
fn no_changelog_update_publish_then_update() {
    update_publish_then_update(RegistryServerConfiguration::NoChangelog);
}

#[cargo_test]
fn changelog_update_publish_then_update() {
    update_publish_then_update(RegistryServerConfiguration::WithChangelog);
}

fn update_multiple_packages(config: RegistryServerConfiguration) {
    let server = setup(config);
    let url = format!("http://{}/", server.addr());
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.5.0"
                authors = []

                [dependencies]
                a = "*"
                b = "*"
                c = "*"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("a", "0.1.0").publish();
    Package::new("b", "0.1.0").publish();
    Package::new("c", "0.1.0").publish();

    p.cargo("fetch").run();

    Package::new("a", "0.1.1").publish();
    Package::new("b", "0.1.1").publish();
    Package::new("c", "0.1.1").publish();

    p.cargo("update -pa -pb")
        .with_stderr(
            "\
[UPDATING] `[..]` index
[UPDATING] a v0.1.0 -> v0.1.1
[UPDATING] b v0.1.0 -> v0.1.1
",
        )
        .run();

    p.cargo("update -pb -pc")
        .with_stderr(
            "\
[UPDATING] `[..]` index
[UPDATING] c v0.1.0 -> v0.1.1
",
        )
        .run();

    p.cargo("build")
        .with_stderr_contains(format!("[DOWNLOADED] a v0.1.1 (http registry `{}`)", url))
        .with_stderr_contains(format!("[DOWNLOADED] b v0.1.1 (http registry `{}`)", url))
        .with_stderr_contains(format!("[DOWNLOADED] c v0.1.1 (http registry `{}`)", url))
        .with_stderr_contains("[COMPILING] a v0.1.1")
        .with_stderr_contains("[COMPILING] b v0.1.1")
        .with_stderr_contains("[COMPILING] c v0.1.1")
        .with_stderr_contains("[COMPILING] foo v0.5.0 ([..])")
        .run();
}

#[cargo_test]
fn no_changelog_update_multiple_packages() {
    update_multiple_packages(RegistryServerConfiguration::NoChangelog);
}

#[cargo_test]
fn changelog_update_multiple_packages() {
    update_multiple_packages(RegistryServerConfiguration::WithChangelog);
}
