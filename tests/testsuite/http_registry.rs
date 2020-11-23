//! Tests for HTTP registry sources.

// Many of these tests are copied from registry.rs.
// It'd be nice if we could share them instead.
// Also, there are many tests in registry.rs that aren't specific to registry.
// It'd be nice if those were in their own module.

use cargo_test_support::paths::{self, CargoPathExt};
use cargo_test_support::registry::{
    registry_path, serve_registry, Dependency, Package, RegistryServer, RegistryServerConfiguration,
};
use cargo_test_support::t;
use cargo_test_support::{basic_manifest, project};
use std::fs;
use std::path::Path;

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

macro_rules! test_w_wo_changelog {
    ($name:ident) => {
        mod $name {
            use super::{$name, RegistryServerConfiguration};

            #[cargo_test]
            fn no_changelog() {
                $name(RegistryServerConfiguration::NoChangelog);
            }

            #[cargo_test]
            fn changelog() {
                $name(RegistryServerConfiguration::WithChangelog);
            }
        }
    };
}

test_w_wo_changelog!(simple);
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

test_w_wo_changelog!(deps);
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

test_w_wo_changelog!(nonexistent);
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

test_w_wo_changelog!(update_registry);
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

test_w_wo_changelog!(invalidate_index_on_rollover);
fn invalidate_index_on_rollover(config: RegistryServerConfiguration) {
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

    // Fish out the path to the .last-updated file
    let last_updated = if !matches!(config, RegistryServerConfiguration::NoChangelog) {
        let dir = fs::read_dir(paths::home().join(".cargo/registry/index/"))
            .unwrap()
            .last()
            .unwrap()
            .unwrap();

        Some(dir.path().join(".last-updated"))
    } else {
        None
    };

    if let Some(last_updated) = &last_updated {
        // Check the contents of the last-updated file to see that it's on epoch 1.
        assert_eq!(
            fs::read_to_string(last_updated).unwrap(),
            format!("1.{}", "1 YYYY-MM-DD HH:MM:SS a\n".len()),
            "{}",
            last_updated.display()
        );
    }

    // Next, publish a new version and make the changelog roll over
    Package::new("a", "0.1.1").publish();
    assert!(registry_path().join("changelog").exists(),);
    fs::write(
        registry_path().join("changelog"),
        b"2 2020-11-23 09:45:09 a\n",
    )
    .unwrap();

    // Now, try to build a project that relies on the newly published version.
    // It should realize it's not in cache, and update the registry.
    // The registry should detect the rollover, invalidate the cache,
    // and then succeed in fetching 0.1.1.
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

    // NOTE: we see UPDATING even when the changelog isn't used even though it is a no-op since
    // update_index is called whenever a version is not in the index cache.
    p2.cargo("build")
        .with_stderr(format!(
            "\
[UPDATING] [..]
[DOWNLOADING] crates ...
[DOWNLOADED] a v0.1.1 (http registry `{reg}`)
[COMPILING] a v0.1.1
[COMPILING] foo v0.5.0 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
",
            reg = url
        ))
        .run();

    if let Some(last_updated) = &last_updated {
        // Check the contents of the last-updated file to see that it picked up the new epoch.
        assert_eq!(
            fs::read_to_string(last_updated).unwrap(),
            format!("2.{}", "1 YYYY-MM-DD HH:MM:SS a\n".len()),
        );
    }

    // Next, publish a new version and make the changelog empty (which is also a rollover)
    Package::new("a", "0.1.2").publish();
    assert!(registry_path().join("changelog").exists(),);
    fs::write(registry_path().join("changelog"), b"").unwrap();

    // And again, build a project that depends on the new version.
    // It should realize it's not in cache, and update the registry,
    // which should again detect the rollover, invalidate the cache,
    // and then succeed in fetching 0.1.2.
    let p3 = project()
        .at("foo3")
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.5.0"
                authors = []

                [dependencies]
                a = "0.1.2"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    // NOTE: again, we see UPDATING even when the changelog isn't used even though it is a no-op
    // since update_index is called whenever a version is not in the index cache.
    p3.cargo("build")
        .with_stderr(format!(
            "\
[UPDATING] [..]
[DOWNLOADING] crates ...
[DOWNLOADED] a v0.1.2 (http registry `{reg}`)
[COMPILING] a v0.1.2
[COMPILING] foo v0.5.0 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
",
            reg = url
        ))
        .run();

    if let Some(last_updated) = &last_updated {
        // Check the contents of the last-updated file to see that it picked up the new epoch.
        assert_eq!(fs::read_to_string(last_updated).unwrap(), "unsupported");
    }
}

test_w_wo_changelog!(update_publish_then_update);
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

test_w_wo_changelog!(update_multiple_packages);
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

test_w_wo_changelog!(bundled_crate_in_registry);
fn bundled_crate_in_registry(config: RegistryServerConfiguration) {
    let _server = setup(config);
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.5.0"
                authors = []

                [dependencies]
                bar = "0.1"
                baz = "0.1"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("bar", "0.1.0").publish();
    Package::new("baz", "0.1.0")
        .dep("bar", "0.1.0")
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "baz"
                version = "0.1.0"
                authors = []

                [dependencies]
                bar = { path = "bar", version = "0.1.0" }
            "#,
        )
        .file("src/lib.rs", "")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "")
        .publish();

    p.cargo("run").run();
}

test_w_wo_changelog!(update_same_prefix_oh_my_how_was_this_a_bug);
fn update_same_prefix_oh_my_how_was_this_a_bug(config: RegistryServerConfiguration) {
    let _server = setup(config);
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "ugh"
                version = "0.5.0"
                authors = []

                [dependencies]
                foo = "0.1"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("foobar", "0.2.0").publish();
    Package::new("foo", "0.1.0")
        .dep("foobar", "0.2.0")
        .publish();

    p.cargo("generate-lockfile").run();
    p.cargo("update -pfoobar --precise=0.2.0").run();
}

test_w_wo_changelog!(use_semver);
fn use_semver(config: RegistryServerConfiguration) {
    let _server = setup(config);
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "bar"
                version = "0.5.0"
                authors = []

                [dependencies]
                foo = "1.2.3-alpha.0"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("foo", "1.2.3-alpha.0").publish();

    p.cargo("build").run();
}

test_w_wo_changelog!(use_semver_package_incorrectly);
fn use_semver_package_incorrectly(config: RegistryServerConfiguration) {
    let _server = setup(config);
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["a", "b"]
            "#,
        )
        .file(
            "a/Cargo.toml",
            r#"
            [project]
            name = "a"
            version = "0.1.1-alpha.0"
            authors = []
            "#,
        )
        .file(
            "b/Cargo.toml",
            r#"
            [project]
            name = "b"
            version = "0.1.0"
            authors = []

            [dependencies]
            a = { version = "^0.1", path = "../a" }
            "#,
        )
        .file("a/src/main.rs", "fn main() {}")
        .file("b/src/main.rs", "fn main() {}")
        .build();

    p.cargo("build")
        .with_status(101)
        .with_stderr(
            "\
error: no matching package named `a` found
location searched: [..]
prerelease package needs to be specified explicitly
a = { version = \"0.1.1-alpha.0\" }
required by package `b v0.1.0 ([..])`
",
        )
        .run();
}

test_w_wo_changelog!(only_download_relevant);
fn only_download_relevant(config: RegistryServerConfiguration) {
    let _server = setup(config);
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "bar"
                version = "0.5.0"
                authors = []

                [target.foo.dependencies]
                foo = "*"
                [dev-dependencies]
                bar = "*"
                [dependencies]
                baz = "*"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("foo", "0.1.0").publish();
    Package::new("bar", "0.1.0").publish();
    Package::new("baz", "0.1.0").publish();

    p.cargo("build")
        .with_stderr(
            "\
[UPDATING] `[..]` index
[DOWNLOADING] crates ...
[DOWNLOADED] baz v0.1.0 ([..])
[COMPILING] baz v0.1.0
[COMPILING] bar v0.5.0 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
",
        )
        .run();
}

test_w_wo_changelog!(resolve_and_backtracking);
fn resolve_and_backtracking(config: RegistryServerConfiguration) {
    let _server = setup(config);
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "bar"
                version = "0.5.0"
                authors = []

                [dependencies]
                foo = "*"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("foo", "0.1.1")
        .feature_dep("bar", "0.1", &["a", "b"])
        .publish();
    Package::new("foo", "0.1.0").publish();

    p.cargo("build").run();
}

test_w_wo_changelog!(disallow_network);
fn disallow_network(config: RegistryServerConfiguration) {
    let _server = setup(config);
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "bar"
                version = "0.5.0"
                authors = []

                [dependencies]
                foo = "*"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    // TODO: this should also check that we don't access the network for things we have in cache.
    p.cargo("build --frozen")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] failed to get `foo` as a dependency of package `bar v0.5.0 [..]`

Caused by:
  failed to load source for dependency `foo`

Caused by:
  Unable to update registry [..]

Caused by:
  failed to update replaced source registry `https://github.com/rust-lang/crates.io-index`

Caused by:
  attempting to make an HTTP request, but --frozen was specified
",
        )
        .run();
}

test_w_wo_changelog!(add_dep_dont_update_registry);
fn add_dep_dont_update_registry(config: RegistryServerConfiguration) {
    let _server = setup(config);
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "bar"
                version = "0.5.0"
                authors = []

                [dependencies]
                baz = { path = "baz" }
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "baz/Cargo.toml",
            r#"
                [project]
                name = "baz"
                version = "0.5.0"
                authors = []

                [dependencies]
                remote = "0.3"
            "#,
        )
        .file("baz/src/lib.rs", "")
        .build();

    Package::new("remote", "0.3.4").publish();

    p.cargo("build").run();

    p.change_file(
        "Cargo.toml",
        r#"
        [project]
        name = "bar"
        version = "0.5.0"
        authors = []

        [dependencies]
        baz = { path = "baz" }
        remote = "0.3"
        "#,
    );

    p.cargo("build")
        .with_stderr(
            "\
[COMPILING] bar v0.5.0 ([..])
[FINISHED] [..]
",
        )
        .run();
}

test_w_wo_changelog!(bump_version_dont_update_registry);
fn bump_version_dont_update_registry(config: RegistryServerConfiguration) {
    let _server = setup(config);
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "bar"
                version = "0.5.0"
                authors = []

                [dependencies]
                baz = { path = "baz" }
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "baz/Cargo.toml",
            r#"
                [project]
                name = "baz"
                version = "0.5.0"
                authors = []

                [dependencies]
                remote = "0.3"
            "#,
        )
        .file("baz/src/lib.rs", "")
        .build();

    Package::new("remote", "0.3.4").publish();

    p.cargo("build").run();

    p.change_file(
        "Cargo.toml",
        r#"
        [project]
        name = "bar"
        version = "0.6.0"
        authors = []

        [dependencies]
        baz = { path = "baz" }
        "#,
    );

    p.cargo("build")
        .with_stderr(
            "\
[COMPILING] bar v0.6.0 ([..])
[FINISHED] [..]
",
        )
        .run();
}

test_w_wo_changelog!(toml_lies_but_index_is_truth);
fn toml_lies_but_index_is_truth(config: RegistryServerConfiguration) {
    let _server = setup(config);
    Package::new("foo", "0.2.0").publish();
    Package::new("bar", "0.3.0")
        .dep("foo", "0.2.0")
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "bar"
                version = "0.3.0"
                authors = []

                [dependencies]
                foo = "0.1.0"
            "#,
        )
        .file("src/lib.rs", "extern crate foo;")
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "bar"
                version = "0.5.0"
                authors = []

                [dependencies]
                bar = "0.3"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("build -v").run();
}

test_w_wo_changelog!(rename_deps_and_features);
fn rename_deps_and_features(config: RegistryServerConfiguration) {
    let _server = setup(config);
    Package::new("foo", "0.1.0")
        .file("src/lib.rs", "pub fn f1() {}")
        .publish();
    Package::new("foo", "0.2.0")
        .file("src/lib.rs", "pub fn f2() {}")
        .publish();
    Package::new("bar", "0.2.0")
        .add_dep(
            Dependency::new("foo01", "0.1.0")
                .package("foo")
                .optional(true),
        )
        .add_dep(Dependency::new("foo02", "0.2.0").package("foo"))
        .feature("another", &["foo01"])
        .file(
            "src/lib.rs",
            r#"
                extern crate foo02;
                #[cfg(feature = "foo01")]
                extern crate foo01;

                pub fn foo() {
                    foo02::f2();
                    #[cfg(feature = "foo01")]
                    foo01::f1();
                }
            "#,
        )
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "a"
                version = "0.5.0"
                authors = []

                [dependencies]
                bar = "0.2"
            "#,
        )
        .file(
            "src/main.rs",
            "
                extern crate bar;
                fn main() { bar::foo(); }
            ",
        )
        .build();

    p.cargo("build").run();
    p.cargo("build --features bar/foo01").run();
    p.cargo("build --features bar/another").run();
}

test_w_wo_changelog!(ignore_invalid_json_lines);
fn ignore_invalid_json_lines(config: RegistryServerConfiguration) {
    let _server = setup(config);
    Package::new("foo", "0.1.0").publish();
    Package::new("foo", "0.1.1").invalid_json(true).publish();
    Package::new("foo", "0.2.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "a"
                version = "0.5.0"
                authors = []

                [dependencies]
                foo = '0.1.0'
                foo02 = { version = '0.2.0', package = 'foo' }
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("build").run();
}

test_w_wo_changelog!(readonly_registry_still_works);
fn readonly_registry_still_works(config: RegistryServerConfiguration) {
    let _server = setup(config);
    Package::new("foo", "0.1.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "a"
                version = "0.5.0"
                authors = []

                [dependencies]
                foo = '0.1.0'
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("generate-lockfile").run();
    p.cargo("fetch --locked").run();
    chmod_readonly(&paths::home(), true);
    p.cargo("build").run();
    // make sure we un-readonly the files afterwards so "cargo clean" can remove them (#6934)
    chmod_readonly(&paths::home(), false);

    fn chmod_readonly(path: &Path, readonly: bool) {
        for entry in t!(path.read_dir()) {
            let entry = t!(entry);
            let path = entry.path();
            if t!(entry.file_type()).is_dir() {
                chmod_readonly(&path, readonly);
            } else {
                set_readonly(&path, readonly);
            }
        }
        set_readonly(path, readonly);
    }

    fn set_readonly(path: &Path, readonly: bool) {
        let mut perms = t!(path.metadata()).permissions();
        perms.set_readonly(readonly);
        t!(fs::set_permissions(path, perms));
    }
}
