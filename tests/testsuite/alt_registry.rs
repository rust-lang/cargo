//! Tests for alternative registries.

use cargo_test_support::compare::assert_match_exact;
use cargo_test_support::publish::validate_alt_upload;
use cargo_test_support::registry::{self, Package, RegistryBuilder};
use cargo_test_support::{basic_manifest, paths, project};
use std::fs;

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

                [dependencies.bar]
                version = "0.0.1"
                registry = "alternative"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("bar", "0.0.1").alternative(true).publish();

    p.cargo("check")
        .with_stderr(
            "\
[UPDATING] `alternative` index
[DOWNLOADING] crates ...
[DOWNLOADED] bar v0.0.1 (registry `alternative`)
[CHECKING] bar v0.0.1 (registry `alternative`)
[CHECKING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
",
        )
        .run();

    p.cargo("clean").run();

    // Don't download a second time
    p.cargo("check")
        .with_stderr(
            "\
[CHECKING] bar v0.0.1 (registry `alternative`)
[CHECKING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
",
        )
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
        .with_stderr(
            "\
[UPDATING] `alternative` index
[DOWNLOADING] crates ...
[DOWNLOADED] [..] v0.0.1 (registry `alternative`)
[DOWNLOADED] [..] v0.0.1 (registry `alternative`)
[CHECKING] baz v0.0.1 (registry `alternative`)
[CHECKING] bar v0.0.1 (registry `alternative`)
[CHECKING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
",
        )
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
        .with_stderr(
            "\
[UPDATING] `alternative` index
[DOWNLOADING] crates ...
[DOWNLOADED] [..] v0.0.1 (registry `alternative`)
[DOWNLOADED] [..] v0.0.1 (registry `alternative`)
[CHECKING] baz v0.0.1 (registry `alternative`)
[CHECKING] bar v0.0.1 (registry `alternative`)
[CHECKING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
",
        )
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
        .with_stderr_unordered(
            "\
[UPDATING] `alternative` index
[UPDATING] `dummy-registry` index
[DOWNLOADING] crates ...
[DOWNLOADED] baz v0.0.1 (registry `dummy-registry`)
[DOWNLOADED] bar v0.0.1 (registry `alternative`)
[CHECKING] baz v0.0.1
[CHECKING] bar v0.0.1 (registry `alternative`)
[CHECKING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
",
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
        .with_stderr(
            "\
[CHECKING] bar v0.0.1 ([CWD]/bar)
[CHECKING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
",
        )
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

                [dependencies.bar]
                git = ""
                registry = "alternative"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_contains(
            "  dependency (bar) specification is ambiguous. \
             Only one of `git` or `registry` is allowed.",
        )
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
        .with_stderr_contains("[ERROR] crates cannot be published to crates.io[..]")
        .run();

    p.cargo("publish")
        .replace_crates_io(crates_io.index_url())
        .arg("--token")
        .arg(crates_io.token())
        .arg("--index")
        .arg(crates_io.index_url().as_str())
        .with_status(101)
        .with_stderr_contains("[ERROR] crates cannot be published to crates.io[..]")
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

                [dependencies.bar]
                version = "0.0.1"
                registry = "alternative"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("bar", "0.0.1").alternative(true).publish();

    p.cargo("publish --registry alternative")
        .with_stderr(
            "\
[UPDATING] `alternative` index
[WARNING] [..]
[..]
[PACKAGING] foo v0.0.1 [..]
[UPDATING] `alternative` index
[VERIFYING] foo v0.0.1 [..]
[DOWNLOADING] [..]
[DOWNLOADED] bar v0.0.1 (registry `alternative`)
[COMPILING] bar v0.0.1 (registry `alternative`)
[COMPILING] foo v0.0.1 [..]
[FINISHED] [..]
[PACKAGED] [..]
[UPLOADING] foo v0.0.1 [..]
[UPLOADED] foo v0.0.1 to registry `alternative`
note: Waiting for `foo v0.0.1` to be available at registry `alternative`.
You may press ctrl-c to skip waiting; the crate should be available shortly.
[PUBLISHED] foo v0.0.1 at registry `alternative`
",
        )
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
        .with_stderr_unordered(
            "\
[UPDATING] `alternative` index
[UPDATING] `dummy-registry` index
[DOWNLOADING] crates ...
[DOWNLOADED] crates_io_dep v0.0.1 (registry `dummy-registry`)
[DOWNLOADED] alt_reg_dep v0.1.0 (registry `alternative`)
[CHECKING] alt_reg_dep v0.1.0 (registry `alternative`)
[CHECKING] crates_io_dep v0.0.1
[CHECKING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
",
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
        .with_stderr(
            "\
[UPDATING] `alternative` index
error: no token found for `alternative`, please run `cargo login --registry alternative`
or use environment variable CARGO_REGISTRIES_ALTERNATIVE_TOKEN",
        )
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
        .with_stderr(
            "\
[UPDATING] `alternative` index
error: no token found for `alternative`, please run `cargo login --registry alternative`
or use environment variable CARGO_REGISTRIES_ALTERNATIVE_TOKEN",
        )
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
        .with_stderr(
            "\
[UPDATING] `alternative` index
[WARNING] [..]
[..]
[PACKAGING] foo v0.0.1 [..]
[VERIFYING] foo v0.0.1 [..]
[COMPILING] foo v0.0.1 [..]
[FINISHED] [..]
[PACKAGED] [..]
[UPLOADING] foo v0.0.1 [..]
[UPLOADED] foo v0.0.1 to registry `alternative`
note: Waiting for `foo v0.0.1` to be available at registry `alternative`.
You may press ctrl-c to skip waiting; the crate should be available shortly.
[PUBLISHED] foo v0.0.1 at registry `alternative`
",
        )
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
        .with_stderr(
            "\
[UPDATING] `alternative` index
[WARNING] [..]
[..]
[PACKAGING] foo v0.0.1 [..]
[UPDATING] `dummy-registry` index
[VERIFYING] foo v0.0.1 [..]
[DOWNLOADING] [..]
[DOWNLOADED] bar v0.0.1 (registry `dummy-registry`)
[COMPILING] bar v0.0.1
[COMPILING] foo v0.0.1 [..]
[FINISHED] [..]
[PACKAGED] [..]
[UPLOADING] foo v0.0.1 [..]
[UPLOADED] foo v0.0.1 to registry `alternative`
note: Waiting for `foo v0.0.1` to be available at registry `alternative`.
You may press ctrl-c to skip waiting; the crate should be available shortly.
[PUBLISHED] foo v0.0.1 at registry `alternative`
",
        )
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

    let config = paths::home().join(".cargo/config");

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
        .with_stderr(
            "\
error: invalid index URL for registry `alternative` defined in [..]/home/.cargo/config

Caused by:
  registry URLs may not contain passwords
",
        )
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
        .with_stderr(
            "\
[UPDATING] `alternative` index
[CHECKING] bar v0.1.0 ([CWD]/bar)
[CHECKING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
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

                [dependencies.bar]
                version = "0.0.1"
                registry = "bad name"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("build")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] failed to parse manifest at `[CWD]/Cargo.toml`

Caused by:
  invalid character ` ` in registry name: `bad name`, [..]",
        )
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
            .with_stderr("[ERROR] invalid character ` ` in registry name: `bad name`, [..]")
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

                [dependencies.bar]
                version = "0.0.1"
                registry = "alternative"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check")
        .with_stderr(
            "\
[UPDATING] `alternative` index
[DOWNLOADING] crates ...
[DOWNLOADED] bar v0.0.1 (registry `alternative`)
[CHECKING] bar v0.0.1 (registry `alternative`)
[CHECKING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
",
        )
        .run();

    // Check all of the API commands.
    let err = "[ERROR] registry `alternative` does not support API commands";

    p.cargo("login --registry alternative TOKEN")
        .with_status(101)
        .with_stderr_contains(&err)
        .run();

    p.cargo("publish --registry alternative")
        .with_status(101)
        .with_stderr_contains(&err)
        .run();

    p.cargo("search --registry alternative")
        .with_status(101)
        .with_stderr_contains(&err)
        .run();

    p.cargo("owner --registry alternative --list")
        .with_status(101)
        .with_stderr_contains(&err)
        .run();

    p.cargo("yank --registry alternative --version=0.0.1 bar")
        .with_status(101)
        .with_stderr_contains(&err)
        .run();

    p.cargo("yank --registry alternative --version=0.0.1 bar")
        .with_stderr_contains(&err)
        .with_status(101)
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
        .with_json(
            r#"
            {
                "packages": [
                    {
                        "name": "foo",
                        "version": "0.0.1",
                        "id": "foo 0.0.1 (path+file:[..]/foo)",
                        "license": null,
                        "license_file": null,
                        "description": null,
                        "source": null,
                        "dependencies": [
                            {
                                "name": "altdep",
                                "source": "registry+file:[..]/alternative-registry",
                                "req": "^0.0.1",
                                "kind": null,
                                "rename": null,
                                "optional": false,
                                "uses_default_features": true,
                                "features": [],
                                "target": null,
                                "registry": "file:[..]/alternative-registry"
                            },
                            {
                                "name": "iodep",
                                "source": "registry+https://github.com/rust-lang/crates.io-index",
                                "req": "^0.0.1",
                                "kind": null,
                                "rename": null,
                                "optional": false,
                                "uses_default_features": true,
                                "features": [],
                                "target": null,
                                "registry": null
                            }
                        ],
                        "targets": "{...}",
                        "features": {},
                        "manifest_path": "[..]/foo/Cargo.toml",
                        "metadata": null,
                        "publish": null,
                        "authors": [],
                        "categories": [],
                        "default_run": null,
                        "keywords": [],
                        "readme": null,
                        "repository": null,
                        "rust_version": null,
                        "homepage": null,
                        "documentation": null,
                        "edition": "2015",
                        "links": null
                    }
                ],
                "workspace_members": [
                    "foo 0.0.1 (path+file:[..]/foo)"
                ],
                "workspace_default_members": [
                    "foo 0.0.1 (path+file:[..]/foo)"
                ],
                "resolve": null,
                "target_directory": "[..]/foo/target",
                "version": 1,
                "workspace_root": "[..]/foo",
                "metadata": null
            }"#,
        )
        .run();

    // --no-deps uses a different code path, make sure both work.
    p.cargo("metadata --format-version=1")
        .with_json(
            r#"
             {
                "packages": [
                    {
                        "name": "altdep",
                        "version": "0.0.1",
                        "id": "altdep 0.0.1 (registry+file:[..]/alternative-registry)",
                        "license": null,
                        "license_file": null,
                        "description": null,
                        "source": "registry+file:[..]/alternative-registry",
                        "dependencies": [
                            {
                                "name": "bar",
                                "source": "registry+https://github.com/rust-lang/crates.io-index",
                                "req": "^0.0.1",
                                "kind": null,
                                "rename": null,
                                "optional": false,
                                "uses_default_features": true,
                                "features": [],
                                "target": null,
                                "registry": null
                            }
                        ],
                        "targets": "{...}",
                        "features": {},
                        "manifest_path": "[..]/altdep-0.0.1/Cargo.toml",
                        "metadata": null,
                        "publish": null,
                        "authors": [],
                        "categories": [],
                        "default_run": null,
                        "keywords": [],
                        "readme": null,
                        "repository": null,
                        "rust_version": null,
                        "homepage": null,
                        "documentation": null,
                        "edition": "2015",
                        "links": null
                    },
                    {
                        "name": "altdep2",
                        "version": "0.0.1",
                        "id": "altdep2 0.0.1 (registry+file:[..]/alternative-registry)",
                        "license": null,
                        "license_file": null,
                        "description": null,
                        "source": "registry+file:[..]/alternative-registry",
                        "dependencies": [],
                        "targets": "{...}",
                        "features": {},
                        "manifest_path": "[..]/altdep2-0.0.1/Cargo.toml",
                        "metadata": null,
                        "publish": null,
                        "authors": [],
                        "categories": [],
                        "default_run": null,
                        "keywords": [],
                        "readme": null,
                        "repository": null,
                        "rust_version": null,
                        "homepage": null,
                        "documentation": null,
                        "edition": "2015",
                        "links": null
                    },
                    {
                        "name": "bar",
                        "version": "0.0.1",
                        "id": "bar 0.0.1 (registry+https://github.com/rust-lang/crates.io-index)",
                        "license": null,
                        "license_file": null,
                        "description": null,
                        "source": "registry+https://github.com/rust-lang/crates.io-index",
                        "dependencies": [],
                        "targets": "{...}",
                        "features": {},
                        "manifest_path": "[..]/bar-0.0.1/Cargo.toml",
                        "metadata": null,
                        "publish": null,
                        "authors": [],
                        "categories": [],
                        "default_run": null,
                        "keywords": [],
                        "readme": null,
                        "repository": null,
                        "rust_version": null,
                        "homepage": null,
                        "documentation": null,
                        "edition": "2015",
                        "links": null
                    },
                    {
                        "name": "foo",
                        "version": "0.0.1",
                        "id": "foo 0.0.1 (path+file:[..]/foo)",
                        "license": null,
                        "license_file": null,
                        "description": null,
                        "source": null,
                        "dependencies": [
                            {
                                "name": "altdep",
                                "source": "registry+file:[..]/alternative-registry",
                                "req": "^0.0.1",
                                "kind": null,
                                "rename": null,
                                "optional": false,
                                "uses_default_features": true,
                                "features": [],
                                "target": null,
                                "registry": "file:[..]/alternative-registry"
                            },
                            {
                                "name": "iodep",
                                "source": "registry+https://github.com/rust-lang/crates.io-index",
                                "req": "^0.0.1",
                                "kind": null,
                                "rename": null,
                                "optional": false,
                                "uses_default_features": true,
                                "features": [],
                                "target": null,
                                "registry": null
                            }
                        ],
                        "targets": "{...}",
                        "features": {},
                        "manifest_path": "[..]/foo/Cargo.toml",
                        "metadata": null,
                        "publish": null,
                        "authors": [],
                        "categories": [],
                        "default_run": null,
                        "keywords": [],
                        "readme": null,
                        "repository": null,
                        "rust_version": null,
                        "homepage": null,
                        "documentation": null,
                        "edition": "2015",
                        "links": null
                    },
                    {
                        "name": "iodep",
                        "version": "0.0.1",
                        "id": "iodep 0.0.1 (registry+https://github.com/rust-lang/crates.io-index)",
                        "license": null,
                        "license_file": null,
                        "description": null,
                        "source": "registry+https://github.com/rust-lang/crates.io-index",
                        "dependencies": [
                            {
                                "name": "altdep2",
                                "source": "registry+file:[..]/alternative-registry",
                                "req": "^0.0.1",
                                "kind": null,
                                "rename": null,
                                "optional": false,
                                "uses_default_features": true,
                                "features": [],
                                "target": null,
                                "registry": "file:[..]/alternative-registry"
                            }
                        ],
                        "targets": "{...}",
                        "features": {},
                        "manifest_path": "[..]/iodep-0.0.1/Cargo.toml",
                        "metadata": null,
                        "publish": null,
                        "authors": [],
                        "categories": [],
                        "default_run": null,
                        "keywords": [],
                        "readme": null,
                        "repository": null,
                        "rust_version": null,
                        "homepage": null,
                        "documentation": null,
                        "edition": "2015",
                        "links": null
                    }
                ],
                "workspace_members": [
                    "foo 0.0.1 (path+file:[..]/foo)"
                ],
                "workspace_default_members": [
                    "foo 0.0.1 (path+file:[..]/foo)"
                ],
                "resolve": "{...}",
                "target_directory": "[..]/foo/target",
                "version": 1,
                "workspace_root": "[..]/foo",
                "metadata": null
            }"#,
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
    let cfg_path = paths::home().join(".cargo/config");
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
        .with_json(
            r#"
            {
              "packages": [
                {
                  "name": "bar",
                  "version": "0.0.1",
                  "id": "bar 0.0.1 (registry+https://github.com/rust-lang/crates.io-index)",
                  "license": null,
                  "license_file": null,
                  "description": null,
                  "source": "registry+https://github.com/rust-lang/crates.io-index",
                  "dependencies": [
                    {
                      "name": "baz",
                      "source": "registry+file://[..]/alternative-registry",
                      "req": "^0.0.1",
                      "kind": null,
                      "rename": null,
                      "optional": false,
                      "uses_default_features": true,
                      "features": [],
                      "target": null,
                      "registry": "file:[..]/alternative-registry"
                    }
                  ],
                  "targets": "{...}",
                  "features": {},
                  "manifest_path": "[..]",
                  "metadata": null,
                  "publish": null,
                  "authors": [],
                  "categories": [],
                  "default_run": null,
                  "keywords": [],
                  "readme": null,
                  "repository": null,
                  "rust_version": null,
                  "homepage": null,
                  "documentation": null,
                  "edition": "2015",
                  "links": null
                },
                {
                  "name": "baz",
                  "version": "0.0.1",
                  "id": "baz 0.0.1 (registry+file://[..]/alternative-registry)",
                  "license": null,
                  "license_file": null,
                  "description": null,
                  "source": "registry+file://[..]/alternative-registry",
                  "dependencies": [],
                  "targets": "{...}",
                  "features": {},
                  "manifest_path": "[..]",
                  "metadata": null,
                  "publish": null,
                  "authors": [],
                  "categories": [],
                  "default_run": null,
                  "keywords": [],
                  "readme": null,
                  "repository": null,
                  "rust_version": null,
                  "homepage": null,
                  "documentation": null,
                  "edition": "2015",
                  "links": null
                },
                {
                  "name": "foo",
                  "version": "0.0.1",
                  "id": "foo 0.0.1 (path+file://[..]/foo)",
                  "license": null,
                  "license_file": null,
                  "description": null,
                  "source": null,
                  "dependencies": [
                    {
                      "name": "bar",
                      "source": "registry+https://github.com/rust-lang/crates.io-index",
                      "req": "^0.0.1",
                      "kind": null,
                      "rename": null,
                      "optional": false,
                      "uses_default_features": true,
                      "features": [],
                      "target": null,
                      "registry": null
                    }
                  ],
                  "targets": "{...}",
                  "features": {},
                  "manifest_path": "[..]/foo/Cargo.toml",
                  "metadata": null,
                  "publish": null,
                  "authors": [],
                  "categories": [],
                  "default_run": null,
                  "keywords": [],
                  "readme": null,
                  "repository": null,
                  "rust_version": null,
                  "homepage": null,
                  "documentation": null,
                  "edition": "2015",
                  "links": null
                }
              ],
              "workspace_members": [
                "foo 0.0.1 (path+file://[..]/foo)"
              ],
              "workspace_default_members": [
                "foo 0.0.1 (path+file://[..]/foo)"
              ],
              "resolve": "{...}",
              "target_directory": "[..]/foo/target",
              "version": 1,
              "workspace_root": "[..]/foo",
              "metadata": null
            }
            "#,
        )
        .run();
}

#[cargo_test]
fn registries_index_relative_url() {
    registry::alt_init();
    let config = paths::root().join(".cargo/config");
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

                [dependencies.bar]
                version = "0.0.1"
                registry = "relative"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("bar", "0.0.1").alternative(true).publish();

    p.cargo("check")
        .with_stderr(
            "\
[UPDATING] `relative` index
[DOWNLOADING] crates ...
[DOWNLOADED] bar v0.0.1 (registry `relative`)
[CHECKING] bar v0.0.1 (registry `relative`)
[CHECKING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
",
        )
        .run();
}

#[cargo_test]
fn registries_index_relative_path_not_allowed() {
    registry::alt_init();
    let config = paths::root().join(".cargo/config");
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

                [dependencies.bar]
                version = "0.0.1"
                registry = "relative"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("bar", "0.0.1").alternative(true).publish();

    p.cargo("check")
        .with_stderr(&format!(
            "\
error: failed to parse manifest at `{root}/foo/Cargo.toml`

Caused by:
  invalid index URL for registry `relative` defined in [..]/.cargo/config

Caused by:
  invalid url `alternative-registry`: relative URL without a base
",
            root = paths::root().to_str().unwrap()
        ))
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
            .with_stderr_contains(
                "error: the argument '--registry <REGISTRY>' cannot be used with '--index <INDEX>'",
            )
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
            .with_stderr("[ERROR] invalid url `index_url`: relative URL without a base")
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

                [dependencies]
                foo = { registry = 'alternative', version = '0.1.0'}
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("generate-lockfile").run();
    assert_match_exact(
        &p.read_lockfile(),
        r#"# This file is automatically @generated by Cargo.
# It is not intended for manual editing.
version = 3

[[package]]
name = "a"
version = "0.5.0"
dependencies = [
 "foo",
]

[[package]]
name = "foo"
version = "0.1.0"
source = "sparse+http://[..]/"
checksum = "f6a200a9339fef960979d94d5c99cbbfd899b6f5a396a55d9775089119050203""#,
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
        .with_stderr(
            "\
[UPDATING] `alternative` index
[WARNING] unused config key `registries.alternative.unexpected-field` in `[..]config.toml`
[ERROR] no token found for `alternative`, please run `cargo login --registry alternative`
or use environment variable CARGO_REGISTRIES_ALTERNATIVE_TOKEN",
        )
        .run();

    p.cargo("publish --registry crates-io")
        .with_status(101)
        .with_stderr(
            "\
[UPDATING] crates.io index
[WARNING] unused config key `registry.unexpected-field` in `[..]config.toml`
[ERROR] no token found, please run `cargo login`
or use environment variable CARGO_REGISTRY_TOKEN",
        )
        .run();
}
