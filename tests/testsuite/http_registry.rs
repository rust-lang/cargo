//! Tests for HTTP registry sources.

// Many of these tests are copied from registry.rs.
// It'd be nice if we could share them instead.
// Also, there are many tests in registry.rs that aren't specific to registry.
// It'd be nice if those were in their own module.

use cargo_test_support::paths::{self, CargoPathExt};
use cargo_test_support::registry::{
    registry_path, serve_registry, Dependency, Package, RegistryServer,
};
use cargo_test_support::t;
use cargo_test_support::{basic_manifest, project};
use std::fs;
use std::path::Path;

fn cargo(p: &cargo_test_support::Project, s: &str) -> cargo_test_support::Execs {
    let mut e = p.cargo(s);
    e.arg("-Zhttp-registry")
        .env("__CARGO_TEST_CHANNEL_OVERRIDE_DO_NOT_USE_THIS", "nightly");
    e
}

fn setup() -> RegistryServer {
    let server = serve_registry(registry_path());

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
            registry = 'sparse+http://{}'
        ",
            server.addr()
        )
    ));

    server
}

#[cargo_test]
fn simple() {
    let server = setup();
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

    cargo(&p, "build")
        .with_stderr(&format!(
            "\
[UPDATING] `{reg}` index
[PREFETCHING] index files ...
[DOWNLOADING] crates ...
[DOWNLOADED] bar v0.0.1 (http registry `{reg}`)
[COMPILING] bar v0.0.1
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
",
            reg = url
        ))
        .run();

    cargo(&p, "clean").run();

    // Don't download a second time
    cargo(&p, "build")
        .with_stderr(
            "\
[PREFETCHING] index files ...
[COMPILING] bar v0.0.1
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
",
        )
        .run();
}

#[cargo_test]
fn deps() {
    let server = setup();
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

    cargo(&p, "build")
        .with_stderr(&format!(
            "\
[UPDATING] `{reg}` index
[PREFETCHING] index files ...
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
fn nonexistent() {
    let _server = setup();
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

    cargo(&p, "build")
        .with_status(101)
        .with_stderr(
            "\
[UPDATING] [..] index
[PREFETCHING] index files ...
error: no matching package named `nonexistent` found
location searched: registry [..]
required by package `foo v0.0.1 ([..])`
",
        )
        .run();
}

#[cargo_test]
fn update_registry() {
    let server = setup();
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

    cargo(&p, "build")
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

    cargo(&p, "build")
        .with_stderr(format!(
            "\
[UPDATING] `{reg}` index
[PREFETCHING] index files ...
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
fn update_publish_then_update() {
    let server = setup();
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
    cargo(&p, "build").run();

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
    cargo(&p2, "build").run();
    registry.rm_rf();
    t!(fs::rename(&backup, &registry));
    t!(fs::rename(
        p2.root().join("Cargo.lock"),
        p.root().join("Cargo.lock")
    ));

    // Finally, build the first project again (with our newer Cargo.lock) which
    // should download the new index file from the registry, download the new crate, and
    // then build everything again.
    cargo(&p, "build")
        .with_stderr(format!(
            "\
[PREFETCHING] index files ...
[DOWNLOADING] crates ...
[DOWNLOADED] a v0.1.1 (http registry `{reg}`)
[COMPILING] a v0.1.1
[COMPILING] foo v0.5.0 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
",
            reg = url
        ))
        .run();
}

#[cargo_test]
fn update_multiple_packages() {
    let server = setup();
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

    cargo(&p, "fetch").run();

    Package::new("a", "0.1.1").publish();
    Package::new("b", "0.1.1").publish();
    Package::new("c", "0.1.1").publish();

    cargo(&p, "update -pa -pb")
        .with_stderr(
            "\
[UPDATING] `[..]` index
[PREFETCHING] index files ...
[UPDATING] a v0.1.0 -> v0.1.1
[UPDATING] b v0.1.0 -> v0.1.1
",
        )
        .run();

    cargo(&p, "update -pb -pc")
        .with_stderr(
            "\
[UPDATING] `[..]` index
[PREFETCHING] index files ...
[UPDATING] c v0.1.0 -> v0.1.1
",
        )
        .run();

    cargo(&p, "build")
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
fn bundled_crate_in_registry() {
    let _server = setup();
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

    cargo(&p, "run").run();
}

#[cargo_test]
fn update_same_prefix_oh_my_how_was_this_a_bug() {
    let _server = setup();
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

    cargo(&p, "generate-lockfile").run();
    cargo(&p, "update -pfoobar --precise=0.2.0").run();
}

#[cargo_test]
fn use_semver() {
    let _server = setup();
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

    cargo(&p, "build").run();
}

#[cargo_test]
fn use_semver_package_incorrectly() {
    let _server = setup();
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

    cargo(&p, "build")
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

#[cargo_test]
fn only_download_relevant() {
    let _server = setup();
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

    cargo(&p, "build")
        .with_stderr(
            "\
[UPDATING] `[..]` index
[PREFETCHING] index files ...
[DOWNLOADING] crates ...
[DOWNLOADED] baz v0.1.0 ([..])
[COMPILING] baz v0.1.0
[COMPILING] bar v0.5.0 ([..])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
",
        )
        .run();
}

#[cargo_test]
fn resolve_and_backtracking() {
    let _server = setup();
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

    cargo(&p, "build").run();
}

#[cargo_test]
fn disallow_network() {
    let _server = setup();
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
    cargo(&p, "build --frozen")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] failed to prefetch dependencies

Caused by:
  failed to load source for dependency `foo`

Caused by:
  Unable to update registry [..]

Caused by:
  failed to update replaced source registry `https://github.com/rust-lang/crates.io-index`

Caused by:
  attempting to update a http repository, but --frozen was specified
",
        )
        .run();
}

#[cargo_test]
fn add_dep_dont_update_registry() {
    let _server = setup();
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

    cargo(&p, "build").run();

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

    cargo(&p, "build")
        .with_stderr(
            "\
[PREFETCHING] index files ...
[COMPILING] bar v0.5.0 ([..])
[FINISHED] [..]
",
        )
        .run();
}

#[cargo_test]
fn bump_version_dont_update_registry() {
    let _server = setup();
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

    cargo(&p, "build").run();

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

    cargo(&p, "build")
        .with_stderr(
            "\
[COMPILING] bar v0.6.0 ([..])
[FINISHED] [..]
",
        )
        .run();
}

#[cargo_test]
fn toml_lies_but_index_is_truth() {
    let _server = setup();
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

    cargo(&p, "build -v").run();
}

#[cargo_test]
fn rename_deps_and_features() {
    let _server = setup();
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

    cargo(&p, "build").run();
    cargo(&p, "build --features bar/foo01").run();
    cargo(&p, "build --features bar/another").run();
}

#[cargo_test]
fn ignore_invalid_json_lines() {
    let _server = setup();
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

    cargo(&p, "build").run();
}

#[cargo_test]
fn readonly_registry_still_works() {
    let _server = setup();
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

    cargo(&p, "generate-lockfile").run();
    cargo(&p, "fetch --locked").run();
    chmod_readonly(&paths::home(), true);
    cargo(&p, "build").run();
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
