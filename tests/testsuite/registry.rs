//! Tests for normal registry dependencies.

use std::fmt::Write;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::Mutex;

use crate::prelude::*;
use crate::utils::cargo_process;
use cargo::core::SourceId;
use cargo_test_support::paths;
use cargo_test_support::registry::{
    self, Dependency, Package, RegistryBuilder, Response, TestRegistry, registry_path,
};
use cargo_test_support::{basic_manifest, project, str};
use cargo_test_support::{git, t};
use cargo_util::paths::remove_dir_all;

fn setup_http() -> TestRegistry {
    RegistryBuilder::new().http_index().build()
}

#[cargo_test]
fn test_server_stops() {
    let server = setup_http();
    server.join(); // ensure the server fully shuts down
}

#[cargo_test]
fn simple_http() {
    let _server = setup_http();
    simple(
        str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] bar v0.0.1 (registry `dummy-registry`)
[CHECKING] bar v0.0.1
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]],
        str![[r#"
[CHECKING] bar v0.0.1
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]],
    );
}

#[cargo_test]
fn simple_git() {
    simple(
        str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] bar v0.0.1 (registry `dummy-registry`)
[CHECKING] bar v0.0.1
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]],
        str![[r#"
[CHECKING] bar v0.0.1
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]],
    );
}

fn simple(pre_clean_expected: impl IntoData, post_clean_expected: impl IntoData) {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies]
                bar = ">= 0.0.0"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("bar", "0.0.1").publish();

    p.cargo("check").with_stderr_data(pre_clean_expected).run();

    p.cargo("clean").run();

    assert!(paths::home().join(".cargo/registry/CACHEDIR.TAG").is_file());

    // Don't download a second time
    p.cargo("check").with_stderr_data(post_clean_expected).run();
}

#[cargo_test]
fn deps_http() {
    let _server = setup_http();
    deps(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 2 packages to latest compatible versions
[DOWNLOADING] crates ...
[DOWNLOADED] ba[..] v0.0.1 (registry `dummy-registry`)
[DOWNLOADED] ba[..] v0.0.1 (registry `dummy-registry`)
[CHECKING] baz v0.0.1
[CHECKING] bar v0.0.1
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]);
}

#[cargo_test]
fn deps_git() {
    deps(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 2 packages to latest compatible versions
[DOWNLOADING] crates ...
[DOWNLOADED] ba[..] v0.0.1 (registry `dummy-registry`)
[DOWNLOADED] ba[..] v0.0.1 (registry `dummy-registry`)
[CHECKING] baz v0.0.1
[CHECKING] bar v0.0.1
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]);
}

fn deps(expected: impl IntoData) {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies]
                bar = ">= 0.0.0"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("baz", "0.0.1").publish();
    Package::new("bar", "0.0.1").dep("baz", "*").publish();

    p.cargo("check").with_stderr_data(expected).run();

    assert!(paths::home().join(".cargo/registry/CACHEDIR.TAG").is_file());
}

#[cargo_test]
fn nonexistent_http() {
    let _server = setup_http();
    nonexistent(str![[r#"
[UPDATING] `dummy-registry` index
[ERROR] no matching package named `nonexistent` found
location searched: `dummy-registry` index (which is replacing registry `crates-io`)
required by package `foo v0.0.1 ([ROOT]/foo)`

"#]]);
}

#[cargo_test]
fn nonexistent_git() {
    nonexistent(str![[r#"
[UPDATING] `dummy-registry` index
[ERROR] no matching package named `nonexistent` found
location searched: `dummy-registry` index (which is replacing registry `crates-io`)
required by package `foo v0.0.1 ([ROOT]/foo)`

"#]]);
}

fn nonexistent(expected: impl IntoData) {
    Package::new("init", "0.0.1").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies]
                nonexistent = ">= 0.0.0"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(expected)
        .run();
}

#[cargo_test]
fn wrong_case_http() {
    let _server = setup_http();
    wrong_case(str![[r#"
[UPDATING] `dummy-registry` index
[ERROR] no matching package found
searched package name: `Init`
perhaps you meant:      init
location searched: `dummy-registry` index (which is replacing registry `crates-io`)
required by package `foo v0.0.1 ([ROOT]/foo)`

"#]]);
}

#[cargo_test]
fn wrong_case_git() {
    wrong_case(str![[r#"
[UPDATING] `dummy-registry` index
[ERROR] no matching package found
searched package name: `Init`
perhaps you meant:      init
location searched: `dummy-registry` index (which is replacing registry `crates-io`)
required by package `foo v0.0.1 ([ROOT]/foo)`

"#]]);
}

fn wrong_case(expected: impl IntoData) {
    Package::new("init", "0.0.1").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies]
                Init = ">= 0.0.0"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    // #5678 to make this work
    p.cargo("check")
        .with_status(101)
        .with_stderr_data(expected)
        .run();
}

#[cargo_test]
fn mis_hyphenated_http() {
    let _server = setup_http();
    mis_hyphenated(str![[r#"
[UPDATING] `dummy-registry` index
[ERROR] no matching package found
searched package name: `mis_hyphenated`
perhaps you meant:      mis-hyphenated
location searched: `dummy-registry` index (which is replacing registry `crates-io`)
required by package `foo v0.0.1 ([ROOT]/foo)`

"#]]);
}

#[cargo_test]
fn mis_hyphenated_git() {
    mis_hyphenated(str![[r#"
[UPDATING] `dummy-registry` index
[ERROR] no matching package found
searched package name: `mis_hyphenated`
perhaps you meant:      mis-hyphenated
location searched: `dummy-registry` index (which is replacing registry `crates-io`)
required by package `foo v0.0.1 ([ROOT]/foo)`

"#]]);
}

fn mis_hyphenated(expected: impl IntoData) {
    Package::new("mis-hyphenated", "0.0.1").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies]
                mis_hyphenated = ">= 0.0.0"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    // #2775 to make this work
    p.cargo("check")
        .with_status(101)
        .with_stderr_data(expected)
        .run();
}

#[cargo_test]
fn wrong_version_http() {
    let _server = setup_http();
    wrong_version(
        str![[r#"
[UPDATING] `dummy-registry` index
[ERROR] failed to select a version for the requirement `foo = ">=1.0.0"`
candidate versions found which didn't match: 0.0.2, 0.0.1
location searched: `dummy-registry` index (which is replacing registry `crates-io`)
required by package `foo v0.0.1 ([ROOT]/foo)`
perhaps a crate was updated and forgotten to be re-vendored?

"#]],
        str![[r#"
[UPDATING] `dummy-registry` index
[ERROR] failed to select a version for the requirement `foo = ">=1.0.0"`
candidate versions found which didn't match: 0.0.4, 0.0.3, 0.0.2, ...
location searched: `dummy-registry` index (which is replacing registry `crates-io`)
required by package `foo v0.0.1 ([ROOT]/foo)`
perhaps a crate was updated and forgotten to be re-vendored?

"#]],
    );
}

#[cargo_test]
fn wrong_version_git() {
    wrong_version(
        str![[r#"
[UPDATING] `dummy-registry` index
[ERROR] failed to select a version for the requirement `foo = ">=1.0.0"`
candidate versions found which didn't match: 0.0.2, 0.0.1
location searched: `dummy-registry` index (which is replacing registry `crates-io`)
required by package `foo v0.0.1 ([ROOT]/foo)`
perhaps a crate was updated and forgotten to be re-vendored?

"#]],
        str![[r#"
[UPDATING] `dummy-registry` index
[ERROR] failed to select a version for the requirement `foo = ">=1.0.0"`
candidate versions found which didn't match: 0.0.4, 0.0.3, 0.0.2, ...
location searched: `dummy-registry` index (which is replacing registry `crates-io`)
required by package `foo v0.0.1 ([ROOT]/foo)`
perhaps a crate was updated and forgotten to be re-vendored?

"#]],
    );
}

fn wrong_version(pre_publish_expected: impl IntoData, post_publish_expected: impl IntoData) {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies]
                foo = ">= 1.0.0"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("foo", "0.0.1").publish();
    Package::new("foo", "0.0.2").publish();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(pre_publish_expected)
        .run();

    Package::new("foo", "0.0.3").publish();
    Package::new("foo", "0.0.4").publish();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(post_publish_expected)
        .run();
}

#[cargo_test]
fn bad_cksum_http() {
    let _server = setup_http();
    bad_cksum(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] bad-cksum v0.0.1 (registry `dummy-registry`)
[ERROR] failed to download replaced source registry `crates-io`

Caused by:
  failed to verify the checksum of `bad-cksum v0.0.1 (registry `dummy-registry`)`

"#]]);
}

#[cargo_test]
fn bad_cksum_git() {
    bad_cksum(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] bad-cksum v0.0.1 (registry `dummy-registry`)
[ERROR] failed to download replaced source registry `crates-io`

Caused by:
  failed to verify the checksum of `bad-cksum v0.0.1 (registry `dummy-registry`)`

"#]]);
}

fn bad_cksum(expected: impl IntoData) {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies]
                bad-cksum = ">= 0.0.0"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    let pkg = Package::new("bad-cksum", "0.0.1");
    pkg.publish();
    t!(File::create(&pkg.archive_dst()));

    p.cargo("check -v")
        .with_status(101)
        .with_stderr_data(expected)
        .run();
}

#[cargo_test]
fn update_registry_http() {
    let _server = setup_http();
    update_registry(
        str![[r#"
[UPDATING] `dummy-registry` index
[ERROR] no matching package named `notyet` found
location searched: `dummy-registry` index (which is replacing registry `crates-io`)
required by package `foo v0.0.1 ([ROOT]/foo)`

"#]],
        str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] notyet v0.0.1 (registry `dummy-registry`)
[CHECKING] notyet v0.0.1
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]],
    );
}

#[cargo_test]
fn update_registry_git() {
    update_registry(
        str![[r#"
[UPDATING] `dummy-registry` index
[ERROR] no matching package named `notyet` found
location searched: `dummy-registry` index (which is replacing registry `crates-io`)
required by package `foo v0.0.1 ([ROOT]/foo)`

"#]],
        str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] notyet v0.0.1 (registry `dummy-registry`)
[CHECKING] notyet v0.0.1
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]],
    );
}

fn update_registry(pre_publish_expected: impl IntoData, post_publish_expected: impl IntoData) {
    Package::new("init", "0.0.1").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies]
                notyet = ">= 0.0.0"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(pre_publish_expected)
        .run();

    Package::new("notyet", "0.0.1").publish();

    p.cargo("check")
        .with_stderr_data(post_publish_expected)
        .run();
}

#[cargo_test]
fn package_with_path_deps_http() {
    let _server = setup_http();
    package_with_path_deps(
        str![[r#"
[PACKAGING] foo v0.0.1 ([ROOT]/foo)
[UPDATING] `dummy-registry` index
[ERROR] failed to prepare local package for uploading

Caused by:
  no matching package named `notyet` found
  location searched: `dummy-registry` index (which is replacing registry `crates-io`)
  required by package `foo v0.0.1 ([ROOT]/foo)`

"#]],
        str![[r#"
[PACKAGING] foo v0.0.1 ([ROOT]/foo)
[UPDATING] `dummy-registry` index
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] foo v0.0.1 ([ROOT]/foo)
[DOWNLOADING] crates ...
[DOWNLOADED] notyet v0.0.1 (registry `dummy-registry`)
[COMPILING] notyet v0.0.1
[COMPILING] foo v0.0.1 ([ROOT]/foo/target/package/foo-0.0.1)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]],
    );
}

#[cargo_test]
fn package_with_path_deps_git() {
    package_with_path_deps(
        str![[r#"
[PACKAGING] foo v0.0.1 ([ROOT]/foo)
[UPDATING] `dummy-registry` index
[ERROR] failed to prepare local package for uploading

Caused by:
  no matching package named `notyet` found
  location searched: `dummy-registry` index (which is replacing registry `crates-io`)
  required by package `foo v0.0.1 ([ROOT]/foo)`

"#]],
        str![[r#"
[PACKAGING] foo v0.0.1 ([ROOT]/foo)
[UPDATING] `dummy-registry` index
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] foo v0.0.1 ([ROOT]/foo)
[DOWNLOADING] crates ...
[DOWNLOADED] notyet v0.0.1 (registry `dummy-registry`)
[COMPILING] notyet v0.0.1
[COMPILING] foo v0.0.1 ([ROOT]/foo/target/package/foo-0.0.1)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]],
    );
}

fn package_with_path_deps(
    pre_publish_expected: impl IntoData,
    post_publish_expected: impl IntoData,
) {
    Package::new("init", "0.0.1").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []
                license = "MIT"
                description = "foo"
                repository = "bar"

                [dependencies.notyet]
                version = "0.0.1"
                path = "notyet"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("notyet/Cargo.toml", &basic_manifest("notyet", "0.0.1"))
        .file("notyet/src/lib.rs", "")
        .build();

    p.cargo("package")
        .with_status(101)
        .with_stderr_data(pre_publish_expected)
        .run();

    Package::new("notyet", "0.0.1").publish();

    p.cargo("package")
        .with_stderr_data(post_publish_expected)
        .run();
}

#[cargo_test]
fn lockfile_locks_http() {
    let _server = setup_http();
    lockfile_locks(
        str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] bar v0.0.1 (registry `dummy-registry`)
[CHECKING] bar v0.0.1
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]],
        str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]],
    );
}

#[cargo_test]
fn lockfile_locks_git() {
    lockfile_locks(
        str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] bar v0.0.1 (registry `dummy-registry`)
[CHECKING] bar v0.0.1
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]],
        str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]],
    );
}

fn lockfile_locks(pre_publish_expected: impl IntoData, post_publish_expected: impl IntoData) {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies]
                bar = "*"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("bar", "0.0.1").publish();

    p.cargo("check")
        .with_stderr_data(pre_publish_expected)
        .run();

    p.root().move_into_the_past();
    Package::new("bar", "0.0.2").publish();

    p.cargo("check")
        .with_stderr_data(post_publish_expected)
        .run();
}

#[cargo_test]
fn lockfile_locks_transitively_http() {
    let _server = setup_http();
    lockfile_locks_transitively(
        str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 2 packages to latest compatible versions
[DOWNLOADING] crates ...
[DOWNLOADED] ba[..] v0.0.1 (registry `dummy-registry`)
[DOWNLOADED] ba[..] v0.0.1 (registry `dummy-registry`)
[CHECKING] baz v0.0.1
[CHECKING] bar v0.0.1
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]],
        str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]],
    );
}

#[cargo_test]
fn lockfile_locks_transitively_git() {
    lockfile_locks_transitively(
        str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 2 packages to latest compatible versions
[DOWNLOADING] crates ...
[DOWNLOADED] ba[..] v0.0.1 (registry `dummy-registry`)
[DOWNLOADED] ba[..] v0.0.1 (registry `dummy-registry`)
[CHECKING] baz v0.0.1
[CHECKING] bar v0.0.1
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]],
        str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]],
    );
}

fn lockfile_locks_transitively(
    pre_publish_expected: impl IntoData,
    post_publish_expected: impl IntoData,
) {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies]
                bar = "*"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("baz", "0.0.1").publish();
    Package::new("bar", "0.0.1").dep("baz", "*").publish();

    p.cargo("check")
        .with_stderr_data(pre_publish_expected)
        .run();

    p.root().move_into_the_past();
    Package::new("baz", "0.0.2").publish();
    Package::new("bar", "0.0.2").dep("baz", "*").publish();

    p.cargo("check")
        .with_stderr_data(post_publish_expected)
        .run();
}

#[cargo_test]
fn yanks_are_not_used_http() {
    let _server = setup_http();
    yanks_are_not_used(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 2 packages to latest compatible versions
[DOWNLOADING] crates ...
[DOWNLOADED] ba[..] v0.0.1 (registry `dummy-registry`)
[DOWNLOADED] ba[..] v0.0.1 (registry `dummy-registry`)
[CHECKING] baz v0.0.1
[CHECKING] bar v0.0.1
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]);
}

#[cargo_test]
fn yanks_are_not_used_git() {
    yanks_are_not_used(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 2 packages to latest compatible versions
[DOWNLOADING] crates ...
[DOWNLOADED] ba[..] v0.0.1 (registry `dummy-registry`)
[DOWNLOADED] ba[..] v0.0.1 (registry `dummy-registry`)
[CHECKING] baz v0.0.1
[CHECKING] bar v0.0.1
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]);
}

fn yanks_are_not_used(expected: impl IntoData) {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies]
                bar = "*"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("baz", "0.0.1").publish();
    Package::new("baz", "0.0.2").yanked(true).publish();
    Package::new("bar", "0.0.1").dep("baz", "*").publish();
    Package::new("bar", "0.0.2")
        .dep("baz", "*")
        .yanked(true)
        .publish();

    p.cargo("check").with_stderr_data(expected).run();
}

#[cargo_test]
fn relying_on_a_yank_is_bad_http() {
    let _server = setup_http();
    relying_on_a_yank_is_bad(str![[r#"
[UPDATING] `dummy-registry` index
[ERROR] failed to select a version for the requirement `baz = "=0.0.2"`
  version 0.0.2 is yanked
location searched: `dummy-registry` index (which is replacing registry `crates-io`)
required by package `bar v0.0.1`
    ... which satisfies dependency `bar = "*"` of package `foo v0.0.1 ([ROOT]/foo)`

"#]]);
}

#[cargo_test]
fn relying_on_a_yank_is_bad_git() {
    relying_on_a_yank_is_bad(str![[r#"
[UPDATING] `dummy-registry` index
[ERROR] failed to select a version for the requirement `baz = "=0.0.2"`
  version 0.0.2 is yanked
location searched: `dummy-registry` index (which is replacing registry `crates-io`)
required by package `bar v0.0.1`
    ... which satisfies dependency `bar = "*"` of package `foo v0.0.1 ([ROOT]/foo)`

"#]]);
}

fn relying_on_a_yank_is_bad(expected: impl IntoData) {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies]
                bar = "*"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("baz", "0.0.1").publish();
    Package::new("baz", "0.0.2").yanked(true).publish();
    Package::new("bar", "0.0.1").dep("baz", "=0.0.2").publish();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(expected)
        .run();
}

#[cargo_test]
fn yanks_in_lockfiles_are_ok_http() {
    let _server = setup_http();
    yanks_in_lockfiles_are_ok(
        str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]],
        str![[r#"
[UPDATING] `dummy-registry` index
[ERROR] failed to select a version for the requirement `bar = "*"`
  version 0.0.1 is yanked
location searched: `dummy-registry` index (which is replacing registry `crates-io`)
required by package `foo v0.0.1 ([ROOT]/foo)`

"#]],
    );
}

#[cargo_test]
fn yanks_in_lockfiles_are_ok_git() {
    yanks_in_lockfiles_are_ok(
        str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]],
        str![[r#"
[UPDATING] `dummy-registry` index
[ERROR] failed to select a version for the requirement `bar = "*"`
  version 0.0.1 is yanked
location searched: `dummy-registry` index (which is replacing registry `crates-io`)
required by package `foo v0.0.1 ([ROOT]/foo)`

"#]],
    );
}

fn yanks_in_lockfiles_are_ok(expected_check: impl IntoData, expected_update: impl IntoData) {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies]
                bar = "*"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("bar", "0.0.1").publish();

    p.cargo("check").run();

    registry_path().join("3").rm_rf();

    Package::new("bar", "0.0.1").yanked(true).publish();

    p.cargo("check").with_stderr_data(expected_check).run();

    p.cargo("update")
        .with_status(101)
        .with_stderr_data(expected_update)
        .run();
}

#[cargo_test]
fn yanks_in_lockfiles_are_ok_for_other_update_http() {
    let _server = setup_http();
    yanks_in_lockfiles_are_ok_for_other_update(
        str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]],
        str![[r#"
[UPDATING] `dummy-registry` index
[ERROR] failed to select a version for the requirement `bar = "*"`
  version 0.0.1 is yanked
location searched: `dummy-registry` index (which is replacing registry `crates-io`)
required by package `foo v0.0.1 ([ROOT]/foo)`

"#]],
        str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[UPDATING] baz v0.0.1 -> v0.0.2

"#]],
    );
}

#[cargo_test]
fn yanks_in_lockfiles_are_ok_for_other_update_git() {
    yanks_in_lockfiles_are_ok_for_other_update(
        str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]],
        str![[r#"
[UPDATING] `dummy-registry` index
[ERROR] failed to select a version for the requirement `bar = "*"`
  version 0.0.1 is yanked
location searched: `dummy-registry` index (which is replacing registry `crates-io`)
required by package `foo v0.0.1 ([ROOT]/foo)`

"#]],
        str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[UPDATING] baz v0.0.1 -> v0.0.2

"#]],
    );
}

fn yanks_in_lockfiles_are_ok_for_other_update(
    expected_check: impl IntoData,
    expected_update: impl IntoData,
    expected_other_update: impl IntoData,
) {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies]
                bar = "*"
                baz = "*"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("bar", "0.0.1").publish();
    Package::new("baz", "0.0.1").publish();

    p.cargo("check").run();

    registry_path().join("3").rm_rf();

    Package::new("bar", "0.0.1").yanked(true).publish();
    Package::new("baz", "0.0.1").publish();

    p.cargo("check").with_stderr_data(expected_check).run();

    Package::new("baz", "0.0.2").publish();

    p.cargo("update")
        .with_status(101)
        .with_stderr_data(expected_update)
        .run();

    p.cargo("update baz")
        .with_stderr_data(expected_other_update)
        .run();
}

#[cargo_test]
fn yanks_in_lockfiles_are_ok_with_new_dep_http() {
    let _server = setup_http();
    yanks_in_lockfiles_are_ok_with_new_dep(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[ADDING] baz v0.0.1
[DOWNLOADING] crates ...
[DOWNLOADED] baz v0.0.1 (registry `dummy-registry`)
[CHECKING] baz v0.0.1
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]);
}

#[cargo_test]
fn yanks_in_lockfiles_are_ok_with_new_dep_git() {
    yanks_in_lockfiles_are_ok_with_new_dep(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[ADDING] baz v0.0.1
[DOWNLOADING] crates ...
[DOWNLOADED] baz v0.0.1 (registry `dummy-registry`)
[CHECKING] baz v0.0.1
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]);
}

fn yanks_in_lockfiles_are_ok_with_new_dep(expected: impl IntoData) {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies]
                bar = "*"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("bar", "0.0.1").publish();

    p.cargo("check").run();

    registry_path().join("3").rm_rf();

    Package::new("bar", "0.0.1").yanked(true).publish();
    Package::new("baz", "0.0.1").publish();

    p.change_file(
        "Cargo.toml",
        r#"
            [package]
            name = "foo"
            version = "0.0.1"
            edition = "2015"
            authors = []

            [dependencies]
            bar = "*"
            baz = "*"
        "#,
    );

    p.cargo("check").with_stderr_data(expected).run();
}

#[cargo_test]
fn update_with_lockfile_if_packages_missing_http() {
    let _server = setup_http();
    update_with_lockfile_if_packages_missing(str![[r#"
[UPDATING] `dummy-registry` index
[DOWNLOADING] crates ...
[DOWNLOADED] bar v0.0.1 (registry `dummy-registry`)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]);
}

#[cargo_test]
fn update_with_lockfile_if_packages_missing_git() {
    update_with_lockfile_if_packages_missing(str![[r#"
[UPDATING] `dummy-registry` index
[DOWNLOADING] crates ...
[DOWNLOADED] bar v0.0.1 (registry `dummy-registry`)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]);
}

fn update_with_lockfile_if_packages_missing(expected: impl IntoData) {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies]
                bar = "*"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("bar", "0.0.1").publish();
    p.cargo("check").run();
    p.root().move_into_the_past();

    paths::home().join(".cargo/registry").rm_rf();
    p.cargo("check").with_stderr_data(expected).run();
}

#[cargo_test]
fn update_lockfile_http() {
    let _server = setup_http();
    update_lockfile(
        str![[r#"
[UPDATING] `dummy-registry` index
[UPDATING] bar v0.0.1 -> v0.0.2

"#]],
        str![[r#"
[DOWNLOADING] crates ...
[DOWNLOADED] bar v0.0.2 (registry `dummy-registry`)
[CHECKING] bar v0.0.2
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]],
        str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[UPDATING] bar v0.0.2 -> v0.0.3

"#]],
        str![[r#"
[DOWNLOADING] crates ...
[DOWNLOADED] bar v0.0.3 (registry `dummy-registry`)
[CHECKING] bar v0.0.3
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]],
        str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 2 packages to latest compatible versions
[UPDATING] bar v0.0.3 -> v0.0.4
[ADDING] spam v0.2.5

"#]],
        str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[UPDATING] bar v0.0.4 -> v0.0.5
[REMOVING] spam v0.2.5

"#]],
    );
}

#[cargo_test]
fn update_lockfile_git() {
    update_lockfile(
        str![[r#"
[UPDATING] `dummy-registry` index
[UPDATING] bar v0.0.1 -> v0.0.2

"#]],
        str![[r#"
[DOWNLOADING] crates ...
[DOWNLOADED] bar v0.0.2 (registry `dummy-registry`)
[CHECKING] bar v0.0.2
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]],
        str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[UPDATING] bar v0.0.2 -> v0.0.3

"#]],
        str![[r#"
[DOWNLOADING] crates ...
[DOWNLOADED] bar v0.0.3 (registry `dummy-registry`)
[CHECKING] bar v0.0.3
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]],
        str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 2 packages to latest compatible versions
[UPDATING] bar v0.0.3 -> v0.0.4
[ADDING] spam v0.2.5

"#]],
        str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[UPDATING] bar v0.0.4 -> v0.0.5
[REMOVING] spam v0.2.5

"#]],
    );
}

fn update_lockfile(
    expected_update: impl IntoData,
    expected_check: impl IntoData,
    expected_other_update: impl IntoData,
    expected_other_check: impl IntoData,
    expected_new_update: impl IntoData,
    expected_new_check: impl IntoData,
) {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies]
                bar = "*"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    println!("0.0.1");
    Package::new("bar", "0.0.1").publish();
    p.cargo("check").run();

    Package::new("bar", "0.0.2").publish();
    Package::new("bar", "0.0.3").publish();
    paths::home().join(".cargo/registry").rm_rf();
    println!("0.0.2 update");
    p.cargo("update bar --precise 0.0.2")
        .with_stderr_data(expected_update)
        .run();

    println!("0.0.2 build");
    p.cargo("check").with_stderr_data(expected_check).run();

    println!("0.0.3 update");
    p.cargo("update bar")
        .with_stderr_data(expected_other_update)
        .run();

    println!("0.0.3 build");
    p.cargo("check")
        .with_stderr_data(expected_other_check)
        .run();

    println!("new dependencies update");
    Package::new("bar", "0.0.4").dep("spam", "0.2.5").publish();
    Package::new("spam", "0.2.5").publish();
    p.cargo("update bar")
        .with_stderr_data(expected_new_update)
        .run();

    println!("new dependencies update");
    Package::new("bar", "0.0.5").publish();
    p.cargo("update bar")
        .with_stderr_data(expected_new_check)
        .run();
}

#[cargo_test]
fn dev_dependency_not_used_http() {
    let _server = setup_http();
    dev_dependency_not_used(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] bar v0.0.1 (registry `dummy-registry`)
[CHECKING] bar v0.0.1
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]);
}

#[cargo_test]
fn dev_dependency_not_used_git() {
    dev_dependency_not_used(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] bar v0.0.1 (registry `dummy-registry`)
[CHECKING] bar v0.0.1
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]);
}

fn dev_dependency_not_used(expected: impl IntoData) {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies]
                bar = "*"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("baz", "0.0.1").publish();
    Package::new("bar", "0.0.1").dev_dep("baz", "*").publish();

    p.cargo("check").with_stderr_data(expected).run();
}

#[cargo_test]
fn bad_license_file_http() {
    let registry = setup_http();
    bad_license_file(
        &registry,
        str![[r#"
...
[ERROR] license-file `foo` does not appear to exist (relative to `[ROOT]/foo`).
...
"#]],
    );
}

#[cargo_test]
fn bad_license_file_git() {
    let registry = registry::init();
    bad_license_file(
        &registry,
        str![[r#"
...
[ERROR] license-file `foo` does not appear to exist (relative to `[ROOT]/foo`).
...
"#]],
    );
}

fn bad_license_file(registry: &TestRegistry, expected: impl IntoData) {
    Package::new("foo", "1.0.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []
                license-file = "foo"
                description = "bar"
                repository = "baz"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();
    p.cargo("publish -v")
        .replace_crates_io(registry.index_url())
        .with_status(101)
        .with_stderr_data(expected)
        .run();
}

#[cargo_test]
fn updating_a_dep_http() {
    let _server = setup_http();
    updating_a_dep(
        str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 2 packages to latest compatible versions
[DOWNLOADING] crates ...
[DOWNLOADED] bar v0.0.1 (registry `dummy-registry`)
[CHECKING] bar v0.0.1
[CHECKING] a v0.0.1 ([ROOT]/foo/a)
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]],
        str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[UPDATING] bar v0.0.1 -> v0.1.0
[DOWNLOADING] crates ...
[DOWNLOADED] bar v0.1.0 (registry `dummy-registry`)
[CHECKING] bar v0.1.0
[CHECKING] a v0.0.1 ([ROOT]/foo/a)
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]],
    );
}

#[cargo_test]
fn updating_a_dep_git() {
    updating_a_dep(
        str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 2 packages to latest compatible versions
[DOWNLOADING] crates ...
[DOWNLOADED] bar v0.0.1 (registry `dummy-registry`)
[CHECKING] bar v0.0.1
[CHECKING] a v0.0.1 ([ROOT]/foo/a)
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]],
        str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[UPDATING] bar v0.0.1 -> v0.1.0
[DOWNLOADING] crates ...
[DOWNLOADED] bar v0.1.0 (registry `dummy-registry`)
[CHECKING] bar v0.1.0
[CHECKING] a v0.0.1 ([ROOT]/foo/a)
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]],
    );
}

fn updating_a_dep(pre_update_expected: impl IntoData, post_update_expected: impl IntoData) {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies.a]
                path = "a"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "a/Cargo.toml",
            r#"
                [package]
                name = "a"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies]
                bar = "*"
            "#,
        )
        .file("a/src/lib.rs", "")
        .build();

    Package::new("bar", "0.0.1").publish();

    p.cargo("check").with_stderr_data(pre_update_expected).run();
    assert!(paths::home().join(".cargo/registry/CACHEDIR.TAG").is_file());

    // Now delete the CACHEDIR.TAG file: this is the situation we'll be in after
    // upgrading from a version of Cargo that doesn't mark this directory, to one that
    // does. It should be recreated.
    fs::remove_file(paths::home().join(".cargo/registry/CACHEDIR.TAG"))
        .expect("remove CACHEDIR.TAG");

    p.change_file(
        "a/Cargo.toml",
        r#"
        [package]
        name = "a"
        version = "0.0.1"
        edition = "2015"
        authors = []

        [dependencies]
        bar = "0.1.0"
        "#,
    );
    Package::new("bar", "0.1.0").publish();

    println!("second");
    p.cargo("check")
        .with_stderr_data(post_update_expected)
        .run();

    assert!(
        paths::home().join(".cargo/registry/CACHEDIR.TAG").is_file(),
        "CACHEDIR.TAG recreated in existing registry"
    );
}

#[cargo_test]
fn git_and_registry_dep_http() {
    let _server = setup_http();
    git_and_registry_dep(
        str![[r#"
[UPDATING] `dummy-registry` index
[UPDATING] git repository `[ROOTURL]/b`
[LOCKING] 2 packages to latest compatible versions
[DOWNLOADING] crates ...
[DOWNLOADED] a v0.0.1 (registry `dummy-registry`)
[CHECKING] a v0.0.1
[CHECKING] b v0.0.1 ([ROOTURL]/b#[..])
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]],
        str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]],
    );
}

#[cargo_test]
fn git_and_registry_dep_git() {
    git_and_registry_dep(
        str![[r#"
[UPDATING] `dummy-registry` index
[UPDATING] git repository `[ROOTURL]/b`
[LOCKING] 2 packages to latest compatible versions
[DOWNLOADING] crates ...
[DOWNLOADED] a v0.0.1 (registry `dummy-registry`)
[CHECKING] a v0.0.1
[CHECKING] b v0.0.1 ([ROOTURL]/b#[..])
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]],
        str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]],
    );
}

fn git_and_registry_dep(pre_move_expected: impl IntoData, post_move_expected: impl IntoData) {
    let b = git::repo(&paths::root().join("b"))
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "b"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies]
                a = "0.0.1"
            "#,
        )
        .file("src/lib.rs", "")
        .build();
    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"
                    version = "0.0.1"
                    edition = "2015"
                    authors = []

                    [dependencies]
                    a = "0.0.1"

                    [dependencies.b]
                    git = '{}'
                "#,
                b.url()
            ),
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("a", "0.0.1").publish();

    p.root().move_into_the_past();
    p.cargo("check").with_stderr_data(pre_move_expected).run();
    p.root().move_into_the_past();

    println!("second");
    p.cargo("check").with_stderr_data(post_move_expected).run();
}

#[cargo_test]
fn update_publish_then_update_http() {
    let _server = setup_http();
    update_publish_then_update(str![[r#"
[UPDATING] `dummy-registry` index
[DOWNLOADING] crates ...
[DOWNLOADED] a v0.1.1 (registry `dummy-registry`)
[COMPILING] a v0.1.1
[COMPILING] foo v0.5.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]);
}

#[cargo_test]
fn update_publish_then_update_git() {
    update_publish_then_update(str![[r#"
[UPDATING] `dummy-registry` index
[DOWNLOADING] crates ...
[DOWNLOADED] a v0.1.1 (registry `dummy-registry`)
[COMPILING] a v0.1.1
[COMPILING] foo v0.5.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]);
}

fn update_publish_then_update(expected: impl IntoData) {
    // First generate a Cargo.lock and a clone of the registry index at the
    // "head" of the current registry.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.5.0"
                edition = "2015"
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
                [package]
                name = "foo"
                version = "0.5.0"
                edition = "2015"
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
    p.cargo("build").with_stderr_data(expected).run();
}

#[cargo_test]
fn fetch_downloads_http() {
    let _server = setup_http();
    fetch_downloads(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] a v0.1.0 (registry `dummy-registry`)

"#]]);
}

#[cargo_test]
fn fetch_downloads_git() {
    fetch_downloads(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] a v0.1.0 (registry `dummy-registry`)

"#]]);
}

fn fetch_downloads(expected: impl IntoData) {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.5.0"
                edition = "2015"
                authors = []

                [dependencies]
                a = "0.1.0"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("a", "0.1.0").publish();

    p.cargo("fetch").with_stderr_data(expected).run();
}

#[cargo_test]
fn update_transitive_dependency_http() {
    let _server = setup_http();
    update_transitive_dependency(
        str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[UPDATING] b v0.1.0 -> v0.1.1

"#]],
        str![[r#"
[DOWNLOADING] crates ...
[DOWNLOADED] b v0.1.1 (registry `dummy-registry`)
[CHECKING] b v0.1.1
[CHECKING] a v0.1.0
[CHECKING] foo v0.5.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]],
    );
}

#[cargo_test]
fn update_transitive_dependency_git() {
    update_transitive_dependency(
        str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[UPDATING] b v0.1.0 -> v0.1.1

"#]],
        str![[r#"
[DOWNLOADING] crates ...
[DOWNLOADED] b v0.1.1 (registry `dummy-registry`)
[CHECKING] b v0.1.1
[CHECKING] a v0.1.0
[CHECKING] foo v0.5.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]],
    );
}

fn update_transitive_dependency(expected_update: impl IntoData, expected_check: impl IntoData) {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.5.0"
                edition = "2015"
                authors = []

                [dependencies]
                a = "0.1.0"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("a", "0.1.0").dep("b", "*").publish();
    Package::new("b", "0.1.0").publish();

    p.cargo("fetch").run();

    Package::new("b", "0.1.1").publish();

    p.cargo("update b").with_stderr_data(expected_update).run();

    p.cargo("check").with_stderr_data(expected_check).run();
}

#[cargo_test]
fn update_backtracking_ok_http() {
    let _server = setup_http();
    update_backtracking_ok(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 2 packages to latest compatible versions
[UPDATING] hyper v0.6.5 -> v0.6.6
[UPDATING] openssl v0.1.0 -> v0.1.1

"#]]);
}

#[cargo_test]
fn update_backtracking_ok_git() {
    update_backtracking_ok(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 2 packages to latest compatible versions
[UPDATING] hyper v0.6.5 -> v0.6.6
[UPDATING] openssl v0.1.0 -> v0.1.1

"#]]);
}

fn update_backtracking_ok(expected: impl IntoData) {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.5.0"
                edition = "2015"
                authors = []

                [dependencies]
                webdriver = "0.1"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("webdriver", "0.1.0")
        .dep("hyper", "0.6")
        .publish();
    Package::new("hyper", "0.6.5")
        .dep("openssl", "0.1")
        .dep("cookie", "0.1")
        .publish();
    Package::new("cookie", "0.1.0")
        .dep("openssl", "0.1")
        .publish();
    Package::new("openssl", "0.1.0").publish();

    p.cargo("generate-lockfile").run();

    Package::new("openssl", "0.1.1").publish();
    Package::new("hyper", "0.6.6")
        .dep("openssl", "0.1.1")
        .dep("cookie", "0.1.0")
        .publish();

    p.cargo("update hyper").with_stderr_data(expected).run();
}

#[cargo_test]
fn update_multiple_packages_http() {
    let _server = setup_http();
    update_multiple_packages(
        str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 2 packages to latest compatible versions
[UPDATING] a v0.1.0 -> v0.1.1
[UPDATING] b v0.1.0 -> v0.1.1
[NOTE] pass `--verbose` to see 1 unchanged dependencies behind latest

"#]],
        str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[UPDATING] c v0.1.0 -> v0.1.1

"#]],
        str![[r#"
[DOWNLOADING] crates ...
[DOWNLOADED] a v0.1.1 (registry `dummy-registry`)
[DOWNLOADED] b v0.1.1 (registry `dummy-registry`)
[DOWNLOADED] c v0.1.1 (registry `dummy-registry`)
[CHECKING] a v0.1.1
[CHECKING] c v0.1.1
[CHECKING] b v0.1.1
[CHECKING] foo v0.5.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]],
    );
}

#[cargo_test]
fn update_multiple_packages_git() {
    update_multiple_packages(
        str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 2 packages to latest compatible versions
[UPDATING] a v0.1.0 -> v0.1.1
[UPDATING] b v0.1.0 -> v0.1.1
[NOTE] pass `--verbose` to see 1 unchanged dependencies behind latest

"#]],
        str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[UPDATING] c v0.1.0 -> v0.1.1

"#]],
        str![[r#"
[DOWNLOADING] crates ...
[DOWNLOADED] c v0.1.1 (registry `dummy-registry`)
[DOWNLOADED] b v0.1.1 (registry `dummy-registry`)
[DOWNLOADED] a v0.1.1 (registry `dummy-registry`)
[CHECKING] b v0.1.1
[CHECKING] a v0.1.1
[CHECKING] c v0.1.1
[CHECKING] foo v0.5.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]],
    );
}

fn update_multiple_packages(
    expected_update: impl IntoData,
    expected_other_update: impl IntoData,
    expected_check: impl IntoData,
) {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.5.0"
                edition = "2015"
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

    p.cargo("update a b")
        .with_stderr_data(expected_update)
        .run();

    p.cargo("update b c")
        .with_stderr_data(expected_other_update)
        .run();

    p.cargo("check")
        .with_stderr_data(IntoData::unordered(expected_check))
        .run();
}

#[cargo_test]
fn bundled_crate_in_registry_http() {
    let _server = setup_http();
    bundled_crate_in_registry();
}

#[cargo_test]
fn bundled_crate_in_registry_git() {
    bundled_crate_in_registry();
}

fn bundled_crate_in_registry() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.5.0"
                edition = "2015"
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
                edition = "2015"
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

#[cargo_test]
fn update_same_prefix_oh_my_how_was_this_a_bug_http() {
    let _server = setup_http();
    update_same_prefix_oh_my_how_was_this_a_bug();
}

#[cargo_test]
fn update_same_prefix_oh_my_how_was_this_a_bug_git() {
    update_same_prefix_oh_my_how_was_this_a_bug();
}

fn update_same_prefix_oh_my_how_was_this_a_bug() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "ugh"
                version = "0.5.0"
                edition = "2015"
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
    p.cargo("update foobar --precise=0.2.0").run();
}

#[cargo_test]
fn use_semver_http() {
    let _server = setup_http();
    use_semver();
}

#[cargo_test]
fn use_semver_git() {
    use_semver();
}

fn use_semver() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.5.0"
                edition = "2015"
                authors = []

                [dependencies]
                foo = "1.2.3-alpha.0"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("foo", "1.2.3-alpha.0").publish();

    p.cargo("check").run();
}

#[cargo_test]
fn use_semver_package_incorrectly_http() {
    let _server = setup_http();
    use_semver_package_incorrectly(str![[r#"
[ERROR] failed to select a version for the requirement `a = "^0.1"`
candidate versions found which didn't match: 0.1.1-alpha.0
location searched: [ROOT]/foo/a
required by package `b v0.1.0 ([ROOT]/foo/b)`
if you are looking for the prerelease package it needs to be specified explicitly
    a = { version = "0.1.1-alpha.0" }

"#]]);
}

#[cargo_test]
fn use_semver_package_incorrectly_git() {
    use_semver_package_incorrectly(str![[r#"
[ERROR] failed to select a version for the requirement `a = "^0.1"`
candidate versions found which didn't match: 0.1.1-alpha.0
location searched: [ROOT]/foo/a
required by package `b v0.1.0 ([ROOT]/foo/b)`
if you are looking for the prerelease package it needs to be specified explicitly
    a = { version = "0.1.1-alpha.0" }

"#]]);
}

fn use_semver_package_incorrectly(expected: impl IntoData) {
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
            [package]
            name = "a"
            version = "0.1.1-alpha.0"
            edition = "2015"
            authors = []
            "#,
        )
        .file(
            "b/Cargo.toml",
            r#"
            [package]
            name = "b"
            version = "0.1.0"
            edition = "2015"
            authors = []

            [dependencies]
            a = { version = "^0.1", path = "../a" }
            "#,
        )
        .file("a/src/main.rs", "fn main() {}")
        .file("b/src/main.rs", "fn main() {}")
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(expected)
        .run();
}

#[cargo_test]
fn only_download_relevant_http() {
    let _server = setup_http();
    only_download_relevant(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 3 packages to latest compatible versions
[DOWNLOADING] crates ...
[DOWNLOADED] baz v0.1.0 (registry `dummy-registry`)
[CHECKING] baz v0.1.0
[CHECKING] bar v0.5.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]);
}

#[cargo_test]
fn only_download_relevant_git() {
    only_download_relevant(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 3 packages to latest compatible versions
[DOWNLOADING] crates ...
[DOWNLOADED] baz v0.1.0 (registry `dummy-registry`)
[CHECKING] baz v0.1.0
[CHECKING] bar v0.5.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]);
}

fn only_download_relevant(expected: impl IntoData) {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.5.0"
                edition = "2015"
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

    p.cargo("check").with_stderr_data(expected).run();
}

#[cargo_test]
fn resolve_and_backtracking_http() {
    let _server = setup_http();
    resolve_and_backtracking();
}

#[cargo_test]
fn resolve_and_backtracking_git() {
    resolve_and_backtracking();
}

fn resolve_and_backtracking() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.5.0"
                edition = "2015"
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

    p.cargo("check").run();
}

#[cargo_test]
fn upstream_warnings_on_extra_verbose_http() {
    let _server = setup_http();
    upstream_warnings_on_extra_verbose(str![[r#"
...
[WARNING] function `unused` is never used
...
"#]]);
}

#[cargo_test]
fn upstream_warnings_on_extra_verbose_git() {
    upstream_warnings_on_extra_verbose(str![[r#"
...
[WARNING] function `unused` is never used
...
"#]]);
}

fn upstream_warnings_on_extra_verbose(expected: impl IntoData) {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.5.0"
                edition = "2015"
                authors = []

                [dependencies]
                foo = "*"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("foo", "0.1.0")
        .file("src/lib.rs", "fn unused() {}")
        .publish();

    p.cargo("check -vv").with_stderr_data(expected).run();
}

#[cargo_test]
fn disallow_network_http() {
    let _server = setup_http();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.5.0"
                edition = "2015"
                authors = []

                [dependencies]
                foo = "*"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("check --frozen")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] no matching package named `foo` found
location searched: `dummy-registry` index (which is replacing registry `crates-io`)
required by package `bar v0.5.0 ([ROOT]/foo)`
As a reminder, you're using offline mode (--frozen) which can sometimes cause surprising resolution failures, if this error is too confusing you may wish to retry without `--frozen`.

"#]])
        .run();
}

#[cargo_test]
fn disallow_network_git() {
    let _server = RegistryBuilder::new().build();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.5.0"
                edition = "2015"
                authors = []

                [dependencies]
                foo = "*"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("check --frozen")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] no matching package named `foo` found
location searched: `dummy-registry` index (which is replacing registry `crates-io`)
required by package `bar v0.5.0 ([ROOT]/foo)`
As a reminder, you're using offline mode (--frozen) which can sometimes cause surprising resolution failures, if this error is too confusing you may wish to retry without `--frozen`.

"#]])
        .run();
}

#[cargo_test]
fn add_dep_dont_update_registry_http() {
    let _server = setup_http();
    add_dep_dont_update_registry(str![[r#"
[CHECKING] bar v0.5.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]);
}

#[cargo_test]
fn add_dep_dont_update_registry_git() {
    add_dep_dont_update_registry(str![[r#"
[CHECKING] bar v0.5.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]);
}

fn add_dep_dont_update_registry(expected: impl IntoData) {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.5.0"
                edition = "2015"
                authors = []

                [dependencies]
                baz = { path = "baz" }
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "baz/Cargo.toml",
            r#"
                [package]
                name = "baz"
                version = "0.5.0"
                edition = "2015"
                authors = []

                [dependencies]
                remote = "0.3"
            "#,
        )
        .file("baz/src/lib.rs", "")
        .build();

    Package::new("remote", "0.3.4").publish();

    p.cargo("check").run();

    p.change_file(
        "Cargo.toml",
        r#"
        [package]
        name = "bar"
        version = "0.5.0"
        edition = "2015"
        authors = []

        [dependencies]
        baz = { path = "baz" }
        remote = "0.3"
        "#,
    );

    p.cargo("check").with_stderr_data(expected).run();
}

#[cargo_test]
fn bump_version_dont_update_registry_http() {
    let _server = setup_http();
    bump_version_dont_update_registry(str![[r#"
[CHECKING] bar v0.6.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]);
}

#[cargo_test]
fn bump_version_dont_update_registry_git() {
    bump_version_dont_update_registry(str![[r#"
[CHECKING] bar v0.6.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]);
}

fn bump_version_dont_update_registry(expected: impl IntoData) {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.5.0"
                edition = "2015"
                authors = []

                [dependencies]
                baz = { path = "baz" }
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "baz/Cargo.toml",
            r#"
                [package]
                name = "baz"
                version = "0.5.0"
                edition = "2015"
                authors = []

                [dependencies]
                remote = "0.3"
            "#,
        )
        .file("baz/src/lib.rs", "")
        .build();

    Package::new("remote", "0.3.4").publish();

    p.cargo("check").run();

    p.change_file(
        "Cargo.toml",
        r#"
        [package]
        name = "bar"
        version = "0.6.0"
        edition = "2015"
        authors = []

        [dependencies]
        baz = { path = "baz" }
        "#,
    );

    p.cargo("check").with_stderr_data(expected).run();
}

#[cargo_test]
fn toml_lies_but_index_is_truth_http() {
    let _server = setup_http();
    toml_lies_but_index_is_truth();
}

#[cargo_test]
fn toml_lies_but_index_is_truth_git() {
    toml_lies_but_index_is_truth();
}

fn toml_lies_but_index_is_truth() {
    Package::new("foo", "0.2.0").publish();
    Package::new("bar", "0.3.0")
        .dep("foo", "0.2.0")
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.3.0"
                edition = "2015"
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
                [package]
                name = "bar"
                version = "0.5.0"
                edition = "2015"
                authors = []

                [dependencies]
                bar = "0.3"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("check -v").run();
}

#[cargo_test]
fn vv_prints_warnings_http() {
    let _server = setup_http();
    vv_prints_warnings();
}

#[cargo_test]
fn vv_prints_warnings_git() {
    vv_prints_warnings();
}

fn vv_prints_warnings() {
    Package::new("foo", "0.2.0")
        .file(
            "src/lib.rs",
            "#![deny(warnings)] fn foo() {} // unused function",
        )
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "fo"
                version = "0.5.0"
                edition = "2015"
                authors = []

                [dependencies]
                foo = "0.2"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("check -vv").run();
}

#[cargo_test]
fn bad_and_or_malicious_packages_rejected_http() {
    let _server = setup_http();
    bad_and_or_malicious_packages_rejected(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] foo v0.2.0 (registry `dummy-registry`)
[ERROR] failed to download replaced source registry `crates-io`

Caused by:
  failed to unpack package `foo v0.2.0 (registry `dummy-registry`)`

Caused by:
  invalid tarball downloaded, contains a file at "foo-0.1.0/src/lib.rs" which isn't under "foo-0.2.0"

"#]]);
}

#[cargo_test]
fn bad_and_or_malicious_packages_rejected_git() {
    bad_and_or_malicious_packages_rejected(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] foo v0.2.0 (registry `dummy-registry`)
[ERROR] failed to download replaced source registry `crates-io`

Caused by:
  failed to unpack package `foo v0.2.0 (registry `dummy-registry`)`

Caused by:
  invalid tarball downloaded, contains a file at "foo-0.1.0/src/lib.rs" which isn't under "foo-0.2.0"

"#]]);
}

fn bad_and_or_malicious_packages_rejected(expected: impl IntoData) {
    Package::new("foo", "0.2.0")
        .extra_file("foo-0.1.0/src/lib.rs", "")
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "fo"
                version = "0.5.0"
                edition = "2015"
                authors = []

                [dependencies]
                foo = "0.2"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("check -vv")
        .with_status(101)
        .with_stderr_data(expected)
        .run();
}

#[cargo_test]
fn git_init_templatedir_missing_http() {
    let _server = setup_http();
    git_init_templatedir_missing();
}

#[cargo_test]
fn git_init_templatedir_missing_git() {
    git_init_templatedir_missing();
}

fn git_init_templatedir_missing() {
    Package::new("foo", "0.2.0").dep("bar", "*").publish();
    Package::new("bar", "0.2.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "fo"
                version = "0.5.0"
                edition = "2015"
                authors = []

                [dependencies]
                foo = "0.2"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("check").run();

    remove_dir_all(paths::home().join(".cargo/registry")).unwrap();
    fs::write(
        paths::home().join(".gitconfig"),
        r#"
            [init]
            templatedir = nowhere
        "#,
    )
    .unwrap();

    p.cargo("check").run();
    p.cargo("check").run();
}

#[cargo_test]
fn rename_deps_and_features_http() {
    let _server = setup_http();
    rename_deps_and_features();
}

#[cargo_test]
fn rename_deps_and_features_git() {
    rename_deps_and_features();
}

fn rename_deps_and_features() {
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
                [package]
                name = "a"
                version = "0.5.0"
                edition = "2015"
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

    p.cargo("check").run();
    p.cargo("check --features bar/foo01").run();
    p.cargo("check --features bar/another").run();
}

#[cargo_test]
fn ignore_invalid_json_lines_http() {
    let _server = setup_http();
    ignore_invalid_json_lines();
}

#[cargo_test]
fn ignore_invalid_json_lines_git() {
    ignore_invalid_json_lines();
}

fn ignore_invalid_json_lines() {
    Package::new("foo", "0.1.0").publish();
    Package::new("foo", "0.1.1")
        .invalid_index_line(true)
        .publish();
    Package::new("foo", "0.2.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "a"
                version = "0.5.0"
                edition = "2015"
                authors = []

                [dependencies]
                foo = '0.1.0'
                foo02 = { version = '0.2.0', package = 'foo' }
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check").run();
}

#[cargo_test]
fn invalid_json_lines_error() {
    Package::new("foo", "0.1.0")
        .rust_version("1.0")
        .schema_version(2)
        .publish();
    Package::new("foo", "0.1.1")
        // Bad name field, too corrupt to use
        .invalid_index_line(true)
        .publish();
    Package::new("foo", "0.1.2")
        // Bad version field, too corrupt to use
        .index_line(
            r#"{"cksum":"7ca5fc2301ad96ade45356faf53225aea36437d99930bbfa951155c01faecf79","deps":[],"features":{},"links":null,"name":"foo","vers":"bad","yanked":false,"rust_version":"1.2345","v":1000000000}"#,
        )
        .publish();
    Package::new("foo", "0.1.3")
        // Bad field, report rust version
        .index_line(
            r#"{"cksum":"7ca5fc2301ad96ade45356faf53225aea36437d99930bbfa951155c01faecf79","deps":[],"features":"bad","links":null,"name":"foo","vers":"0.1.3","yanked":false,"rust_version":"1.2345","v":1000000000}"#,
        )
        .publish();
    Package::new("foo", "0.1.4")
        // Bad field, report schema
        .index_line(
            r#"{"cksum":"7ca5fc2301ad96ade45356faf53225aea36437d99930bbfa951155c01faecf79","deps":[],"features":"bad","links":null,"name":"foo","vers":"0.1.4","yanked":false,"v":1000000000}"#,
        )
        .publish();
    Package::new("foo", "0.1.5")
        // Bad field, report error
        .index_line(
            r#"{"cksum":"7ca5fc2301ad96ade45356faf53225aea36437d99930bbfa951155c01faecf79","deps":[],"features":"bad","links":null,"name":"foo","vers":"0.1.5","yanked":false}"#,
        )
        .publish();
    Package::new("foo", "0.1.6")
        // Bad field with bad rust version, report schema
        .index_line(
            r#"{"cksum":"7ca5fc2301ad96ade45356faf53225aea36437d99930bbfa951155c01faecf79","deps":[],"features":"bad","links":null,"name":"foo","vers":"0.1.6","yanked":false,"rust_version":"bad","v":1000000000}"#,
        )
        .publish();
    Package::new("foo", "0.1.7")
        // Bad field with bad rust version and schema, report error
        .index_line(
            r#"{"cksum":"7ca5fc2301ad96ade45356faf53225aea36437d99930bbfa951155c01faecf79","deps":[],"features":"bad","links":null,"name":"foo","vers":"0.1.7","yanked":false,"rust_version":"bad","v":"bad"}"#,
        )
        .publish();
    Package::new("foo", "0.2.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "a"
                version = "0.5.0"
                edition = "2015"
                authors = []

                [dependencies]
                foo = "0.1.1"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("generate-lockfile")
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[ERROR] failed to select a version for the requirement `foo = "^0.1.1"`
  version 0.1.3 requires cargo 1.2345
  version 0.1.4 requires a Cargo version that supports index version 1000000000
  version 0.1.5's index entry is invalid
  version 0.1.6 requires a Cargo version that supports index version 1000000000
  version 0.1.7's index entry is invalid
location searched: `dummy-registry` index (which is replacing registry `crates-io`)
required by package `a v0.5.0 ([ROOT]/foo)`

"#]])
        .run();
    p.cargo("generate-lockfile")
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[ERROR] failed to select a version for the requirement `foo = "^0.1.1"`
  version 0.1.3 requires cargo 1.2345
  version 0.1.4 requires a Cargo version that supports index version 1000000000
  version 0.1.5's index entry is invalid
  version 0.1.6 requires a Cargo version that supports index version 1000000000
  version 0.1.7's index entry is invalid
location searched: `dummy-registry` index (which is replacing registry `crates-io`)
required by package `a v0.5.0 ([ROOT]/foo)`

"#]])
        .run();
}

#[cargo_test]
fn readonly_registry_still_works_http() {
    let _server = setup_http();
    readonly_registry_still_works();
}

#[cargo_test]
fn readonly_registry_still_works_git() {
    readonly_registry_still_works();
}

fn readonly_registry_still_works() {
    Package::new("foo", "0.1.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "a"
                version = "0.5.0"
                edition = "2015"
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
    p.cargo("check").run();
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

#[cargo_test(ignore_windows = "On Windows setting file attributes is a bit complicated")]
fn inaccessible_registry_cache_still_works() {
    Package::new("foo", "0.1.0").publish();
    Package::new("fo2", "0.1.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "a"
                version = "0.5.0"
                edition = "2015"
                authors = []

                [dependencies]
                foo = '0.1.0'
                fo2 = '0.1.0'
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("generate-lockfile").run();
    p.cargo("fetch --locked").run();

    let cache_path = inner_dir(&paths::cargo_home().join("registry/index")).join(".cache");
    let f_cache_path = cache_path.join("3/f");

    // Remove the permissions from the cache path that contains the "foo" crate
    set_permissions(&f_cache_path, 0o000);

    // Now run a build and make sure we properly build and warn the user
    p.cargo("build")
        .with_stderr_data(str![[r#"
[WARNING] failed to write cache, path: [ROOT]/home/.cargo/registry/index/-[HASH]/.cache/3/f/fo[..], [ERROR] Permission denied (os error 13)
[COMPILING] fo[..] v0.1.0
[COMPILING] fo[..] v0.1.0
[COMPILING] a v0.5.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    // make sure we add the permissions to the files afterwards so "cargo clean" can remove them (#6934)
    set_permissions(&f_cache_path, 0o777);

    #[cfg_attr(windows, allow(unused_variables))]
    fn set_permissions(path: &Path, permissions: u32) {
        #[cfg(not(windows))]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = t!(path.metadata()).permissions();
            perms.set_mode(permissions);
            t!(fs::set_permissions(path, perms));
        }

        #[cfg(windows)]
        panic!("This test is not supported on windows. See the reason in the #[cargo_test] macro");
    }

    fn inner_dir(path: &Path) -> PathBuf {
        for entry in t!(path.read_dir()) {
            let path = t!(entry).path();

            if path.is_dir() {
                return path;
            }
        }

        panic!("could not find inner directory of {path:?}");
    }
}

#[cargo_test]
fn registry_index_rejected_http() {
    let _server = setup_http();
    registry_index_rejected(
        str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  the `registry.index` config value is no longer supported
  Use `[source]` replacement to alter the default index for crates.io.

"#]],
        str![[r#"
[ERROR] the `registry.index` config value is no longer supported
Use `[source]` replacement to alter the default index for crates.io.

"#]],
    );
}

#[cargo_test]
fn registry_index_rejected_git() {
    registry_index_rejected(
        str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  the `registry.index` config value is no longer supported
  Use `[source]` replacement to alter the default index for crates.io.

"#]],
        str![[r#"
[ERROR] the `registry.index` config value is no longer supported
Use `[source]` replacement to alter the default index for crates.io.

"#]],
    );
}

fn registry_index_rejected(expected_check: impl IntoData, expected_login: impl IntoData) {
    Package::new("dep", "0.1.0").publish();

    let p = project()
        .file(
            ".cargo/config.toml",
            r#"
            [registry]
            index = "https://example.com/"
            "#,
        )
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            edition = "2015"

            [dependencies]
            dep = "0.1"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(expected_check)
        .run();

    p.cargo("login")
        .with_status(101)
        .with_stderr_data(expected_login)
        .run();
}

#[cargo_test]
fn package_lock_inside_package_is_overwritten() {
    let registry = registry::init();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies]
                bar = ">= 0.0.0"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("bar", "0.0.1")
        .file("src/lib.rs", "")
        .file(".cargo-ok", "")
        .publish();

    p.cargo("check").run();

    let id = SourceId::for_registry(registry.index_url()).unwrap();
    let hash = cargo::util::hex::short_hash(&id);
    let ok = paths::cargo_home()
        .join("registry")
        .join("src")
        .join(format!("-{}", hash))
        .join("bar-0.0.1")
        .join(".cargo-ok");

    assert_eq!(ok.metadata().unwrap().len(), 7);
}

#[cargo_test]
fn package_lock_as_a_symlink_inside_package_is_overwritten() {
    let registry = registry::init();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies]
                bar = ">= 0.0.0"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("bar", "0.0.1")
        .file("src/lib.rs", "pub fn f() {}")
        .symlink(".cargo-ok", "src/lib.rs")
        .publish();

    p.cargo("check").run();

    let id = SourceId::for_registry(registry.index_url()).unwrap();
    let hash = cargo::util::hex::short_hash(&id);
    let pkg_root = paths::cargo_home()
        .join("registry")
        .join("src")
        .join(format!("-{}", hash))
        .join("bar-0.0.1");
    let ok = pkg_root.join(".cargo-ok");
    let librs = pkg_root.join("src/lib.rs");

    // Is correctly overwritten and doesn't affect the file linked to
    assert_eq!(ok.metadata().unwrap().len(), 7);
    assert_eq!(fs::read_to_string(librs).unwrap(), "pub fn f() {}");
}

#[cargo_test]
fn ignores_unknown_index_version_http() {
    let _server = setup_http();
    ignores_unknown_index_version(str![[r#"
foo v0.1.0 ([ROOT]/foo)
└── bar v1.0.0

"#]]);
}

#[cargo_test]
fn ignores_unknown_index_version_git() {
    ignores_unknown_index_version(str![[r#"
foo v0.1.0 ([ROOT]/foo)
└── bar v1.0.0

"#]]);
}

fn ignores_unknown_index_version(expected: impl IntoData) {
    // If the version field is not understood, it is ignored.
    Package::new("bar", "1.0.0").publish();
    Package::new("bar", "1.0.1")
        .schema_version(u32::MAX)
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

                [dependencies]
                bar = "1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("tree").with_stdout_data(expected).run();
}

#[cargo_test]
fn unknown_index_version_error() {
    Package::new("bar", "0.0.1").publish();
    // If the version field is not understood, it is ignored.
    Package::new("bar", "1.0.1")
        .schema_version(u32::MAX)
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

                [dependencies]
                bar = "1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("generate-lockfile")
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[ERROR] failed to select a version for the requirement `bar = "^1.0"`
  version 1.0.1 requires a Cargo version that supports index version 4294967295
location searched: `dummy-registry` index (which is replacing registry `crates-io`)
required by package `foo v0.1.0 ([ROOT]/foo)`

"#]])
        .run();
    p.cargo("generate-lockfile")
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[ERROR] failed to select a version for the requirement `bar = "^1.0"`
  version 1.0.1 requires a Cargo version that supports index version 4294967295
location searched: `dummy-registry` index (which is replacing registry `crates-io`)
required by package `foo v0.1.0 ([ROOT]/foo)`

"#]])
        .run();
}

#[cargo_test]
fn unknown_index_version_with_msrv_error() {
    Package::new("bar", "0.0.1").publish();
    // If the version field is not understood, it is ignored.
    Package::new("bar", "1.0.1")
        .schema_version(u32::MAX)
        .rust_version("1.2345")
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

                [dependencies]
                bar = "1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("generate-lockfile")
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[ERROR] failed to select a version for the requirement `bar = "^1.0"`
  version 1.0.1 requires cargo 1.2345
location searched: `dummy-registry` index (which is replacing registry `crates-io`)
required by package `foo v0.1.0 ([ROOT]/foo)`

"#]])
        .run();
}

#[cargo_test]
fn protocol() {
    cargo_process("install bar")
        .with_status(101)
        .env("CARGO_REGISTRIES_CRATES_IO_PROTOCOL", "invalid")
        .with_stderr_data(str![[r#"
[ERROR] unsupported registry protocol `invalid` (defined in environment variable `CARGO_REGISTRIES_CRATES_IO_PROTOCOL`)

"#]])
        .run();
}

#[cargo_test]
fn http_requires_trailing_slash() {
    cargo_process("install bar --index sparse+https://invalid.crates.io/test")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] sparse registry url must end in a slash `/`: sparse+https://invalid.crates.io/test

"#]])
        .run();
}

// Limit the test to debug builds so that `__CARGO_TEST_MAX_UNPACK_SIZE` will take affect.
#[cfg(debug_assertions)]
#[cargo_test]
fn reach_max_unpack_size() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"

                [dependencies]
                bar = ">= 0.0.0"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    // Size of bar.crate is around 180 bytes.
    Package::new("bar", "0.0.1").publish();

    p.cargo("check")
        .env("__CARGO_TEST_MAX_UNPACK_SIZE", "8") // hit 8 bytes limit and boom!
        .env("__CARGO_TEST_MAX_UNPACK_RATIO", "0")
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] bar v0.0.1 (registry `dummy-registry`)
[ERROR] failed to download replaced source registry `crates-io`

Caused by:
  failed to unpack package `bar v0.0.1 (registry `dummy-registry`)`

Caused by:
  failed to iterate over archive

Caused by:
  maximum limit reached when reading

"#]])
        .run();

    // Restore to the default ratio and it should compile.
    p.cargo("check")
        .env("__CARGO_TEST_MAX_UNPACK_SIZE", "8")
        .with_stderr_data(str![[r#"
[CHECKING] bar v0.0.1
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn sparse_blocking_count() {
    let fail_count = Mutex::new(0);
    let _registry = RegistryBuilder::new()
        .http_index()
        .add_responder("/index/3/b/bar", move |req, server| {
            let mut fail_count = fail_count.lock().unwrap();
            if *fail_count < 1 {
                *fail_count += 1;
                server.internal_server_error(req)
            } else {
                server.index(req)
            }
        })
        .build();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies]
                bar = ">= 0.0.0"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("bar", "0.0.1").publish();

    // Ensure we have the expected number of `block_until_ready` calls.
    // The 1st (0 transfers pending), is the deliberate extra call in `ensure_loaded` for a source.
    // The 2nd (1 transfers pending), is the registry `config.json`.
    // the 3rd (1 transfers pending), is the package metadata for `bar`.

    p.cargo("check")
        .env("CARGO_LOG", "network::HttpRegistry::block_until_ready=trace")
        .with_stderr_data(str![[r#"
   [..] TRACE network::HttpRegistry::block_until_ready: 0 transfers pending
[UPDATING] `dummy-registry` index
   [..] TRACE network::HttpRegistry::block_until_ready: 1 transfers pending
   [..] TRACE network::HttpRegistry::block_until_ready: 1 transfers pending
[WARNING] spurious network error (3 tries remaining): failed to get successful HTTP response from `[..]/index/3/b/bar` ([..]), got 500
body:
internal server error
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] bar v0.0.1 (registry `dummy-registry`)
[CHECKING] bar v0.0.1
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]).run();
}

#[cargo_test]
fn sparse_retry_single() {
    let fail_count = Mutex::new(0);
    let _registry = RegistryBuilder::new()
        .http_index()
        .add_responder("/index/3/b/bar", move |req, server| {
            let mut fail_count = fail_count.lock().unwrap();
            if *fail_count < 2 {
                *fail_count += 1;
                server.internal_server_error(req)
            } else {
                server.index(req)
            }
        })
        .build();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies]
                bar = ">= 0.0.0"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("bar", "0.0.1").publish();

    p.cargo("check").with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[WARNING] spurious network error (3 tries remaining): failed to get successful HTTP response from `http://127.0.0.1:[..]/index/3/b/bar` (127.0.0.1), got 500
body:
internal server error
[WARNING] spurious network error (2 tries remaining): failed to get successful HTTP response from `http://127.0.0.1:[..]/index/3/b/bar` (127.0.0.1), got 500
body:
internal server error
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] bar v0.0.1 (registry `dummy-registry`)
[CHECKING] bar v0.0.1
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]).run();
}

#[cargo_test]
fn sparse_retry_multiple() {
    // Tests retry behavior of downloading lots of packages with various
    // failure rates accessing the sparse index.

    // The index is the number of retries, the value is the number of packages
    // that retry that number of times. Thus 50 packages succeed on first try,
    // 25 on second, etc.
    const RETRIES: &[u32] = &[50, 25, 12, 6];

    let pkgs: Vec<_> = RETRIES
        .iter()
        .enumerate()
        .flat_map(|(retries, num)| {
            (0..*num)
                .into_iter()
                .map(move |n| (retries as u32, format!("{}-{n}-{retries}", rand_prefix())))
        })
        .collect();

    let mut builder = RegistryBuilder::new().http_index();
    let fail_counts: Arc<Mutex<Vec<u32>>> = Arc::new(Mutex::new(vec![0; pkgs.len()]));
    let mut cargo_toml = r#"
        [package]
        name = "foo"
        version = "0.1.0"
        edition = "2015"

        [dependencies]
        "#
    .to_string();
    // The expected stderr output.
    let mut expected = "\
[UPDATING] `dummy-registry` index
[DOWNLOADING] crates ...
"
    .to_string();
    for (n, (retries, name)) in pkgs.iter().enumerate() {
        let count_clone = fail_counts.clone();
        let retries = *retries;
        let ab = &name[..2];
        let cd = &name[2..4];
        builder = builder.add_responder(format!("/index/{ab}/{cd}/{name}"), move |req, server| {
            let mut fail_counts = count_clone.lock().unwrap();
            if fail_counts[n] < retries {
                fail_counts[n] += 1;
                server.internal_server_error(req)
            } else {
                server.index(req)
            }
        });
        write!(&mut cargo_toml, "{name} = \"1.0.0\"\n").unwrap();
        for retry in 0..retries {
            let remain = 3 - retry;
            write!(
                &mut expected,
                "[WARNING] spurious network error ({remain} {} remaining): \
                failed to get successful HTTP response from \
                `http://127.0.0.1:[..]/{ab}/{cd}/{name}` (127.0.0.1), got 500\n\
                body:\n\
                internal server error\n",
                if remain != 1 { "tries" } else { "try" }
            )
            .unwrap();
        }
        write!(
            &mut expected,
            "\
[DOWNLOADED] {name} v1.0.0 (registry `dummy-registry`)
"
        )
        .unwrap();
    }
    write!(
        &mut expected,
        "\
[LOCKING] 93 packages to latest compatible versions
"
    )
    .unwrap();
    let _server = builder.build();
    for (_, name) in &pkgs {
        Package::new(name, "1.0.0").publish();
    }
    let p = project()
        .file("Cargo.toml", &cargo_toml)
        .file("src/lib.rs", "")
        .build();
    p.cargo("fetch")
        .with_stderr_data(IntoData::unordered(expected))
        .run();
}

#[cargo_test]
fn dl_retry_single() {
    // Tests retry behavior of downloading a package.
    // This tests a single package which exercises the code path that causes
    // it to block.
    let fail_count = Mutex::new(0);
    let _server = RegistryBuilder::new()
        .http_index()
        .add_responder("/dl/bar/1.0.0/download", move |req, server| {
            let mut fail_count = fail_count.lock().unwrap();
            if *fail_count < 2 {
                *fail_count += 1;
                server.internal_server_error(req)
            } else {
                server.dl(req)
            }
        })
        .build();
    Package::new("bar", "1.0.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

                [dependencies]
                bar = "1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .build();
    p.cargo("fetch").with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[WARNING] spurious network error (3 tries remaining): failed to get successful HTTP response from `http://127.0.0.1:[..]/dl/bar/1.0.0/download` (127.0.0.1), got 500
body:
internal server error
[WARNING] spurious network error (2 tries remaining): failed to get successful HTTP response from `http://127.0.0.1:[..]/dl/bar/1.0.0/download` (127.0.0.1), got 500
body:
internal server error
[DOWNLOADED] bar v1.0.0 (registry `dummy-registry`)

"#]]).run();
}

/// Creates a random prefix to randomly spread out the package names
/// to somewhat evenly distribute the different failures at different
/// points.
fn rand_prefix() -> String {
    use rand::Rng;
    const CHARS: &[u8] = b"abcdefghijklmnopqrstuvwxyz";
    let mut rng = rand::rng();
    (0..5)
        .map(|_| CHARS[rng.random_range(0..CHARS.len())] as char)
        .collect()
}

#[cargo_test]
fn dl_retry_multiple() {
    // Tests retry behavior of downloading lots of packages with various
    // failure rates.

    // The index is the number of retries, the value is the number of packages
    // that retry that number of times. Thus 50 packages succeed on first try,
    // 25 on second, etc.
    const RETRIES: &[u32] = &[50, 25, 12, 6];

    let pkgs: Vec<_> = RETRIES
        .iter()
        .enumerate()
        .flat_map(|(retries, num)| {
            (0..*num)
                .into_iter()
                .map(move |n| (retries as u32, format!("{}-{n}-{retries}", rand_prefix())))
        })
        .collect();

    let mut builder = RegistryBuilder::new().http_index();
    let fail_counts: Arc<Mutex<Vec<u32>>> = Arc::new(Mutex::new(vec![0; pkgs.len()]));
    let mut cargo_toml = r#"
        [package]
        name = "foo"
        version = "0.1.0"
        edition = "2015"

        [dependencies]
        "#
    .to_string();
    // The expected stderr output.
    let mut expected = "\
[UPDATING] `dummy-registry` index
[DOWNLOADING] crates ...
"
    .to_string();
    for (n, (retries, name)) in pkgs.iter().enumerate() {
        let count_clone = fail_counts.clone();
        let retries = *retries;
        builder =
            builder.add_responder(format!("/dl/{name}/1.0.0/download"), move |req, server| {
                let mut fail_counts = count_clone.lock().unwrap();
                if fail_counts[n] < retries {
                    fail_counts[n] += 1;
                    server.internal_server_error(req)
                } else {
                    server.dl(req)
                }
            });
        write!(&mut cargo_toml, "{name} = \"1.0.0\"\n").unwrap();
        for retry in 0..retries {
            let remain = 3 - retry;
            write!(
                &mut expected,
                "[WARNING] spurious network error ({remain} {} remaining): \
                failed to get successful HTTP response from \
                `http://127.0.0.1:[..]/dl/{name}/1.0.0/download` (127.0.0.1), got 500\n\
                body:\n\
                internal server error\n",
                if remain != 1 { "tries" } else { "try" }
            )
            .unwrap();
        }
        write!(
            &mut expected,
            "[DOWNLOADED] {name} v1.0.0 (registry `dummy-registry`)\n"
        )
        .unwrap();
    }
    write!(
        &mut expected,
        "[LOCKING] 93 packages to latest compatible versions\n"
    )
    .unwrap();
    let _server = builder.build();
    for (_, name) in &pkgs {
        Package::new(name, "1.0.0").publish();
    }
    let p = project()
        .file("Cargo.toml", &cargo_toml)
        .file("src/lib.rs", "")
        .build();
    p.cargo("fetch")
        .with_stderr_data(IntoData::unordered(expected))
        .run();
}

#[cargo_test]
fn retry_too_many_requests() {
    let fail_count = Mutex::new(0);
    let _registry = RegistryBuilder::new()
        .http_index()
        .add_responder("/index/3/b/bar", move |req, server| {
            let mut fail_count = fail_count.lock().unwrap();
            if *fail_count < 1 {
                *fail_count += 1;
                server.too_many_requests(req, std::time::Duration::from_secs(1))
            } else {
                server.index(req)
            }
        })
        .build();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies]
                bar = ">= 0.0.0"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("bar", "0.0.1").publish();

    p.cargo("check")
    .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[WARNING] spurious network error (3 tries remaining): failed to get successful HTTP response from `[..]/index/3/b/bar` ([..]), got 429
body:
too many requests, try again in 1 seconds
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] bar v0.0.1 (registry `dummy-registry`)
[CHECKING] bar v0.0.1
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]).run();
}

#[cargo_test]
fn deleted_entry() {
    // Checks the behavior when a package is removed from the index.
    // This is done occasionally on crates.io to handle things like
    // copyright takedowns.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

                [dependencies]
                bar = "0.1"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    // First, test removing a single version, but leaving an older version.
    Package::new("bar", "0.1.0").publish();
    let bar_path = Path::new("3/b/bar");
    let bar_reg_path = registry_path().join(&bar_path);
    let old_index = fs::read_to_string(&bar_reg_path).unwrap();
    Package::new("bar", "0.1.1").publish();
    p.cargo("tree")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] bar v0.1.1 (registry `dummy-registry`)

"#]])
        .with_stdout_data(str![[r#"
foo v0.1.0 ([ROOT]/foo)
└── bar v0.1.1

"#]])
        .run();

    // Remove 0.1.1
    fs::remove_file(paths::root().join("dl/bar/0.1.1/download")).unwrap();
    let repo = git2::Repository::open(registry_path()).unwrap();
    let mut index = repo.index().unwrap();
    fs::write(&bar_reg_path, &old_index).unwrap();
    index.add_path(&bar_path).unwrap();
    index.write().unwrap();
    git::commit(&repo);

    // With `Cargo.lock` unchanged, it shouldn't have an impact.
    p.cargo("tree")
        .with_stderr_data("")
        .with_stdout_data(str![[r#"
foo v0.1.0 ([ROOT]/foo)
└── bar v0.1.1

"#]])
        .run();

    // Regenerating Cargo.lock should switch to old version.
    fs::remove_file(p.root().join("Cargo.lock")).unwrap();
    p.cargo("tree")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] bar v0.1.0 (registry `dummy-registry`)

"#]])
        .with_stdout_data(str![[r#"
foo v0.1.0 ([ROOT]/foo)
└── bar v0.1.0

"#]])
        .run();

    // Remove the package entirely.
    fs::remove_file(paths::root().join("dl/bar/0.1.0/download")).unwrap();
    let mut index = repo.index().unwrap();
    index.remove(&bar_path, 0).unwrap();
    index.write().unwrap();
    git::commit(&repo);
    fs::remove_file(&bar_reg_path).unwrap();

    // With `Cargo.lock` unchanged, it shouldn't have an impact.
    p.cargo("tree")
        .with_stderr_data("")
        .with_stdout_data(str![[r#"
foo v0.1.0 ([ROOT]/foo)
└── bar v0.1.0

"#]])
        .run();

    // Regenerating Cargo.lock should fail.
    fs::remove_file(p.root().join("Cargo.lock")).unwrap();
    p.cargo("tree")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[ERROR] no matching package named `bar` found
location searched: `dummy-registry` index (which is replacing registry `crates-io`)
required by package `foo v0.1.0 ([ROOT]/foo)`

"#]])
        .with_status(101)
        .run();
}

#[cargo_test]
fn corrupted_ok_overwritten() {
    // Checks what happens if .cargo-ok gets truncated, such as if the file is
    // created, but the flush/close is interrupted.
    Package::new("bar", "1.0.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

                [dependencies]
                bar = "1"
            "#,
        )
        .file("src/lib.rs", "")
        .build();
    p.cargo("fetch")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] bar v1.0.0 (registry `dummy-registry`)

"#]])
        .run();
    let ok = glob::glob(
        paths::home()
            .join(".cargo/registry/src/*/bar-1.0.0/.cargo-ok")
            .to_str()
            .unwrap(),
    )
    .unwrap()
    .next()
    .unwrap()
    .unwrap();
    // Simulate cargo being interrupted, or filesystem corruption.
    fs::write(&ok, "").unwrap();
    assert_eq!(fs::read_to_string(&ok).unwrap(), "");
    p.cargo("fetch").with_stderr_data("").run();
    assert_eq!(fs::read_to_string(&ok).unwrap(), r#"{"v":1}"#);
}

#[cargo_test]
fn not_found_permutations() {
    // Test for querying permutations for a missing dependency.
    let misses = Arc::new(Mutex::new(Vec::new()));
    let misses2 = misses.clone();
    let _registry = RegistryBuilder::new()
        .http_index()
        .not_found_handler(move |req, _server| {
            let mut misses = misses2.lock().unwrap();
            misses.push(req.url.path().to_string());
            Response {
                code: 404,
                headers: vec![],
                body: b"not found".to_vec(),
            }
        })
        .build();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies]
                a-b_c = "1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[ERROR] no matching package named `a-b_c` found
location searched: `dummy-registry` index (which is replacing registry `crates-io`)
required by package `foo v0.0.1 ([ROOT]/foo)`

"#]])
        .run();
    let mut misses = misses.lock().unwrap();
    misses.sort();
    assert_eq!(
        &*misses,
        &[
            "/index/a-/b-/a-b-c",
            "/index/a-/b_/a-b_c",
            "/index/a_/b_/a_b_c"
        ]
    );
}

#[cargo_test]
fn default_auth_error() {
    // Check for the error message for an authentication error when default is set.
    let crates_io = RegistryBuilder::new().http_api().build();
    let _alternative = RegistryBuilder::new().http_api().alternative().build();

    paths::home().join(".cargo/credentials.toml").rm_rf();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"
                license = "MIT"
                description = "foo"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    // Test output before setting the default.
    p.cargo("publish --no-verify")
        .replace_crates_io(crates_io.index_url())
        .with_stderr_data(str![[r#"
[UPDATING] crates.io index
[ERROR] no token found, please run `cargo login`
or use environment variable CARGO_REGISTRY_TOKEN

"#]])
        .with_status(101)
        .run();

    p.cargo("publish --no-verify --registry alternative")
        .replace_crates_io(crates_io.index_url())
        .with_stderr_data(str![[r#"
[UPDATING] `alternative` index
[ERROR] no token found for `alternative`, please run `cargo login --registry alternative`
or use environment variable CARGO_REGISTRIES_ALTERNATIVE_TOKEN

"#]])
        .with_status(101)
        .run();

    // Test the output with the default.
    cargo_util::paths::append(
        &paths::cargo_home().join("config.toml"),
        br#"
            [registry]
            default = "alternative"
        "#,
    )
    .unwrap();

    p.cargo("publish --no-verify")
        .replace_crates_io(crates_io.index_url())
        .with_stderr_data(str![[r#"
[UPDATING] `alternative` index
[ERROR] no token found for `alternative`, please run `cargo login --registry alternative`
or use environment variable CARGO_REGISTRIES_ALTERNATIVE_TOKEN

"#]])
        .with_status(101)
        .run();

    p.cargo("publish --no-verify --registry crates-io")
        .replace_crates_io(crates_io.index_url())
        .with_stderr_data(str![[r#"
[UPDATING] crates.io index
[ERROR] no token found, please run `cargo login --registry crates-io`
or use environment variable CARGO_REGISTRY_TOKEN

"#]])
        .with_status(101)
        .run();
}

const SAMPLE_HEADERS: &[&str] = &[
    "x-amz-cf-pop: SFO53-P2",
    "x-amz-cf-id: vEc3osJrCAXVaciNnF4Vev-hZFgnYwmNZtxMKRJ5bF6h9FTOtbTMnA==",
    "x-cache: Hit from cloudfront",
    "server: AmazonS3",
    "x-amz-version-id: pvsJYY_JGsWiSETZvLJKb7DeEW5wWq1W",
    "x-amz-server-side-encryption: AES256",
    "content-type: text/plain",
    "via: 1.1 bcbc5b46216015493e082cfbcf77ef10.cloudfront.net (CloudFront)",
];

#[cargo_test]
fn debug_header_message_index() {
    // The error message should include some headers for debugging purposes.
    let _server = RegistryBuilder::new()
        .http_index()
        .add_responder("/index/3/b/bar", |_, _| Response {
            code: 503,
            headers: SAMPLE_HEADERS.iter().map(|s| s.to_string()).collect(),
            body: b"Please slow down".to_vec(),
        })
        .build();
    Package::new("bar", "1.0.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

                [dependencies]
                bar = "1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .build();
    p.cargo("fetch")
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[WARNING] spurious network error (3 tries remaining): failed to get successful HTTP response from `http://127.0.0.1:[..]/index/3/b/bar` (127.0.0.1), got 503
body:
Please slow down
[WARNING] spurious network error (2 tries remaining): failed to get successful HTTP response from `http://127.0.0.1:[..]/index/3/b/bar` (127.0.0.1), got 503
body:
Please slow down
[WARNING] spurious network error (1 try remaining): failed to get successful HTTP response from `http://127.0.0.1:[..]/index/3/b/bar` (127.0.0.1), got 503
body:
Please slow down
[ERROR] failed to get `bar` as a dependency of package `foo v0.1.0 ([ROOT]/foo)`

Caused by:
  failed to query replaced source registry `crates-io`

Caused by:
  download of 3/b/bar failed

Caused by:
  failed to get successful HTTP response from `http://127.0.0.1:[..]/index/3/b/bar` (127.0.0.1), got 503
  debug headers:
  x-amz-cf-pop: SFO53-P2
  x-amz-cf-id: vEc3osJrCAXVaciNnF4Vev-hZFgnYwmNZtxMKRJ5bF6h9FTOtbTMnA==
  x-cache: Hit from cloudfront
  body:
  Please slow down

"#]])
        .run();
}

#[cargo_test]
fn debug_header_message_dl() {
    // Same as debug_header_message_index, but for the dl endpoint which goes
    // through a completely different code path.
    let _server = RegistryBuilder::new()
        .http_index()
        .add_responder("/dl/bar/1.0.0/download", |_, _| Response {
            code: 503,
            headers: SAMPLE_HEADERS.iter().map(|s| s.to_string()).collect(),
            body: b"Please slow down".to_vec(),
        })
        .build();
    Package::new("bar", "1.0.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

                [dependencies]
                bar = "1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("fetch")
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[WARNING] spurious network error (3 tries remaining): failed to get successful HTTP response from `http://127.0.0.1:[..]/dl/bar/1.0.0/download` (127.0.0.1), got 503
body:
Please slow down
[WARNING] spurious network error (2 tries remaining): failed to get successful HTTP response from `http://127.0.0.1:[..]/dl/bar/1.0.0/download` (127.0.0.1), got 503
body:
Please slow down
[WARNING] spurious network error (1 try remaining): failed to get successful HTTP response from `http://127.0.0.1:[..]/dl/bar/1.0.0/download` (127.0.0.1), got 503
body:
Please slow down
[ERROR] failed to download from `http://127.0.0.1:[..]/dl/bar/1.0.0/download`

Caused by:
  failed to get successful HTTP response from `http://127.0.0.1:[..]/dl/bar/1.0.0/download` (127.0.0.1), got 503
  debug headers:
  x-amz-cf-pop: SFO53-P2
  x-amz-cf-id: vEc3osJrCAXVaciNnF4Vev-hZFgnYwmNZtxMKRJ5bF6h9FTOtbTMnA==
  x-cache: Hit from cloudfront
  body:
  Please slow down

"#]])
        .run();
}

#[cfg(unix)]
#[cargo_test]
fn set_mask_during_unpacking() {
    use std::os::unix::fs::MetadataExt;

    Package::new("bar", "1.0.0")
        .file_with_mode("example.sh", 0o777, "#!/bin/sh")
        .file_with_mode("src/lib.rs", 0o666, "")
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

                [dependencies]
                bar = "1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("fetch")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] bar v1.0.0 (registry `dummy-registry`)

"#]])
        .run();
    let src_file_path = |path: &str| {
        glob::glob(
            paths::home()
                .join(".cargo/registry/src/*/bar-1.0.0/")
                .join(path)
                .to_str()
                .unwrap(),
        )
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
    };

    let umask = cargo::util::get_umask();
    let metadata = fs::metadata(src_file_path("src/lib.rs")).unwrap();
    assert_eq!(metadata.mode() & 0o777, 0o666 & !umask);
    let metadata = fs::metadata(src_file_path("example.sh")).unwrap();
    assert_eq!(metadata.mode() & 0o777, 0o777 & !umask);
}

#[cargo_test]
fn unpack_again_when_cargo_ok_is_unrecognized() {
    Package::new("bar", "1.0.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

                [dependencies]
                bar = "1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("fetch")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] bar v1.0.0 (registry `dummy-registry`)

"#]])
        .run();

    let src_file_path = |path: &str| {
        glob::glob(
            paths::home()
                .join(".cargo/registry/src/*/bar-1.0.0/")
                .join(path)
                .to_str()
                .unwrap(),
        )
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
    };

    // Change permissions to simulate the old behavior not respecting umask.
    let lib_rs = src_file_path("src/lib.rs");
    let cargo_ok = src_file_path(".cargo-ok");
    let mut perms = fs::metadata(&lib_rs).unwrap().permissions();
    assert!(!perms.readonly());
    perms.set_readonly(true);
    fs::set_permissions(&lib_rs, perms).unwrap();
    let ok = fs::read_to_string(&cargo_ok).unwrap();
    assert_eq!(&ok, r#"{"v":1}"#);

    p.cargo("fetch").with_stderr_data("").run();

    // Without changing `.cargo-ok`, a unpack won't be triggered.
    let perms = fs::metadata(&lib_rs).unwrap().permissions();
    assert!(perms.readonly());

    // Write "ok" to simulate the old behavior and trigger the unpack again.
    fs::write(&cargo_ok, "ok").unwrap();

    p.cargo("fetch").with_stderr_data("").run();

    // Permission has been restored and `.cargo-ok` is in the new format.
    let perms = fs::metadata(lib_rs).unwrap().permissions();
    assert!(!perms.readonly());
    let ok = fs::read_to_string(&cargo_ok).unwrap();
    assert_eq!(&ok, r#"{"v":1}"#);
}

#[cargo_test]
fn differ_only_by_metadata() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies]
                baz = "=0.0.1"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("baz", "0.0.1+b").publish();
    Package::new("baz", "0.0.1+c").yanked(true).publish();

    p.cargo("check")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] baz v0.0.1+b (registry `dummy-registry`)
[CHECKING] baz v0.0.1+b
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    Package::new("baz", "0.0.1+d").publish();

    p.cargo("clean").run();
    p.cargo("check")
        .with_stderr_data(str![[r#"
[CHECKING] baz v0.0.1+b
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn differ_only_by_metadata_with_lockfile() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []

                [dependencies]
                baz = "=0.0.1"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("baz", "0.0.1+a").publish();
    Package::new("baz", "0.0.1+b").publish();
    Package::new("baz", "0.0.1+c").publish();

    p.cargo("update --package baz --precise 0.0.1+b")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[UPDATING] baz v0.0.1+c -> v0.0.1+b

"#]])
        .run();

    p.cargo("check")
        .with_stderr_data(str![[r#"
[DOWNLOADING] crates ...
[DOWNLOADED] baz v0.0.1+b (registry `dummy-registry`)
[CHECKING] baz v0.0.1+b
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn builtin_source_replacement() {
    // errors for builtin source replacement of crates.io
    // should not include mention of source replacement in the error message.
    let server = RegistryBuilder::new().build();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"

                [dependencies]
                bad-cksum = ">= 0.0.0"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    let pkg = Package::new("bad-cksum", "0.0.1");
    pkg.publish();
    t!(File::create(&pkg.archive_dst()));

    p.cargo("check -v")
        .replace_crates_io(&server.index_url())
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] crates.io index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] bad-cksum v0.0.1
[ERROR] failed to verify the checksum of `bad-cksum v0.0.1`

"#]])
        .run();
}

#[cargo_test]
fn builtin_source_replacement_no_vendor_error() {
    // errors for builtin source replacement of crates.io
    // should not mention outdated vendor dependencies
    let server = RegistryBuilder::new().build();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2021"

                [dependencies]
                dep = "0.2.0"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    let pkg = Package::new("dep", "0.1.0");
    pkg.publish();

    p.cargo("check -v")
        .replace_crates_io(&server.index_url())
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] crates.io index
[ERROR] failed to select a version for the requirement `dep = "^0.2.0"`
candidate versions found which didn't match: 0.1.0
location searched: crates.io index
required by package `foo v0.0.1 ([ROOT]/foo)`

"#]])
        .run();
}
