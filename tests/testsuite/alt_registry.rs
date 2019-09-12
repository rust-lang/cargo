use cargo::util::IntoUrl;
use cargo_test_support::publish::validate_alt_upload;
use cargo_test_support::registry::{self, Package};
use cargo_test_support::{basic_manifest, git, paths, project};
use std::fs::{self, File};
use std::io::Write;

#[cargo_test]
fn depend_on_alt_registry() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
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

    p.cargo("build")
        .with_stderr(&format!(
            "\
[UPDATING] `{reg}` index
[DOWNLOADING] crates ...
[DOWNLOADED] bar v0.0.1 (registry `[ROOT][..]`)
[COMPILING] bar v0.0.1 (registry `[ROOT][..]`)
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
",
            reg = registry::alt_registry_path().to_str().unwrap()
        ))
        .run();

    p.cargo("clean").run();

    // Don't download a second time
    p.cargo("build")
        .with_stderr(
            "\
[COMPILING] bar v0.0.1 (registry `[ROOT][..]`)
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
",
        )
        .run();
}

#[cargo_test]
fn depend_on_alt_registry_depends_on_same_registry_no_index() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
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

    p.cargo("build")
        .with_stderr(&format!(
            "\
[UPDATING] `{reg}` index
[DOWNLOADING] crates ...
[DOWNLOADED] [..] v0.0.1 (registry `[ROOT][..]`)
[DOWNLOADED] [..] v0.0.1 (registry `[ROOT][..]`)
[COMPILING] baz v0.0.1 (registry `[ROOT][..]`)
[COMPILING] bar v0.0.1 (registry `[ROOT][..]`)
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
",
            reg = registry::alt_registry_path().to_str().unwrap()
        ))
        .run();
}

#[cargo_test]
fn depend_on_alt_registry_depends_on_same_registry() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
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

    p.cargo("build")
        .with_stderr(&format!(
            "\
[UPDATING] `{reg}` index
[DOWNLOADING] crates ...
[DOWNLOADED] [..] v0.0.1 (registry `[ROOT][..]`)
[DOWNLOADED] [..] v0.0.1 (registry `[ROOT][..]`)
[COMPILING] baz v0.0.1 (registry `[ROOT][..]`)
[COMPILING] bar v0.0.1 (registry `[ROOT][..]`)
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
",
            reg = registry::alt_registry_path().to_str().unwrap()
        ))
        .run();
}

#[cargo_test]
fn depend_on_alt_registry_depends_on_crates_io() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
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

    p.cargo("build")
        .with_stderr_unordered(&format!(
            "\
[UPDATING] `{alt_reg}` index
[UPDATING] `{reg}` index
[DOWNLOADING] crates ...
[DOWNLOADED] baz v0.0.1 (registry `[ROOT][..]`)
[DOWNLOADED] bar v0.0.1 (registry `[ROOT][..]`)
[COMPILING] baz v0.0.1
[COMPILING] bar v0.0.1 (registry `[ROOT][..]`)
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
",
            alt_reg = registry::alt_registry_path().to_str().unwrap(),
            reg = registry::registry_path().to_str().unwrap()
        ))
        .run();
}

#[cargo_test]
fn registry_and_path_dep_works() {
    registry::init();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
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

    p.cargo("build")
        .with_stderr(
            "\
[COMPILING] bar v0.0.1 ([CWD]/bar)
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
",
        )
        .run();
}

#[cargo_test]
fn registry_incompatible_with_git() {
    registry::init();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
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

    p.cargo("build")
        .with_status(101)
        .with_stderr_contains(
            "  dependency (bar) specification is ambiguous. \
             Only one of `git` or `registry` is allowed.",
        )
        .run();
}

#[cargo_test]
fn cannot_publish_to_crates_io_with_registry_dependency() {
    let fakeio_path = paths::root().join("fake.io");
    let fakeio_url = fakeio_path.into_url().unwrap();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
            [dependencies.bar]
            version = "0.0.1"
            registry = "alternative"
        "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            ".cargo/config",
            &format!(
                r#"
            [registries.fakeio]
            index = "{}"
        "#,
                fakeio_url
            ),
        )
        .build();

    Package::new("bar", "0.0.1").alternative(true).publish();

    // Since this can't really call plain `publish` without fetching the real
    // crates.io index, create a fake one that points to the real crates.io.
    git::repo(&fakeio_path)
        .file(
            "config.json",
            r#"
            {"dl": "https://crates.io/api/v1/crates", "api": "https://crates.io"}
        "#,
        )
        .build();

    // Login so that we have the token available
    p.cargo("login --registry fakeio TOKEN").run();

    p.cargo("publish --registry fakeio")
        .with_status(101)
        .with_stderr_contains("[ERROR] crates cannot be published to crates.io[..]")
        .run();

    p.cargo("publish --index")
        .arg(fakeio_url.to_string())
        .with_status(101)
        .with_stderr_contains("[ERROR] crates cannot be published to crates.io[..]")
        .run();
}

#[cargo_test]
fn publish_with_registry_dependency() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
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

    // Login so that we have the token available
    p.cargo("login --registry alternative TOKEN").run();

    p.cargo("publish --registry alternative").run();

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
            "vers": "0.0.1"
        }"#,
        "foo-0.0.1.crate",
        &["Cargo.lock", "Cargo.toml", "Cargo.toml.orig", "src/main.rs"],
    );
}

#[cargo_test]
fn alt_registry_and_crates_io_deps() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
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

    p.cargo("build")
        .with_stderr_contains(format!(
            "[UPDATING] `{}` index",
            registry::alt_registry_path().to_str().unwrap()
        ))
        .with_stderr_contains(&format!(
            "[UPDATING] `{}` index",
            registry::registry_path().to_str().unwrap()
        ))
        .with_stderr_contains("[DOWNLOADED] crates_io_dep v0.0.1 (registry `[ROOT][..]`)")
        .with_stderr_contains("[DOWNLOADED] alt_reg_dep v0.1.0 (registry `[ROOT][..]`)")
        .with_stderr_contains("[COMPILING] alt_reg_dep v0.1.0 (registry `[ROOT][..]`)")
        .with_stderr_contains("[COMPILING] crates_io_dep v0.0.1")
        .with_stderr_contains("[COMPILING] foo v0.0.1 ([CWD])")
        .with_stderr_contains("[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s")
        .run();
}

#[cargo_test]
fn block_publish_due_to_no_token() {
    let p = project().file("src/main.rs", "fn main() {}").build();

    // Setup the registry by publishing a package
    Package::new("bar", "0.0.1").alternative(true).publish();

    fs::remove_file(paths::home().join(".cargo/credentials")).unwrap();

    // Now perform the actual publish
    p.cargo("publish --registry alternative")
        .with_status(101)
        .with_stderr_contains("error: no upload token found, please run `cargo login`")
        .run();
}

#[cargo_test]
fn publish_to_alt_registry() {
    let p = project().file("src/main.rs", "fn main() {}").build();

    // Setup the registry by publishing a package
    Package::new("bar", "0.0.1").alternative(true).publish();

    // Login so that we have the token available
    p.cargo("login --registry alternative TOKEN").run();

    // Now perform the actual publish
    p.cargo("publish --registry alternative").run();

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
            "vers": "0.0.1"
        }"#,
        "foo-0.0.1.crate",
        &["Cargo.lock", "Cargo.toml", "Cargo.toml.orig", "src/main.rs"],
    );
}

#[cargo_test]
fn publish_with_crates_io_dep() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
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

    // Login so that we have the token available
    p.cargo("login --registry alternative TOKEN").run();

    p.cargo("publish --registry alternative").run();

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
            "vers": "0.0.1"
        }"#,
        "foo-0.0.1.crate",
        &["Cargo.lock", "Cargo.toml", "Cargo.toml.orig", "src/main.rs"],
    );
}

#[cargo_test]
fn passwords_in_registry_index_url_forbidden() {
    registry::init();

    let config = paths::home().join(".cargo/config");

    File::create(config)
        .unwrap()
        .write_all(
            br#"
        [registry]
        index = "ssh://git:secret@foobar.com"
        "#,
        )
        .unwrap();

    let p = project().file("src/main.rs", "fn main() {}").build();

    p.cargo("publish")
        .with_status(101)
        .with_stderr_contains("error: Registry URLs may not contain passwords")
        .run();
}

#[cargo_test]
fn passwords_in_registries_index_url_forbidden() {
    registry::init();

    let config = paths::home().join(".cargo/config");

    File::create(config)
        .unwrap()
        .write_all(
            br#"
        [registries.alternative]
        index = "ssh://git:secret@foobar.com"
        "#,
        )
        .unwrap();

    let p = project().file("src/main.rs", "fn main() {}").build();

    p.cargo("publish --registry alternative")
        .with_status(101)
        .with_stderr_contains("error: Registry URLs may not contain passwords")
        .run();
}

#[cargo_test]
fn patch_alt_reg() {
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

    p.cargo("build")
        .with_stderr(
            "\
[UPDATING] `[ROOT][..]` index
[COMPILING] bar v0.1.0 ([CWD]/bar)
[COMPILING] foo v0.0.1 ([CWD])
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
            [project]
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
  Invalid character ` ` in registry name: `bad name`",
        )
        .run();

    for cmd in &[
        "init",
        "install foo",
        "login",
        "owner",
        "publish",
        "search",
        "yank",
    ] {
        p.cargo(cmd)
            .arg("--registry")
            .arg("bad name")
            .with_status(101)
            .with_stderr("[ERROR] Invalid character ` ` in registry name: `bad name`")
            .run();
    }
}

#[cargo_test]
fn no_api() {
    Package::new("bar", "0.0.1").alternative(true).publish();
    // Configure without `api`.
    let repo = git2::Repository::open(registry::alt_registry_path()).unwrap();
    let cfg_path = registry::alt_registry_path().join("config.json");
    fs::write(
        cfg_path,
        format!(r#"{{"dl": "{}"}}"#, registry::alt_dl_url()),
    )
    .unwrap();
    git::add(&repo);
    git::commit(&repo);

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

    p.cargo("build")
        .with_stderr(&format!(
            "\
[UPDATING] `{reg}` index
[DOWNLOADING] crates ...
[DOWNLOADED] bar v0.0.1 (registry `[ROOT][..]`)
[COMPILING] bar v0.0.1 (registry `[ROOT][..]`)
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
",
            reg = registry::alt_registry_path().to_str().unwrap()
        ))
        .run();

    // Check all of the API commands.
    let err = format!(
        "[ERROR] registry `{}` does not support API commands",
        registry::alt_registry_path().display()
    );

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

    p.cargo("yank --registry alternative --vers=0.0.1 bar")
        .with_status(101)
        .with_stderr_contains(&err)
        .run();

    p.cargo("yank --registry alternative --vers=0.0.1 bar")
        .with_stderr_contains(&err)
        .with_status(101)
        .run();
}

#[cargo_test]
fn alt_reg_metadata() {
    // Check for "registry" entries in `cargo metadata` with alternative registries.
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
                        "keywords": [],
                        "readme": null,
                        "repository": null,
                        "edition": "2015",
                        "links": null
                    }
                ],
                "workspace_members": [
                    "foo 0.0.1 (path+file:[..]/foo)"
                ],
                "resolve": null,
                "target_directory": "[..]/foo/target",
                "version": 1,
                "workspace_root": "[..]/foo"
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
                        "keywords": [],
                        "readme": null,
                        "repository": null,
                        "edition": "2015",
                        "links": null
                    },
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
                        "keywords": [],
                        "readme": null,
                        "repository": null,
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
                        "keywords": [],
                        "readme": null,
                        "repository": null,
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
                        "keywords": [],
                        "readme": null,
                        "repository": null,
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
                        "keywords": [],
                        "readme": null,
                        "repository": null,
                        "edition": "2015",
                        "links": null
                    }
                ],
                "workspace_members": [
                    "foo 0.0.1 (path+file:[..]/foo)"
                ],
                "resolve": "{...}",
                "target_directory": "[..]/foo/target",
                "version": 1,
                "workspace_root": "[..]/foo"
            }"#,
        )
        .run();
}

#[cargo_test]
fn unknown_registry() {
    // A known registry refers to an unknown registry.
    // foo -> bar(crates.io) -> baz(alt)
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
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

    p.cargo("build").run();

    // Important parts:
    // foo -> bar registry = null
    // bar -> baz registry = alternate
    p.cargo("metadata --format-version=1")
        .with_json(
            r#"
            {
              "packages": [
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
                  "keywords": [],
                  "readme": null,
                  "repository": null,
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
                  "keywords": [],
                  "readme": null,
                  "repository": null,
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
                  "keywords": [],
                  "readme": null,
                  "repository": null,
                  "edition": "2015",
                  "links": null
                }
              ],
              "workspace_members": [
                "foo 0.0.1 (path+file://[..]/foo)"
              ],
              "resolve": "{...}",
              "target_directory": "[..]/foo/target",
              "version": 1,
              "workspace_root": "[..]/foo"
            }
            "#,
        )
        .run();
}

#[cargo_test]
fn registries_index_relative_url() {
    let config = paths::root().join(".cargo/config");
    fs::create_dir_all(config.parent().unwrap()).unwrap();
    File::create(&config)
        .unwrap()
        .write_all(
            br#"
            [registries.relative]
            index = "file:alternative-registry"
        "#,
        )
        .unwrap();

    registry::init();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
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

    p.cargo("build")
        .with_stderr(&format!(
            "\
[UPDATING] `{reg}` index
[DOWNLOADING] crates ...
[DOWNLOADED] bar v0.0.1 (registry `[ROOT][..]`)
[COMPILING] bar v0.0.1 (registry `[ROOT][..]`)
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
",
            reg = registry::alt_registry_path().to_str().unwrap()
        ))
        .run();
}

#[cargo_test]
fn registry_index_relative_url() {
    let config = paths::root().join(".cargo/config");
    fs::create_dir_all(config.parent().unwrap()).unwrap();
    File::create(&config)
        .unwrap()
        .write_all(
            br#"
            [registry]
            index = "file:alternative-registry"
        "#,
        )
        .unwrap();

    registry::init();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
            name = "foo"
            version = "0.0.1"
            authors = []

            [dependencies.bar]
            version = "0.0.1"
        "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("bar", "0.0.1").alternative(true).publish();

    fs::remove_file(paths::home().join(".cargo/config")).unwrap();

    p.cargo("build")
        .with_stderr(&format!(
            "\
warning: custom registry support via the `registry.index` configuration is being removed, this functionality will not work in the future
[UPDATING] `{reg}` index
[DOWNLOADING] crates ...
[DOWNLOADED] bar v0.0.1 (registry `[ROOT][..]`)
[COMPILING] bar v0.0.1 (registry `[ROOT][..]`)
[COMPILING] foo v0.0.1 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]s
",
            reg = registry::alt_registry_path().to_str().unwrap()
        ))
        .run();
}

#[cargo_test]
fn registries_index_relative_path_not_allowed() {
    let config = paths::root().join(".cargo/config");
    fs::create_dir_all(config.parent().unwrap()).unwrap();
    File::create(&config)
        .unwrap()
        .write_all(
            br#"
            [registries.relative]
            index = "alternative-registry"
        "#,
        )
        .unwrap();

    registry::init();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [project]
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

    p.cargo("build")
        .with_stderr(&format!(
            "\
error: failed to parse manifest at `{root}/foo/Cargo.toml`

Caused by:
  invalid url `alternative-registry`: relative URL without a base
",
            root = paths::root().to_str().unwrap()
        ))
        .with_status(101)
        .run();
}
