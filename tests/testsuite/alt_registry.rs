//! Tests for alternative registries.

use std::fs;

use crate::prelude::*;
use cargo_test_support::compare::assert_e2e;
use cargo_test_support::publish::validate_alt_upload;
use cargo_test_support::registry::{self, Package, RegistryBuilder};
use cargo_test_support::str;
use cargo_test_support::{basic_manifest, paths, project};

#[cargo_test]
fn depend_on_alt_registry() {
    registry::alt_init();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []
                edition = "2015"

                [dependencies.bar]
                version = "0.0.1"
                registry = "alternative"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("bar", "0.0.1").alternative(true).publish();

    p.cargo("check")
        .with_stderr_data(str![[r#"
[UPDATING] `alternative` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] bar v0.0.1 (registry `alternative`)
[CHECKING] bar v0.0.1 (registry `alternative`)
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("clean").run();

    // Don't download a second time
    p.cargo("check")
        .with_stderr_data(str![[r#"
[CHECKING] bar v0.0.1 (registry `alternative`)
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn depend_on_alt_registry_depends_on_same_registry_no_index() {
    registry::alt_init();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []
                edition = "2015"

                [dependencies.bar]
                version = "0.0.1"
                registry = "alternative"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("baz", "0.0.1").alternative(true).publish();
    Package::new("bar", "0.0.1")
        .registry_dep("baz", "0.0.1")
        .alternative(true)
        .publish();

    p.cargo("check")
        .with_stderr_data(str![[r#"
[UPDATING] `alternative` index
[LOCKING] 2 packages to latest compatible versions
[DOWNLOADING] crates ...
[DOWNLOADED] baz v0.0.1 (registry `alternative`)
[DOWNLOADED] bar v0.0.1 (registry `alternative`)
[CHECKING] baz v0.0.1 (registry `alternative`)
[CHECKING] bar v0.0.1 (registry `alternative`)
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn depend_on_alt_registry_depends_on_same_registry() {
    registry::alt_init();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []
                edition = "2015"

                [dependencies.bar]
                version = "0.0.1"
                registry = "alternative"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("baz", "0.0.1").alternative(true).publish();
    Package::new("bar", "0.0.1")
        .registry_dep("baz", "0.0.1")
        .alternative(true)
        .publish();

    p.cargo("check")
        .with_stderr_data(str![[r#"
[UPDATING] `alternative` index
[LOCKING] 2 packages to latest compatible versions
[DOWNLOADING] crates ...
[DOWNLOADED] baz v0.0.1 (registry `alternative`)
[DOWNLOADED] bar v0.0.1 (registry `alternative`)
[CHECKING] baz v0.0.1 (registry `alternative`)
[CHECKING] bar v0.0.1 (registry `alternative`)
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn depend_on_alt_registry_depends_on_crates_io() {
    registry::alt_init();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []
                edition = "2015"

                [dependencies.bar]
                version = "0.0.1"
                registry = "alternative"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("baz", "0.0.1").publish();
    Package::new("bar", "0.0.1")
        .dep("baz", "0.0.1")
        .alternative(true)
        .publish();

    p.cargo("check")
        .with_stderr_data(
            str![[r#"
[UPDATING] `alternative` index
[UPDATING] `dummy-registry` index
[LOCKING] 2 packages to latest compatible versions
[DOWNLOADING] crates ...
[DOWNLOADED] baz v0.0.1 (registry `dummy-registry`)
[DOWNLOADED] bar v0.0.1 (registry `alternative`)
[CHECKING] baz v0.0.1
[CHECKING] bar v0.0.1 (registry `alternative`)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[CHECKING] foo v0.0.1 ([ROOT]/foo)

"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test]
fn registry_and_path_dep_works() {
    registry::alt_init();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []
                edition = "2015"

                [dependencies.bar]
                path = "bar"
                registry = "alternative"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file("bar/src/lib.rs", "")
        .build();

    p.cargo("check")
        .with_stderr_data(str![[r#"
[LOCKING] 1 package to latest compatible version
[CHECKING] bar v0.0.1 ([ROOT]/foo/bar)
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn registry_incompatible_with_git() {
    registry::alt_init();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []
                edition = "2015"

                [dependencies.bar]
                git = ""
                registry = "alternative"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  dependency (bar) specification is ambiguous. Only one of `git` or `registry` is allowed.

"#]])
        .run();
}

#[cargo_test]
fn cannot_publish_to_crates_io_with_registry_dependency() {
    let crates_io = registry::init();
    let _alternative = RegistryBuilder::new().alternative().build();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []
                edition = "2015"
                [dependencies.bar]
                version = "0.0.1"
                registry = "alternative"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("bar", "0.0.1").alternative(true).publish();

    p.cargo("publish")
        .replace_crates_io(crates_io.index_url())
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] crates.io index
[ERROR] failed to verify manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  crates cannot be published to crates.io with dependencies sourced from other
  registries. `bar` needs to be published to crates.io before publishing this crate.
  (crate `bar` is pulled from registry `alternative`)

"#]])
        .run();

    p.cargo("publish")
        .replace_crates_io(crates_io.index_url())
        .arg("--token")
        .arg(crates_io.token())
        .arg("--index")
        .arg(crates_io.index_url().as_str())
        .with_status(101)
        .with_stderr_data(str![[r#"
[WARNING] `cargo publish --token` is deprecated in favor of using `cargo login` and environment variables
[UPDATING] crates.io index
[ERROR] failed to verify manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  crates cannot be published to crates.io with dependencies sourced from other
  registries. `bar` needs to be published to crates.io before publishing this crate.
  (crate `bar` is pulled from registry `alternative`)

"#]])
        .run();
}

#[cargo_test]
fn publish_with_registry_dependency() {
    let _reg = RegistryBuilder::new()
        .http_api()
        .http_index()
        .alternative()
        .build();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []
                edition = "2015"

                [dependencies.bar]
                version = "0.0.1"
                registry = "alternative"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("bar", "0.0.1").alternative(true).publish();

    p.cargo("publish --registry alternative")
        .with_stderr_data(str![[r#"
[UPDATING] `alternative` index
[WARNING] manifest has no description, license, license-file, documentation, homepage or repository
  |
  = [NOTE] see https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info
[PACKAGING] foo v0.0.1 ([ROOT]/foo)
[UPDATING] `alternative` index
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] foo v0.0.1 ([ROOT]/foo)
[DOWNLOADING] crates ...
[DOWNLOADED] bar v0.0.1 (registry `alternative`)
[COMPILING] bar v0.0.1 (registry `alternative`)
[COMPILING] foo v0.0.1 ([ROOT]/foo/target/package/foo-0.0.1)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[UPLOADING] foo v0.0.1 ([ROOT]/foo)
[UPLOADED] foo v0.0.1 to registry `alternative`
[NOTE] waiting for foo v0.0.1 to be available at registry `alternative`
[HELP] you may press ctrl-c to skip waiting; the crate should be available shortly
[PUBLISHED] foo v0.0.1 at registry `alternative`

"#]])
        .run();

    validate_alt_upload(
        r#"{
            "authors": [],
            "badges": {},
            "categories": [],
            "deps": [
              {
                "default_features": true,
                "features": [],
                "kind": "normal",
                "name": "bar",
                "optional": false,
                "target": null,
                "version_req": "^0.0.1"
              }
            ],
            "description": null,
            "documentation": null,
            "features": {},
            "homepage": null,
            "keywords": [],
            "license": null,
            "license_file": null,
            "links": null,
            "name": "foo",
            "readme": null,
            "readme_file": null,
            "repository": null,
            "homepage": null,
            "documentation": null,
            "rust_version": null,
            "vers": "0.0.1"
        }"#,
        "foo-0.0.1.crate",
        &["Cargo.lock", "Cargo.toml", "Cargo.toml.orig", "src/main.rs"],
    );
}

#[cargo_test]
fn alt_registry_and_crates_io_deps() {
    registry::alt_init();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []
                edition = "2015"

                [dependencies]
                crates_io_dep = "0.0.1"

                [dependencies.alt_reg_dep]
                version = "0.1.0"
                registry = "alternative"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("crates_io_dep", "0.0.1").publish();
    Package::new("alt_reg_dep", "0.1.0")
        .alternative(true)
        .publish();

    p.cargo("check")
        .with_stderr_data(
            str![[r#"
[UPDATING] `alternative` index
[UPDATING] `dummy-registry` index
[LOCKING] 2 packages to latest compatible versions
[DOWNLOADING] crates ...
[DOWNLOADED] crates_io_dep v0.0.1 (registry `dummy-registry`)
[DOWNLOADED] alt_reg_dep v0.1.0 (registry `alternative`)
[CHECKING] crates_io_dep v0.0.1
[CHECKING] alt_reg_dep v0.1.0 (registry `alternative`)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[CHECKING] foo v0.0.1 ([ROOT]/foo)

"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test]
fn block_publish_due_to_no_token() {
    registry::alt_init();
    let p = project().file("src/lib.rs", "").build();

    fs::remove_file(paths::home().join(".cargo/credentials.toml")).unwrap();

    // Now perform the actual publish
    p.cargo("publish --registry alternative")
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] `alternative` index
[ERROR] no token found for `alternative`, please run `cargo login --registry alternative`
or use environment variable CARGO_REGISTRIES_ALTERNATIVE_TOKEN

"#]])
        .run();
}

#[cargo_test]
fn cargo_registries_crates_io_protocol() {
    let _ = RegistryBuilder::new()
        .no_configure_token()
        .alternative()
        .build();
    // Should not produce a warning due to the registries.crates-io.protocol = 'sparse' configuration
    let p = project()
        .file("src/lib.rs", "")
        .file(
            ".cargo/config.toml",
            "[registries.crates-io]
            protocol = 'sparse'",
        )
        .build();

    p.cargo("publish --registry alternative")
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] `alternative` index
[ERROR] no token found for `alternative`, please run `cargo login --registry alternative`
or use environment variable CARGO_REGISTRIES_ALTERNATIVE_TOKEN

"#]])
        .run();
}

#[cargo_test]
fn publish_to_alt_registry() {
    let _reg = RegistryBuilder::new()
        .http_api()
        .http_index()
        .alternative()
        .build();

    let p = project().file("src/main.rs", "fn main() {}").build();

    // Now perform the actual publish
    p.cargo("publish --registry alternative")
        .with_stderr_data(str![[r#"
[UPDATING] `alternative` index
[WARNING] manifest has no description, license, license-file, documentation, homepage or repository
  |
  = [NOTE] see https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info
[PACKAGING] foo v0.0.1 ([ROOT]/foo)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] foo v0.0.1 ([ROOT]/foo)
[COMPILING] foo v0.0.1 ([ROOT]/foo/target/package/foo-0.0.1)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[UPLOADING] foo v0.0.1 ([ROOT]/foo)
[UPLOADED] foo v0.0.1 to registry `alternative`
[NOTE] waiting for foo v0.0.1 to be available at registry `alternative`
[HELP] you may press ctrl-c to skip waiting; the crate should be available shortly
[PUBLISHED] foo v0.0.1 at registry `alternative`

"#]])
        .run();

    validate_alt_upload(
        r#"{
            "authors": [],
            "badges": {},
            "categories": [],
            "deps": [],
            "description": null,
            "documentation": null,
            "features": {},
            "homepage": null,
            "keywords": [],
            "license": null,
            "license_file": null,
            "links": null,
            "name": "foo",
            "readme": null,
            "readme_file": null,
            "repository": null,
            "homepage": null,
            "documentation": null,
            "rust_version": null,
            "vers": "0.0.1"
        }"#,
        "foo-0.0.1.crate",
        &["Cargo.lock", "Cargo.toml", "Cargo.toml.orig", "src/main.rs"],
    );
}

#[cargo_test]
fn publish_with_crates_io_dep() {
    // crates.io registry.
    let _dummy_reg = registry::init();
    // Alternative registry.
    let _alt_reg = RegistryBuilder::new()
        .http_api()
        .http_index()
        .alternative()
        .build();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = ["me"]
                edition = "2015"
                license = "MIT"
                description = "foo"

                [dependencies.bar]
                version = "0.0.1"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("bar", "0.0.1").publish();

    p.cargo("publish --registry alternative")
        .with_stderr_data(str![[r#"
[UPDATING] `alternative` index
[WARNING] manifest has no documentation, homepage or repository
  |
  = [NOTE] see https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info
[PACKAGING] foo v0.0.1 ([ROOT]/foo)
[UPDATING] `dummy-registry` index
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] foo v0.0.1 ([ROOT]/foo)
[DOWNLOADING] crates ...
[DOWNLOADED] bar v0.0.1 (registry `dummy-registry`)
[COMPILING] bar v0.0.1
[COMPILING] foo v0.0.1 ([ROOT]/foo/target/package/foo-0.0.1)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[UPLOADING] foo v0.0.1 ([ROOT]/foo)
[UPLOADED] foo v0.0.1 to registry `alternative`
[NOTE] waiting for foo v0.0.1 to be available at registry `alternative`
[HELP] you may press ctrl-c to skip waiting; the crate should be available shortly
[PUBLISHED] foo v0.0.1 at registry `alternative`

"#]])
        .run();

    validate_alt_upload(
        r#"{
            "authors": ["me"],
            "badges": {},
            "categories": [],
            "deps": [
              {
                "default_features": true,
                "features": [],
                "kind": "normal",
                "name": "bar",
                "optional": false,
                "registry": "https://github.com/rust-lang/crates.io-index",
                "target": null,
                "version_req": "^0.0.1"
              }
            ],
            "description": "foo",
            "documentation": null,
            "features": {},
            "homepage": null,
            "keywords": [],
            "license": "MIT",
            "license_file": null,
            "links": null,
            "name": "foo",
            "readme": null,
            "readme_file": null,
            "repository": null,
            "homepage": null,
            "documentation": null,
            "rust_version": null,
            "vers": "0.0.1"
        }"#,
        "foo-0.0.1.crate",
        &["Cargo.lock", "Cargo.toml", "Cargo.toml.orig", "src/main.rs"],
    );
}

#[cargo_test]
fn passwords_in_registries_index_url_forbidden() {
    registry::alt_init();

    let config = paths::home().join(".cargo/config.toml");

    fs::write(
        config,
        r#"
        [registries.alternative]
        index = "ssh://git:secret@foobar.com"
        "#,
    )
    .unwrap();

    let p = project().file("src/main.rs", "fn main() {}").build();

    p.cargo("publish --registry alternative")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] invalid index URL for registry `alternative` defined in [ROOT]/home/.cargo/config.toml

Caused by:
  registry URLs may not contain passwords

"#]])
        .run();
}

#[cargo_test]
fn patch_alt_reg() {
    registry::alt_init();
    Package::new("bar", "0.1.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"

                [dependencies]
                bar = { version = "0.1.0", registry = "alternative" }

                [patch.alternative]
                bar = { path = "bar" }
            "#,
        )
        .file(
            "src/lib.rs",
            "
            extern crate bar;
            pub fn f() { bar::bar(); }
            ",
        )
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.1.0"))
        .file("bar/src/lib.rs", "pub fn bar() {}")
        .build();

    p.cargo("check")
        .with_stderr_data(str![[r#"
[UPDATING] `alternative` index
[LOCKING] 1 package to latest compatible version
[CHECKING] bar v0.1.0 ([ROOT]/foo/bar)
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn bad_registry_name() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []
                edition = "2015"

                [dependencies.bar]
                version = "0.0.1"
                registry = "bad name"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("build")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] invalid character ` ` in registry name: `bad name`, characters must be Unicode XID characters (numbers, `-`, `_`, or most letters)
       
       
 --> Cargo.toml:8:17
  |
8 |                 [dependencies.bar]
  |                 ^^^^^^^^^^^^^^^^^^

"#]])
        .run();

    for cmd in &[
        "init",
        "install foo",
        "login",
        "owner",
        "publish",
        "search",
        "yank --version 0.0.1",
    ] {
        p.cargo(cmd)
            .arg("--registry")
            .arg("bad name")
            .with_status(101)
            .with_stderr_data(str![[r#"
[ERROR] invalid character ` ` in registry name: `bad name`, characters must be Unicode XID characters (numbers, `-`, `_`, or most letters)

"#]])
            .run();
    }
}

#[cargo_test]
fn no_api() {
    let _registry = RegistryBuilder::new().alternative().no_api().build();
    Package::new("bar", "0.0.1").alternative(true).publish();

    // First check that a dependency works.
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"

                [dependencies.bar]
                version = "0.0.1"
                registry = "alternative"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check")
        .with_stderr_data(str![[r#"
[UPDATING] `alternative` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] bar v0.0.1 (registry `alternative`)
[CHECKING] bar v0.0.1 (registry `alternative`)
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("login --registry alternative")
        .with_stdin("TOKEN")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] registry `alternative` does not support API commands

"#]])
        .run();

    p.cargo("publish --registry alternative")
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] `alternative` index
[ERROR] registry `alternative` does not support API commands

"#]])
        .run();

    p.cargo("search --registry alternative")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] registry `alternative` does not support API commands

"#]])
        .run();

    p.cargo("owner --registry alternative --list")
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] `alternative` index
[ERROR] registry `alternative` does not support API commands

"#]])
        .run();

    p.cargo("yank --registry alternative --version=0.0.1 bar")
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] `alternative` index
[ERROR] registry `alternative` does not support API commands

"#]])
        .run();

    p.cargo("yank --registry alternative --version=0.0.1 bar")
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] `alternative` index
[ERROR] registry `alternative` does not support API commands

"#]])
        .run();
}

#[cargo_test]
fn alt_reg_metadata() {
    // Check for "registry" entries in `cargo metadata` with alternative registries.
    registry::alt_init();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"

                [dependencies]
                altdep = { version = "0.0.1", registry = "alternative" }
                iodep = { version = "0.0.1" }
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    Package::new("bar", "0.0.1").publish();
    Package::new("altdep", "0.0.1")
        .dep("bar", "0.0.1")
        .alternative(true)
        .publish();
    Package::new("altdep2", "0.0.1").alternative(true).publish();
    Package::new("iodep", "0.0.1")
        .registry_dep("altdep2", "0.0.1")
        .publish();

    // The important thing to check here is the "registry" value in `deps`.
    // They should be:
    // foo -> altdep: alternative-registry
    // foo -> iodep: null (because it is in crates.io)
    // altdep -> bar: null (because it is in crates.io)
    // iodep -> altdep2: alternative-registry
    p.cargo("metadata --format-version=1 --no-deps")
        .with_stdout_data(
            str![[r#"
{
  "metadata": null,
  "packages": [
    {
      "authors": [],
      "categories": [],
      "default_run": null,
      "dependencies": [
        {
          "features": [],
          "kind": null,
          "name": "altdep",
          "optional": false,
          "registry": "[ROOTURL]/alternative-registry",
          "rename": null,
          "req": "^0.0.1",
          "source": "registry+[ROOTURL]/alternative-registry",
          "target": null,
          "uses_default_features": true
        },
        {
          "features": [],
          "kind": null,
          "name": "iodep",
          "optional": false,
          "registry": null,
          "rename": null,
          "req": "^0.0.1",
          "source": "registry+https://github.com/rust-lang/crates.io-index",
          "target": null,
          "uses_default_features": true
        }
      ],
      "description": null,
      "documentation": null,
      "edition": "2015",
      "features": {},
      "homepage": null,
      "id": "path+[ROOTURL]/foo#0.0.1",
      "keywords": [],
      "license": null,
      "license_file": null,
      "links": null,
      "manifest_path": "[ROOT]/foo/Cargo.toml",
      "metadata": null,
      "name": "foo",
      "publish": null,
      "readme": null,
      "repository": null,
      "rust_version": null,
      "source": null,
      "targets": [
        {
          "crate_types": [
            "lib"
          ],
          "doc": true,
          "doctest": true,
          "edition": "2015",
          "kind": [
            "lib"
          ],
          "name": "foo",
          "src_path": "[ROOT]/foo/src/lib.rs",
          "test": true
        }
      ],
      "version": "0.0.1"
    }
  ],
  "resolve": null,
  "target_directory": "[ROOT]/foo/target",
  "build_directory": "[ROOT]/foo/target",
  "version": 1,
  "workspace_default_members": [
    "path+[ROOTURL]/foo#0.0.1"
  ],
  "workspace_members": [
    "path+[ROOTURL]/foo#0.0.1"
  ],
  "workspace_root": "[ROOT]/foo"
}
"#]]
            .is_json(),
        )
        .run();

    // --no-deps uses a different code path, make sure both work.
    p.cargo("metadata --format-version=1")
        .with_stdout_data(
            str![[r#"
{
  "metadata": null,
  "packages": [
    {
      "authors": [],
      "categories": [],
      "default_run": null,
      "dependencies": [
        {
          "features": [],
          "kind": null,
          "name": "bar",
          "optional": false,
          "registry": null,
          "rename": null,
          "req": "^0.0.1",
          "source": "registry+https://github.com/rust-lang/crates.io-index",
          "target": null,
          "uses_default_features": true
        }
      ],
      "description": null,
      "documentation": null,
      "edition": "2015",
      "features": {},
      "homepage": null,
      "id": "registry+[ROOTURL]/alternative-registry#altdep@0.0.1",
      "keywords": [],
      "license": null,
      "license_file": null,
      "links": null,
      "manifest_path": "[ROOT]/home/.cargo/registry/src/-[HASH]/altdep-0.0.1/Cargo.toml",
      "metadata": null,
      "name": "altdep",
      "publish": null,
      "readme": null,
      "repository": null,
      "rust_version": null,
      "source": "registry+[ROOTURL]/alternative-registry",
      "targets": [
        {
          "crate_types": [
            "lib"
          ],
          "doc": true,
          "doctest": true,
          "edition": "2015",
          "kind": [
            "lib"
          ],
          "name": "altdep",
          "src_path": "[ROOT]/home/.cargo/registry/src/-[HASH]/altdep-0.0.1/src/lib.rs",
          "test": true
        }
      ],
      "version": "0.0.1"
    },
    {
      "authors": [],
      "categories": [],
      "default_run": null,
      "dependencies": [],
      "description": null,
      "documentation": null,
      "edition": "2015",
      "features": {},
      "homepage": null,
      "id": "registry+[ROOTURL]/alternative-registry#altdep2@0.0.1",
      "keywords": [],
      "license": null,
      "license_file": null,
      "links": null,
      "manifest_path": "[ROOT]/home/.cargo/registry/src/-[HASH]/altdep2-0.0.1/Cargo.toml",
      "metadata": null,
      "name": "altdep2",
      "publish": null,
      "readme": null,
      "repository": null,
      "rust_version": null,
      "source": "registry+[ROOTURL]/alternative-registry",
      "targets": [
        {
          "crate_types": [
            "lib"
          ],
          "doc": true,
          "doctest": true,
          "edition": "2015",
          "kind": [
            "lib"
          ],
          "name": "altdep2",
          "src_path": "[ROOT]/home/.cargo/registry/src/-[HASH]/altdep2-0.0.1/src/lib.rs",
          "test": true
        }
      ],
      "version": "0.0.1"
    },
    {
      "authors": [],
      "categories": [],
      "default_run": null,
      "dependencies": [],
      "description": null,
      "documentation": null,
      "edition": "2015",
      "features": {},
      "homepage": null,
      "id": "registry+https://github.com/rust-lang/crates.io-index#bar@0.0.1",
      "keywords": [],
      "license": null,
      "license_file": null,
      "links": null,
      "manifest_path": "[ROOT]/home/.cargo/registry/src/-[HASH]/bar-0.0.1/Cargo.toml",
      "metadata": null,
      "name": "bar",
      "publish": null,
      "readme": null,
      "repository": null,
      "rust_version": null,
      "source": "registry+https://github.com/rust-lang/crates.io-index",
      "targets": [
        {
          "crate_types": [
            "lib"
          ],
          "doc": true,
          "doctest": true,
          "edition": "2015",
          "kind": [
            "lib"
          ],
          "name": "bar",
          "src_path": "[ROOT]/home/.cargo/registry/src/-[HASH]/bar-0.0.1/src/lib.rs",
          "test": true
        }
      ],
      "version": "0.0.1"
    },
    {
      "authors": [],
      "categories": [],
      "default_run": null,
      "dependencies": [
        {
          "features": [],
          "kind": null,
          "name": "altdep",
          "optional": false,
          "registry": "[ROOTURL]/alternative-registry",
          "rename": null,
          "req": "^0.0.1",
          "source": "registry+[ROOTURL]/alternative-registry",
          "target": null,
          "uses_default_features": true
        },
        {
          "features": [],
          "kind": null,
          "name": "iodep",
          "optional": false,
          "registry": null,
          "rename": null,
          "req": "^0.0.1",
          "source": "registry+https://github.com/rust-lang/crates.io-index",
          "target": null,
          "uses_default_features": true
        }
      ],
      "description": null,
      "documentation": null,
      "edition": "2015",
      "features": {},
      "homepage": null,
      "id": "path+[ROOTURL]/foo#0.0.1",
      "keywords": [],
      "license": null,
      "license_file": null,
      "links": null,
      "manifest_path": "[ROOT]/foo/Cargo.toml",
      "metadata": null,
      "name": "foo",
      "publish": null,
      "readme": null,
      "repository": null,
      "rust_version": null,
      "source": null,
      "targets": [
        {
          "crate_types": [
            "lib"
          ],
          "doc": true,
          "doctest": true,
          "edition": "2015",
          "kind": [
            "lib"
          ],
          "name": "foo",
          "src_path": "[ROOT]/foo/src/lib.rs",
          "test": true
        }
      ],
      "version": "0.0.1"
    },
    {
      "authors": [],
      "categories": [],
      "default_run": null,
      "dependencies": [
        {
          "features": [],
          "kind": null,
          "name": "altdep2",
          "optional": false,
          "registry": "[ROOTURL]/alternative-registry",
          "rename": null,
          "req": "^0.0.1",
          "source": "registry+[ROOTURL]/alternative-registry",
          "target": null,
          "uses_default_features": true
        }
      ],
      "description": null,
      "documentation": null,
      "edition": "2015",
      "features": {},
      "homepage": null,
      "id": "registry+https://github.com/rust-lang/crates.io-index#iodep@0.0.1",
      "keywords": [],
      "license": null,
      "license_file": null,
      "links": null,
      "manifest_path": "[ROOT]/home/.cargo/registry/src/-[HASH]/iodep-0.0.1/Cargo.toml",
      "metadata": null,
      "name": "iodep",
      "publish": null,
      "readme": null,
      "repository": null,
      "rust_version": null,
      "source": "registry+https://github.com/rust-lang/crates.io-index",
      "targets": [
        {
          "crate_types": [
            "lib"
          ],
          "doc": true,
          "doctest": true,
          "edition": "2015",
          "kind": [
            "lib"
          ],
          "name": "iodep",
          "src_path": "[ROOT]/home/.cargo/registry/src/-[HASH]/iodep-0.0.1/src/lib.rs",
          "test": true
        }
      ],
      "version": "0.0.1"
    }
  ],
  "resolve": {
    "nodes": [
      {
        "dependencies": [
          "registry+https://github.com/rust-lang/crates.io-index#bar@0.0.1"
        ],
        "deps": [
          {
            "dep_kinds": [
              {
                "kind": null,
                "target": null
              }
            ],
            "name": "bar",
            "pkg": "registry+https://github.com/rust-lang/crates.io-index#bar@0.0.1"
          }
        ],
        "features": [],
        "id": "registry+[ROOTURL]/alternative-registry#altdep@0.0.1"
      },
      {
        "dependencies": [],
        "deps": [],
        "features": [],
        "id": "registry+[ROOTURL]/alternative-registry#altdep2@0.0.1"
      },
      {
        "dependencies": [],
        "deps": [],
        "features": [],
        "id": "registry+https://github.com/rust-lang/crates.io-index#bar@0.0.1"
      },
      {
        "dependencies": [
          "registry+[ROOTURL]/alternative-registry#altdep@0.0.1",
          "registry+https://github.com/rust-lang/crates.io-index#iodep@0.0.1"
        ],
        "deps": [
          {
            "dep_kinds": [
              {
                "kind": null,
                "target": null
              }
            ],
            "name": "altdep",
            "pkg": "registry+[ROOTURL]/alternative-registry#altdep@0.0.1"
          },
          {
            "dep_kinds": [
              {
                "kind": null,
                "target": null
              }
            ],
            "name": "iodep",
            "pkg": "registry+https://github.com/rust-lang/crates.io-index#iodep@0.0.1"
          }
        ],
        "features": [],
        "id": "path+[ROOTURL]/foo#0.0.1"
      },
      {
        "dependencies": [
          "registry+[ROOTURL]/alternative-registry#altdep2@0.0.1"
        ],
        "deps": [
          {
            "dep_kinds": [
              {
                "kind": null,
                "target": null
              }
            ],
            "name": "altdep2",
            "pkg": "registry+[ROOTURL]/alternative-registry#altdep2@0.0.1"
          }
        ],
        "features": [],
        "id": "registry+https://github.com/rust-lang/crates.io-index#iodep@0.0.1"
      }
    ],
    "root": "path+[ROOTURL]/foo#0.0.1"
  },
  "target_directory": "[ROOT]/foo/target",
  "build_directory": "[ROOT]/foo/target",
  "version": 1,
  "workspace_default_members": [
    "path+[ROOTURL]/foo#0.0.1"
  ],
  "workspace_members": [
    "path+[ROOTURL]/foo#0.0.1"
  ],
  "workspace_root": "[ROOT]/foo"
}
"#]]
            .is_json(),
        )
        .run();
}

#[cargo_test]
fn unknown_registry() {
    // A known registry refers to an unknown registry.
    // foo -> bar(crates.io) -> baz(alt)
    registry::alt_init();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []
                edition = "2015"

                [dependencies.bar]
                version = "0.0.1"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("baz", "0.0.1").alternative(true).publish();
    Package::new("bar", "0.0.1")
        .registry_dep("baz", "0.0.1")
        .publish();

    // Remove "alternative" from config.
    let cfg_path = paths::home().join(".cargo/config.toml");
    let mut config = fs::read_to_string(&cfg_path).unwrap();
    let start = config.find("[registries.alternative]").unwrap();
    config.insert(start, '#');
    let start_index = &config[start..].find("index =").unwrap();
    config.insert(start + start_index, '#');
    fs::write(&cfg_path, config).unwrap();

    p.cargo("check").run();

    // Important parts:
    // foo -> bar registry = null
    // bar -> baz registry = alternate
    p.cargo("metadata --format-version=1")
        .with_stdout_data(
            str![[r#"
{
  "metadata": null,
  "packages": [
    {
      "authors": [],
      "categories": [],
      "default_run": null,
      "dependencies": [
        {
          "features": [],
          "kind": null,
          "name": "baz",
          "optional": false,
          "registry": "[ROOTURL]/alternative-registry",
          "rename": null,
          "req": "^0.0.1",
          "source": "registry+[ROOTURL]/alternative-registry",
          "target": null,
          "uses_default_features": true
        }
      ],
      "description": null,
      "documentation": null,
      "edition": "2015",
      "features": {},
      "homepage": null,
      "id": "registry+https://github.com/rust-lang/crates.io-index#bar@0.0.1",
      "keywords": [],
      "license": null,
      "license_file": null,
      "links": null,
      "manifest_path": "[ROOT]/home/.cargo/registry/src/-[HASH]/bar-0.0.1/Cargo.toml",
      "metadata": null,
      "name": "bar",
      "publish": null,
      "readme": null,
      "repository": null,
      "rust_version": null,
      "source": "registry+https://github.com/rust-lang/crates.io-index",
      "targets": [
        {
          "crate_types": [
            "lib"
          ],
          "doc": true,
          "doctest": true,
          "edition": "2015",
          "kind": [
            "lib"
          ],
          "name": "bar",
          "src_path": "[ROOT]/home/.cargo/registry/src/-[HASH]/bar-0.0.1/src/lib.rs",
          "test": true
        }
      ],
      "version": "0.0.1"
    },
    {
      "authors": [],
      "categories": [],
      "default_run": null,
      "dependencies": [],
      "description": null,
      "documentation": null,
      "edition": "2015",
      "features": {},
      "homepage": null,
      "id": "registry+[ROOTURL]/alternative-registry#baz@0.0.1",
      "keywords": [],
      "license": null,
      "license_file": null,
      "links": null,
      "manifest_path": "[ROOT]/home/.cargo/registry/src/-[HASH]/baz-0.0.1/Cargo.toml",
      "metadata": null,
      "name": "baz",
      "publish": null,
      "readme": null,
      "repository": null,
      "rust_version": null,
      "source": "registry+[ROOTURL]/alternative-registry",
      "targets": [
        {
          "crate_types": [
            "lib"
          ],
          "doc": true,
          "doctest": true,
          "edition": "2015",
          "kind": [
            "lib"
          ],
          "name": "baz",
          "src_path": "[ROOT]/home/.cargo/registry/src/-[HASH]/baz-0.0.1/src/lib.rs",
          "test": true
        }
      ],
      "version": "0.0.1"
    },
    {
      "authors": [],
      "categories": [],
      "default_run": null,
      "dependencies": [
        {
          "features": [],
          "kind": null,
          "name": "bar",
          "optional": false,
          "registry": null,
          "rename": null,
          "req": "^0.0.1",
          "source": "registry+https://github.com/rust-lang/crates.io-index",
          "target": null,
          "uses_default_features": true
        }
      ],
      "description": null,
      "documentation": null,
      "edition": "2015",
      "features": {},
      "homepage": null,
      "id": "path+[ROOTURL]/foo#0.0.1",
      "keywords": [],
      "license": null,
      "license_file": null,
      "links": null,
      "manifest_path": "[ROOT]/foo/Cargo.toml",
      "metadata": null,
      "name": "foo",
      "publish": null,
      "readme": null,
      "repository": null,
      "rust_version": null,
      "source": null,
      "targets": [
        {
          "crate_types": [
            "bin"
          ],
          "doc": true,
          "doctest": false,
          "edition": "2015",
          "kind": [
            "bin"
          ],
          "name": "foo",
          "src_path": "[ROOT]/foo/src/main.rs",
          "test": true
        }
      ],
      "version": "0.0.1"
    }
  ],
  "resolve": {
    "nodes": [
      {
        "dependencies": [
          "registry+[ROOTURL]/alternative-registry#baz@0.0.1"
        ],
        "deps": [
          {
            "dep_kinds": [
              {
                "kind": null,
                "target": null
              }
            ],
            "name": "baz",
            "pkg": "registry+[ROOTURL]/alternative-registry#baz@0.0.1"
          }
        ],
        "features": [],
        "id": "registry+https://github.com/rust-lang/crates.io-index#bar@0.0.1"
      },
      {
        "dependencies": [],
        "deps": [],
        "features": [],
        "id": "registry+[ROOTURL]/alternative-registry#baz@0.0.1"
      },
      {
        "dependencies": [
          "registry+https://github.com/rust-lang/crates.io-index#bar@0.0.1"
        ],
        "deps": [
          {
            "dep_kinds": [
              {
                "kind": null,
                "target": null
              }
            ],
            "name": "bar",
            "pkg": "registry+https://github.com/rust-lang/crates.io-index#bar@0.0.1"
          }
        ],
        "features": [],
        "id": "path+[ROOTURL]/foo#0.0.1"
      }
    ],
    "root": "path+[ROOTURL]/foo#0.0.1"
  },
  "target_directory": "[ROOT]/foo/target",
  "build_directory": "[ROOT]/foo/target",
  "version": 1,
  "workspace_default_members": [
    "path+[ROOTURL]/foo#0.0.1"
  ],
  "workspace_members": [
    "path+[ROOTURL]/foo#0.0.1"
  ],
  "workspace_root": "[ROOT]/foo"
}
"#]]
            .is_json(),
        )
        .run();
}

#[cargo_test]
fn registries_index_relative_url() {
    registry::alt_init();
    let config = paths::root().join(".cargo/config.toml");
    fs::create_dir_all(config.parent().unwrap()).unwrap();
    fs::write(
        &config,
        r#"
            [registries.relative]
            index = "file:alternative-registry"
        "#,
    )
    .unwrap();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []
                edition = "2015"

                [dependencies.bar]
                version = "0.0.1"
                registry = "relative"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("bar", "0.0.1").alternative(true).publish();

    p.cargo("check")
        .with_stderr_data(str![[r#"
[UPDATING] `relative` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] bar v0.0.1 (registry `relative`)
[CHECKING] bar v0.0.1 (registry `relative`)
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn registries_index_relative_path_not_allowed() {
    registry::alt_init();
    let config = paths::root().join(".cargo/config.toml");
    fs::create_dir_all(config.parent().unwrap()).unwrap();
    fs::write(
        &config,
        r#"
            [registries.relative]
            index = "alternative-registry"
        "#,
    )
    .unwrap();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []
                edition = "2015"

                [dependencies.bar]
                version = "0.0.1"
                registry = "relative"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("bar", "0.0.1").alternative(true).publish();

    p.cargo("check")
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  invalid index URL for registry `relative` defined in [ROOT]/.cargo/config.toml

Caused by:
  invalid url `alternative-registry`: relative URL without a base

"#]])
        .with_status(101)
        .run();
}

#[cargo_test]
fn both_index_and_registry() {
    let p = project().file("src/lib.rs", "").build();
    for cmd in &["publish", "owner", "search", "yank --version 1.0.0"] {
        p.cargo(cmd)
            .arg("--registry=foo")
            .arg("--index=foo")
            .with_status(1)
            .with_stderr_data(str![[r#"
[ERROR] the argument '--registry <REGISTRY>' cannot be used with '--index <INDEX>'

Usage: [..]

For more information, try '--help'.

"#]])
            .run();
    }
}

#[cargo_test]
fn both_index_and_default() {
    let p = project().file("src/lib.rs", "").build();
    for cmd in &[
        "publish",
        "owner",
        "search",
        "yank --version 1.0.0",
        "install foo",
    ] {
        p.cargo(cmd)
            .env("CARGO_REGISTRY_DEFAULT", "undefined")
            .arg(format!("--index=index_url"))
            .with_status(101)
            .with_stderr_data(str![[r#"
[ERROR] invalid url `index_url`: relative URL without a base

"#]])
            .run();
    }
}

#[cargo_test]
fn sparse_lockfile() {
    let _registry = registry::RegistryBuilder::new()
        .http_index()
        .alternative()
        .build();
    Package::new("foo", "0.1.0").alternative(true).publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "a"
                version = "0.5.0"
                authors = []
                edition = "2015"

                [dependencies]
                foo = { registry = 'alternative', version = '0.1.0'}
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("generate-lockfile").run();
    assert_e2e().eq(
        &p.read_lockfile(),
        str![[r##"
# This file is automatically @generated by Cargo.
# It is not intended for manual editing.
version = 4

[[package]]
name = "a"
version = "0.5.0"
dependencies = [
 "foo",
]

[[package]]
name = "foo"
version = "0.1.0"
source = "sparse+http://127.0.0.1:[..]/index/"
checksum = "458c1addb23fde7dfbca0410afdbcc0086f96197281ec304d9e0e10def3cb899"

"##]],
    );
}

#[cargo_test]
fn publish_with_transitive_dep() {
    let _alt1 = RegistryBuilder::new()
        .http_api()
        .http_index()
        .alternative_named("Alt-1")
        .build();
    let _alt2 = RegistryBuilder::new()
        .http_api()
        .http_index()
        .alternative_named("Alt-2")
        .build();

    let p1 = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "a"
                version = "0.5.0"
                edition = "2015"
            "#,
        )
        .file("src/lib.rs", "")
        .build();
    p1.cargo("publish --registry Alt-1").run();

    let p2 = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "b"
                version = "0.6.0"
                publish = ["Alt-2"]
                edition = "2015"

                [dependencies]
                a = { version = "0.5.0", registry = "Alt-1" }
            "#,
        )
        .file("src/lib.rs", "")
        .build();
    p2.cargo("publish").run();
}

#[cargo_test]
fn warn_for_unused_fields() {
    let _ = RegistryBuilder::new()
        .no_configure_token()
        .alternative()
        .build();
    let p = project()
        .file("src/lib.rs", "")
        .file(
            ".cargo/config.toml",
            "[registry]
            unexpected-field = 'foo'
            [registries.alternative]
            unexpected-field = 'foo'
            ",
        )
        .build();

    p.cargo("publish --registry alternative")
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] `alternative` index
[WARNING] unused config key `registries.alternative.unexpected-field` in `[ROOT]/foo/.cargo/config.toml`
[ERROR] no token found for `alternative`, please run `cargo login --registry alternative`
or use environment variable CARGO_REGISTRIES_ALTERNATIVE_TOKEN

"#]])
        .run();

    let crates_io = registry::RegistryBuilder::new()
        .no_configure_token()
        .build();
    p.cargo("publish --registry crates-io")
        .replace_crates_io(crates_io.index_url())
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] crates.io index
[WARNING] unused config key `registry.unexpected-field` in `[ROOT]/foo/.cargo/config.toml`
[ERROR] no token found, please run `cargo login`
or use environment variable CARGO_REGISTRY_TOKEN

"#]])
        .run();
}

#[cargo_test]
fn config_empty_registry_name() {
    let _ = RegistryBuilder::new()
        .no_configure_token()
        .alternative()
        .build();
    let p = project()
        .file("src/lib.rs", "")
        .file(
            ".cargo/config.toml",
            "[registry.'']
            ",
        )
        .build();

    p.cargo("publish")
        .arg("--registry")
        .arg("")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] registry name cannot be empty

"#]])
        .run();
}

#[cargo_test]
fn empty_registry_flag() {
    let p = project().file("src/lib.rs", "").build();

    p.cargo("publish")
        .arg("--registry")
        .arg("")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] registry name cannot be empty

"#]])
        .run();
}

#[cargo_test]
fn empty_dependency_registry() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"

                [dependencies]
                bar = { version = "0.1.0", registry = "" }
            "#,
        )
        .file(
            "src/lib.rs",
            "
            extern crate bar;
            pub fn f() { bar::bar(); }
            ",
        )
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] registry name cannot be empty
       
       
 --> Cargo.toml:8:23
  |
8 |                 bar = { version = "0.1.0", registry = "" }
  |                       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

"#]])
        .run();
}
