//! Tests for inheriting Cargo.toml fields with field.workspace = true
use cargo_test_support::registry::{Dependency, Package, RegistryBuilder};
use cargo_test_support::{
    basic_lib_manifest, basic_manifest, git, path2url, paths, project, publish, registry,
};

#[cargo_test]
fn permit_additional_workspace_fields() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["bar"]
            [workspace.package]
            version = "1.2.3"
            authors = ["Rustaceans"]
            description = "This is a crate"
            documentation = "https://www.rust-lang.org/learn"
            readme = "README.md"
            homepage = "https://www.rust-lang.org"
            repository = "https://github.com/example/example"
            license = "MIT"
            license-file = "LICENSE"
            keywords = ["cli"]
            categories = ["development-tools"]
            publish = false
            edition = "2018"
            rust-version = "1.60"
            exclude = ["foo.txt"]
            include = ["bar.txt", "**/*.rs", "Cargo.toml", "LICENSE", "README.md"]

            [workspace.package.badges]
            gitlab = { repository = "https://gitlab.com/rust-lang/rust", branch = "master" }

            [workspace.dependencies]
            dep = "0.1"
        "#,
        )
        .file(
            "bar/Cargo.toml",
            r#"
              [package]
              name = "bar"
              version = "0.1.0"
              authors = []
              workspace = ".."
              "#,
        )
        .file("bar/src/main.rs", "fn main() {}")
        .build();

    p.cargo("check")
        // Should not warn about unused fields.
        .with_stderr(
            "\
[CHECKING] bar v0.1.0 ([CWD]/bar)
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();

    p.cargo("check").run();
    let lockfile = p.read_lockfile();
    assert!(!lockfile.contains("dep"));
}

#[cargo_test]
fn deny_optional_dependencies() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["bar"]

            [workspace.dependencies]
            dep1 = { version = "0.1", optional = true }
        "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "bar/Cargo.toml",
            r#"
              [package]
              name = "bar"
              version = "0.1.0"
              authors = []
              workspace = ".."
              "#,
        )
        .file("bar/src/main.rs", "fn main() {}")
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] failed to parse manifest at `[..]foo/Cargo.toml`

Caused by:
  dep1 is optional, but workspace dependencies cannot be optional
",
        )
        .run();
}

#[cargo_test]
fn inherit_own_workspace_fields() {
    let registry = RegistryBuilder::new().http_api().http_index().build();

    let p = project().build();

    let _ = git::repo(&paths::root().join("foo"))
        .file(
            "Cargo.toml",
            r#"
            badges.workspace = true

            [package]
            name = "foo"
            version.workspace = true
            authors.workspace = true
            description.workspace = true
            documentation.workspace = true
            homepage.workspace = true
            repository.workspace = true
            license.workspace = true
            keywords.workspace = true
            categories.workspace = true
            publish.workspace = true
            edition.workspace = true
            rust-version.workspace = true
            exclude.workspace = true
            include.workspace = true

            [workspace]
            members = []
            [workspace.package]
            version = "1.2.3"
            authors = ["Rustaceans"]
            description = "This is a crate"
            documentation = "https://www.rust-lang.org/learn"
            homepage = "https://www.rust-lang.org"
            repository = "https://github.com/example/example"
            license = "MIT"
            keywords = ["cli"]
            categories = ["development-tools"]
            publish = true
            edition = "2018"
            rust-version = "1.60"
            exclude = ["foo.txt"]
            include = ["bar.txt", "**/*.rs", "Cargo.toml"]
            [workspace.package.badges]
            gitlab = { repository = "https://gitlab.com/rust-lang/rust", branch = "master" }
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("foo.txt", "") // should be ignored when packaging
        .file("bar.txt", "") // should be included when packaging
        .build();

    p.cargo("publish")
        .replace_crates_io(registry.index_url())
        .with_stderr(
            "\
[UPDATING] [..]
[WARNING] [..]
[..]
[VERIFYING] foo v1.2.3 [..]
[COMPILING] foo v1.2.3 [..]
[FINISHED] [..]
[PACKAGED] [..]
[UPLOADING] foo v1.2.3 [..]
[UPLOADED] foo v1.2.3 to registry `crates-io`
note: Waiting for `foo v1.2.3` to be available at registry `crates-io`.
You may press ctrl-c to skip waiting; the crate should be available shortly.
[PUBLISHED] foo v1.2.3 at registry `crates-io`
",
        )
        .run();

    publish::validate_upload_with_contents(
        r#"
        {
          "authors": ["Rustaceans"],
          "badges": {
            "gitlab": { "branch": "master", "repository": "https://gitlab.com/rust-lang/rust" }
          },
          "categories": ["development-tools"],
          "deps": [],
          "description": "This is a crate",
          "documentation": "https://www.rust-lang.org/learn",
          "features": {},
          "homepage": "https://www.rust-lang.org",
          "keywords": ["cli"],
          "license": "MIT",
          "license_file": null,
          "links": null,
          "name": "foo",
          "readme": null,
          "readme_file": null,
          "repository": "https://github.com/example/example",
          "rust_version": "1.60",
          "vers": "1.2.3"
          }
        "#,
        "foo-1.2.3.crate",
        &[
            "Cargo.lock",
            "Cargo.toml",
            "Cargo.toml.orig",
            "src/main.rs",
            ".cargo_vcs_info.json",
            "bar.txt",
        ],
        &[(
            "Cargo.toml",
            &format!(
                r#"{}
[package]
edition = "2018"
rust-version = "1.60"
name = "foo"
version = "1.2.3"
authors = ["Rustaceans"]
exclude = ["foo.txt"]
include = [
    "bar.txt",
    "**/*.rs",
    "Cargo.toml",
]
publish = true
description = "This is a crate"
homepage = "https://www.rust-lang.org"
documentation = "https://www.rust-lang.org/learn"
keywords = ["cli"]
categories = ["development-tools"]
license = "MIT"
repository = "https://github.com/example/example"

[badges.gitlab]
branch = "master"
repository = "https://gitlab.com/rust-lang/rust"
"#,
                cargo::core::package::MANIFEST_PREAMBLE
            ),
        )],
    );
}

#[cargo_test]
fn inherit_own_dependencies() {
    let registry = RegistryBuilder::new().http_api().http_index().build();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "bar"
            version = "0.2.0"
            authors = []

            [dependencies]
            dep.workspace = true

            [build-dependencies]
            dep-build.workspace = true

            [dev-dependencies]
            dep-dev.workspace = true

            [workspace]
            members = []

            [workspace.dependencies]
            dep = "0.1"
            dep-build = "0.8"
            dep-dev = "0.5.2"
        "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("dep", "0.1.2").publish();
    Package::new("dep-build", "0.8.2").publish();
    Package::new("dep-dev", "0.5.2").publish();

    p.cargo("check")
        // Unordered because the download order is nondeterministic.
        .with_stderr_unordered(
            "\
[UPDATING] `[..]` index
[DOWNLOADING] crates ...
[DOWNLOADED] dep v0.1.2 ([..])
[DOWNLOADED] dep-build v0.8.2 ([..])
[CHECKING] dep v0.1.2
[CHECKING] bar v0.2.0 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();

    p.cargo("check").run();
    let lockfile = p.read_lockfile();
    assert!(lockfile.contains("dep"));
    assert!(lockfile.contains("dep-dev"));
    assert!(lockfile.contains("dep-build"));

    p.cargo("publish")
        .replace_crates_io(registry.index_url())
        .with_stderr(
            "\
[UPDATING] [..]
[WARNING] [..]
[..]
[PACKAGING] bar v0.2.0 [..]
[UPDATING] [..]
[VERIFYING] bar v0.2.0 [..]
[COMPILING] dep v0.1.2
[COMPILING] bar v0.2.0 [..]
[FINISHED] [..]
[PACKAGED] [..]
[UPLOADING] bar v0.2.0 [..]
[UPLOADED] bar v0.2.0 to registry `crates-io`
note: Waiting for `bar v0.2.0` to be available at registry `crates-io`.
You may press ctrl-c to skip waiting; the crate should be available shortly.
[PUBLISHED] bar v0.2.0 at registry `crates-io`
",
        )
        .run();

    publish::validate_upload_with_contents(
        r#"
        {
          "authors": [],
          "badges": {},
          "categories": [],
          "deps": [
            {
              "default_features": true,
              "features": [],
              "kind": "normal",
              "name": "dep",
              "optional": false,
              "target": null,
              "version_req": "^0.1"
            },
            {
              "default_features": true,
              "features": [],
              "kind": "dev",
              "name": "dep-dev",
              "optional": false,
              "target": null,
              "version_req": "^0.5.2"
            },
            {
              "default_features": true,
              "features": [],
              "kind": "build",
              "name": "dep-build",
              "optional": false,
              "target": null,
              "version_req": "^0.8"
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
          "name": "bar",
          "readme": null,
          "readme_file": null,
          "repository": null,
          "rust_version": null,
          "vers": "0.2.0"
          }
        "#,
        "bar-0.2.0.crate",
        &["Cargo.toml", "Cargo.toml.orig", "Cargo.lock", "src/main.rs"],
        &[(
            "Cargo.toml",
            &format!(
                r#"{}
[package]
name = "bar"
version = "0.2.0"
authors = []

[dependencies.dep]
version = "0.1"

[dev-dependencies.dep-dev]
version = "0.5.2"

[build-dependencies.dep-build]
version = "0.8"
"#,
                cargo::core::package::MANIFEST_PREAMBLE
            ),
        )],
    );
}

#[cargo_test]
fn inherit_own_detailed_dependencies() {
    let registry = RegistryBuilder::new().http_api().http_index().build();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "bar"
            version = "0.2.0"
            authors = []

            [dependencies]
            dep.workspace = true

            [workspace]
            members = []

            [workspace.dependencies]
            dep = { version = "0.1.2", features = ["testing"] }
        "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    Package::new("dep", "0.1.2")
        .feature("testing", &vec![])
        .publish();

    p.cargo("check")
        .with_stderr(
            "\
[UPDATING] `[..]` index
[DOWNLOADING] crates ...
[DOWNLOADED] dep v0.1.2 ([..])
[CHECKING] dep v0.1.2
[CHECKING] bar v0.2.0 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();

    p.cargo("check").run();
    let lockfile = p.read_lockfile();
    assert!(lockfile.contains("dep"));

    p.cargo("publish")
        .replace_crates_io(registry.index_url())
        .with_stderr(
            "\
[UPDATING] [..]
[WARNING] [..]
[..]
[PACKAGING] bar v0.2.0 [..]
[UPDATING] [..]
[VERIFYING] bar v0.2.0 [..]
[COMPILING] dep v0.1.2
[COMPILING] bar v0.2.0 [..]
[FINISHED] [..]
[PACKAGED] [..]
[UPLOADING] bar v0.2.0 [..]
[UPLOADED] bar v0.2.0 to registry `crates-io`
note: Waiting for `bar v0.2.0` to be available at registry `crates-io`.
You may press ctrl-c to skip waiting; the crate should be available shortly.
[PUBLISHED] bar v0.2.0 at registry `crates-io`
",
        )
        .run();

    publish::validate_upload_with_contents(
        r#"
        {
          "authors": [],
          "badges": {},
          "categories": [],
          "deps": [
            {
              "default_features": true,
              "features": ["testing"],
              "kind": "normal",
              "name": "dep",
              "optional": false,
              "target": null,
              "version_req": "^0.1.2"
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
          "name": "bar",
          "readme": null,
          "readme_file": null,
          "repository": null,
          "rust_version": null,
          "vers": "0.2.0"
          }
        "#,
        "bar-0.2.0.crate",
        &["Cargo.toml", "Cargo.toml.orig", "Cargo.lock", "src/main.rs"],
        &[(
            "Cargo.toml",
            &format!(
                r#"{}
[package]
name = "bar"
version = "0.2.0"
authors = []

[dependencies.dep]
version = "0.1.2"
features = ["testing"]
"#,
                cargo::core::package::MANIFEST_PREAMBLE
            ),
        )],
    );
}

#[cargo_test]
fn inherit_from_own_undefined_field() {
    registry::init();

    let p = project().build();

    let _ = git::repo(&paths::root().join("foo"))
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "1.2.5"
            authors = ["rustaceans"]
            description.workspace = true

            [workspace]
            members = []
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] failed to parse manifest at `[CWD]/Cargo.toml`

Caused by:
  error inheriting `description` from workspace root manifest's `workspace.package.description`

Caused by:
  `workspace.package.description` was not defined
",
        )
        .run();
}

#[cargo_test]
fn inherited_dependencies_union_features() {
    Package::new("dep", "0.1.0")
        .feature("fancy", &["fancy_dep"])
        .feature("dancy", &["dancy_dep"])
        .add_dep(Dependency::new("fancy_dep", "0.2").optional(true))
        .add_dep(Dependency::new("dancy_dep", "0.6").optional(true))
        .file("src/lib.rs", "")
        .publish();

    Package::new("fancy_dep", "0.2.4").publish();
    Package::new("dancy_dep", "0.6.8").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "bar"
            version = "0.2.0"
            authors = []
            [dependencies]
            dep = { workspace = true, features = ["dancy"] }

            [workspace]
            members = []
            [workspace.dependencies]
            dep = { version = "0.1", features = ["fancy"] }
        "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("check")
        .with_stderr(
            "\
[UPDATING] `[..]` index
[DOWNLOADING] crates ...
[DOWNLOADED] fancy_dep v0.2.4 ([..])
[DOWNLOADED] dep v0.1.0 ([..])
[DOWNLOADED] dancy_dep v0.6.8 ([..])
[CHECKING] [..]
[CHECKING] [..]
[CHECKING] dep v0.1.0
[CHECKING] bar v0.2.0 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();

    let lockfile = p.read_lockfile();
    assert!(lockfile.contains("dep"));
    assert!(lockfile.contains("fancy_dep"));
    assert!(lockfile.contains("dancy_dep"));
}

#[cargo_test]
fn inherit_workspace_fields() {
    let registry = RegistryBuilder::new().http_api().http_index().build();

    let p = project().build();

    let _ = git::repo(&paths::root().join("foo"))
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["bar"]
            [workspace.package]
            version = "1.2.3"
            authors = ["Rustaceans"]
            description = "This is a crate"
            documentation = "https://www.rust-lang.org/learn"
            readme = "README.md"
            homepage = "https://www.rust-lang.org"
            repository = "https://github.com/example/example"
            license = "MIT"
            license-file = "LICENSE"
            keywords = ["cli"]
            categories = ["development-tools"]
            publish = true
            edition = "2018"
            rust-version = "1.60"
            exclude = ["foo.txt"]
            include = ["bar.txt", "**/*.rs", "Cargo.toml", "LICENSE", "README.md"]
            [workspace.package.badges]
            gitlab = { repository = "https://gitlab.com/rust-lang/rust", branch = "master" }
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "bar/Cargo.toml",
            r#"
            badges.workspace = true
            [package]
            name = "bar"
            workspace = ".."
            version.workspace = true
            authors.workspace = true
            description.workspace = true
            documentation.workspace = true
            readme.workspace = true
            homepage.workspace = true
            repository.workspace = true
            license.workspace = true
            license-file.workspace = true
            keywords.workspace = true
            categories.workspace = true
            publish.workspace = true
            edition.workspace = true
            rust-version.workspace = true
            exclude.workspace = true
            include.workspace = true
        "#,
        )
        .file("LICENSE", "license")
        .file("README.md", "README.md")
        .file("bar/src/main.rs", "fn main() {}")
        .file("bar/foo.txt", "") // should be ignored when packaging
        .file("bar/bar.txt", "") // should be included when packaging
        .build();

    p.cargo("publish")
        .replace_crates_io(registry.index_url())
        .cwd("bar")
        .with_stderr(
            "\
[UPDATING] [..]
[WARNING] [..]
[..]
[VERIFYING] bar v1.2.3 [..]
[WARNING] [..]
[..]
[..]
[..]
[COMPILING] bar v1.2.3 [..]
[FINISHED] [..]
[PACKAGED] [..]
[UPLOADING] bar v1.2.3 [..]
[UPLOADED] bar v1.2.3 to registry `crates-io`
note: Waiting for `bar v1.2.3` to be available at registry `crates-io`.
You may press ctrl-c to skip waiting; the crate should be available shortly.
[PUBLISHED] bar v1.2.3 at registry `crates-io`
",
        )
        .run();

    publish::validate_upload_with_contents(
        r#"
        {
          "authors": ["Rustaceans"],
          "badges": {
            "gitlab": { "branch": "master", "repository": "https://gitlab.com/rust-lang/rust" }
          },
          "categories": ["development-tools"],
          "deps": [],
          "description": "This is a crate",
          "documentation": "https://www.rust-lang.org/learn",
          "features": {},
          "homepage": "https://www.rust-lang.org",
          "keywords": ["cli"],
          "license": "MIT",
          "license_file": "../LICENSE",
          "links": null,
          "name": "bar",
          "readme": "README.md",
          "readme_file": "../README.md",
          "repository": "https://github.com/example/example",
          "rust_version": "1.60",
          "vers": "1.2.3"
          }
        "#,
        "bar-1.2.3.crate",
        &[
            "Cargo.lock",
            "Cargo.toml",
            "Cargo.toml.orig",
            "src/main.rs",
            "README.md",
            "LICENSE",
            ".cargo_vcs_info.json",
            "bar.txt",
        ],
        &[(
            "Cargo.toml",
            &format!(
                r#"{}
[package]
edition = "2018"
rust-version = "1.60"
name = "bar"
version = "1.2.3"
authors = ["Rustaceans"]
exclude = ["foo.txt"]
include = [
    "bar.txt",
    "**/*.rs",
    "Cargo.toml",
    "LICENSE",
    "README.md",
]
publish = true
description = "This is a crate"
homepage = "https://www.rust-lang.org"
documentation = "https://www.rust-lang.org/learn"
readme = "README.md"
keywords = ["cli"]
categories = ["development-tools"]
license = "MIT"
license-file = "LICENSE"
repository = "https://github.com/example/example"

[badges.gitlab]
branch = "master"
repository = "https://gitlab.com/rust-lang/rust"
"#,
                cargo::core::package::MANIFEST_PREAMBLE
            ),
        )],
    );
}

#[cargo_test]
fn inherit_dependencies() {
    let registry = RegistryBuilder::new().http_api().http_index().build();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["bar"]
            [workspace.dependencies]
            dep = "0.1"
            dep-build = "0.8"
            dep-dev = "0.5.2"
        "#,
        )
        .file(
            "bar/Cargo.toml",
            r#"
            [package]
            workspace = ".."
            name = "bar"
            version = "0.2.0"
            authors = []
            [dependencies]
            dep.workspace = true
            [build-dependencies]
            dep-build.workspace = true
            [dev-dependencies]
            dep-dev.workspace = true
        "#,
        )
        .file("bar/src/main.rs", "fn main() {}")
        .build();

    Package::new("dep", "0.1.2").publish();
    Package::new("dep-build", "0.8.2").publish();
    Package::new("dep-dev", "0.5.2").publish();

    p.cargo("check")
        // Unordered because the download order is nondeterministic.
        .with_stderr_unordered(
            "\
[UPDATING] `[..]` index
[DOWNLOADING] crates ...
[DOWNLOADED] dep v0.1.2 ([..])
[DOWNLOADED] dep-build v0.8.2 ([..])
[CHECKING] dep v0.1.2
[CHECKING] bar v0.2.0 ([CWD]/bar)
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();

    p.cargo("check").run();
    let lockfile = p.read_lockfile();
    assert!(lockfile.contains("dep"));
    assert!(lockfile.contains("dep-dev"));
    assert!(lockfile.contains("dep-build"));

    p.cargo("publish")
        .replace_crates_io(registry.index_url())
        .cwd("bar")
        .with_stderr(
            "\
[UPDATING] [..]
[WARNING] [..]
[..]
[PACKAGING] bar v0.2.0 [..]
[UPDATING] [..]
[VERIFYING] bar v0.2.0 [..]
[COMPILING] dep v0.1.2
[COMPILING] bar v0.2.0 [..]
[FINISHED] [..]
[PACKAGED] [..]
[UPLOADING] bar v0.2.0 [..]
[UPLOADED] bar v0.2.0 to registry `crates-io`
note: Waiting for `bar v0.2.0` to be available at registry `crates-io`.
You may press ctrl-c to skip waiting; the crate should be available shortly.
[PUBLISHED] bar v0.2.0 at registry `crates-io`
",
        )
        .run();

    publish::validate_upload_with_contents(
        r#"
        {
          "authors": [],
          "badges": {},
          "categories": [],
          "deps": [
            {
              "default_features": true,
              "features": [],
              "kind": "normal",
              "name": "dep",
              "optional": false,
              "target": null,
              "version_req": "^0.1"
            },
            {
              "default_features": true,
              "features": [],
              "kind": "dev",
              "name": "dep-dev",
              "optional": false,
              "target": null,
              "version_req": "^0.5.2"
            },
            {
              "default_features": true,
              "features": [],
              "kind": "build",
              "name": "dep-build",
              "optional": false,
              "target": null,
              "version_req": "^0.8"
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
          "name": "bar",
          "readme": null,
          "readme_file": null,
          "repository": null,
          "rust_version": null,
          "vers": "0.2.0"
          }
        "#,
        "bar-0.2.0.crate",
        &["Cargo.toml", "Cargo.toml.orig", "Cargo.lock", "src/main.rs"],
        &[(
            "Cargo.toml",
            &format!(
                r#"{}
[package]
name = "bar"
version = "0.2.0"
authors = []

[dependencies.dep]
version = "0.1"

[dev-dependencies.dep-dev]
version = "0.5.2"

[build-dependencies.dep-build]
version = "0.8"
"#,
                cargo::core::package::MANIFEST_PREAMBLE
            ),
        )],
    );
}

#[cargo_test]
fn inherit_target_dependencies() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["bar"]
            [workspace.dependencies]
            dep = "0.1"
        "#,
        )
        .file(
            "bar/Cargo.toml",
            r#"
            [package]
            workspace = ".."
            name = "bar"
            version = "0.2.0"
            authors = []
            [target.'cfg(unix)'.dependencies]
            dep.workspace = true
            [target.'cfg(windows)'.dependencies]
            dep.workspace = true
        "#,
        )
        .file("bar/src/main.rs", "fn main() {}")
        .build();

    Package::new("dep", "0.1.2").publish();

    p.cargo("check")
        .with_stderr(
            "\
[UPDATING] `[..]` index
[DOWNLOADING] crates ...
[DOWNLOADED] dep v0.1.2 ([..])
[CHECKING] dep v0.1.2
[CHECKING] bar v0.2.0 ([CWD]/bar)
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();

    let lockfile = p.read_lockfile();
    assert!(lockfile.contains("dep"));
}

#[cargo_test]
fn inherit_dependency_override_optional() {
    Package::new("dep", "0.1.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["bar"]
            [workspace.dependencies]
            dep = "0.1.0"
        "#,
        )
        .file(
            "bar/Cargo.toml",
            r#"
            [package]
            workspace = ".."
            name = "bar"
            version = "0.2.0"
            authors = []
            [dependencies]
            dep = { workspace = true, optional = true }
        "#,
        )
        .file("bar/src/main.rs", "fn main() {}")
        .build();

    p.cargo("check")
        .with_stderr(
            "\
[UPDATING] `[..]` index
[CHECKING] bar v0.2.0 ([CWD]/bar)
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cargo_test]
fn inherit_dependency_features() {
    Package::new("dep", "0.1.0")
        .feature("fancy", &["fancy_dep"])
        .add_dep(Dependency::new("fancy_dep", "0.2").optional(true))
        .file("src/lib.rs", "")
        .publish();

    Package::new("fancy_dep", "0.2.4").publish();
    Package::new("dancy_dep", "0.6.8").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "bar"
            version = "0.2.0"
            authors = []
            [dependencies]
            dep = { workspace = true, features = ["fancy"] }

            [workspace]
            members = []
            [workspace.dependencies]
            dep = "0.1"
        "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("check")
        .with_stderr(
            "\
[UPDATING] `[..]` index
[DOWNLOADING] crates ...
[DOWNLOADED] fancy_dep v0.2.4 ([..])
[DOWNLOADED] dep v0.1.0 ([..])
[CHECKING] fancy_dep v0.2.4
[CHECKING] dep v0.1.0
[CHECKING] bar v0.2.0 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();

    let lockfile = p.read_lockfile();
    assert!(lockfile.contains("dep"));
    assert!(lockfile.contains("fancy_dep"));
}

#[cargo_test]
fn inherit_detailed_dependencies() {
    let git_project = git::new("detailed", |project| {
        project
            .file("Cargo.toml", &basic_lib_manifest("detailed"))
            .file(
                "src/detailed.rs",
                r#"
                pub fn hello() -> &'static str {
                    "hello world"
                }
            "#,
            )
    });

    // Make a new branch based on the current HEAD commit
    let repo = git2::Repository::open(&git_project.root()).unwrap();
    let head = repo.head().unwrap().target().unwrap();
    let head = repo.find_commit(head).unwrap();
    repo.branch("branchy", &head, true).unwrap();

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
            [workspace]
            members = ["bar"]
            [workspace.dependencies]
            detailed = {{ git = '{}', branch = "branchy" }}
        "#,
                git_project.url()
            ),
        )
        .file(
            "bar/Cargo.toml",
            r#"
            [package]
            workspace = ".."
            name = "bar"
            version = "0.2.0"
            authors = []
            [dependencies]
            detailed.workspace = true
        "#,
        )
        .file("bar/src/main.rs", "fn main() {}")
        .build();

    let git_root = git_project.root();

    p.cargo("check")
        .with_stderr(&format!(
            "\
[UPDATING] git repository `{}`\n\
[CHECKING] detailed v0.5.0 ({}?branch=branchy#[..])\n\
[CHECKING] bar v0.2.0 ([CWD]/bar)\n\
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]\n",
            path2url(&git_root),
            path2url(&git_root),
        ))
        .run();
}

#[cargo_test]
fn inherit_path_dependencies() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["bar"]
            [workspace.dependencies]
            dep = { path = "dep" }
        "#,
        )
        .file(
            "bar/Cargo.toml",
            r#"
            [package]
            workspace = ".."
            name = "bar"
            version = "0.2.0"
            authors = []
            [dependencies]
            dep.workspace = true
        "#,
        )
        .file("bar/src/main.rs", "fn main() {}")
        .file("dep/Cargo.toml", &basic_manifest("dep", "0.9.0"))
        .file("dep/src/lib.rs", "")
        .build();

    p.cargo("check")
        .with_stderr(
            "\
[CHECKING] dep v0.9.0 ([CWD]/dep)
[CHECKING] bar v0.2.0 ([CWD]/bar)
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();

    let lockfile = p.read_lockfile();
    assert!(lockfile.contains("dep"));
}

#[cargo_test]
fn error_workspace_false() {
    registry::init();

    let p = project().build();

    let _ = git::repo(&paths::root().join("foo"))
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["bar"]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "bar/Cargo.toml",
            r#"
            [package]
            name = "bar"
            workspace = ".."
            version = "1.2.3"
            authors = ["rustaceans"]
            description = { workspace = false }
        "#,
        )
        .file("bar/src/main.rs", "fn main() {}")
        .build();

    p.cargo("check")
        .cwd("bar")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] failed to parse manifest at `[CWD]/Cargo.toml`

Caused by:
  TOML parse error at line 7, column 41
    |
  7 |             description = { workspace = false }
    |                                         ^^^^^
  `workspace` cannot be false
",
        )
        .run();
}

#[cargo_test]
fn error_workspace_dependency_looked_for_workspace_itself() {
    registry::init();

    let p = project().build();

    let _ = git::repo(&paths::root().join("foo"))
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "bar"
            version = "1.2.3"

            [dependencies]
            dep.workspace = true

            [workspace]
            members = []

            [workspace.dependencies]
            dep.workspace = true

            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr(
            "\
[WARNING] [CWD]/Cargo.toml: unused manifest key: workspace.dependencies.dep.workspace
[WARNING] [CWD]/Cargo.toml: dependency (dep) specified without providing a local path, Git repository, version, \
or workspace dependency to use. \
This will be considered an error in future versions
[UPDATING] `dummy-registry` index
[ERROR] no matching package named `dep` found
location searched: registry `crates-io`
required by package `bar v1.2.3 ([CWD])`
",
        )
        .run();
}

#[cargo_test]
fn error_malformed_workspace_root() {
    registry::init();

    let p = project().build();

    let _ = git::repo(&paths::root().join("foo"))
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = [invalid toml
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "bar/Cargo.toml",
            r#"
            [package]
            name = "bar"
            workspace = ".."
            version = "1.2.3"
            authors = ["rustaceans"]
        "#,
        )
        .file("bar/src/main.rs", "fn main() {}")
        .build();

    p.cargo("check")
        .cwd("bar")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] failed to parse manifest at `[..]/foo/Cargo.toml`

Caused by:
  [..]
    |
  3 |             members = [invalid toml
    |                        ^
  invalid array
  expected `]`
",
        )
        .run();
}

#[cargo_test]
fn error_no_root_workspace() {
    registry::init();

    let p = project().build();

    let _ = git::repo(&paths::root().join("foo"))
        .file(
            "bar/Cargo.toml",
            r#"
            [package]
            name = "bar"
            workspace = ".."
            version = "1.2.3"
            authors = ["rustaceans"]
            description.workspace = true
        "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("bar/src/main.rs", "fn main() {}")
        .build();

    p.cargo("check")
        .cwd("bar")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] failed to parse manifest at `[..]/Cargo.toml`

Caused by:
  error inheriting `description` from workspace root manifest's `workspace.package.description`

Caused by:
  root of a workspace inferred but wasn't a root: [..]/Cargo.toml
",
        )
        .run();
}

#[cargo_test]
fn error_inherit_unspecified_dependency() {
    let p = project().build();

    let _ = git::repo(&paths::root().join("foo"))
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["bar"]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "bar/Cargo.toml",
            r#"
            [package]
            name = "bar"
            workspace = ".."
            version = "1.2.3"
            authors = ["rustaceans"]
            [dependencies]
            foo.workspace = true
        "#,
        )
        .file("bar/src/main.rs", "fn main() {}")
        .build();

    p.cargo("check")
        .cwd("bar")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] failed to parse manifest at `[CWD]/Cargo.toml`

Caused by:
  error inheriting `foo` from workspace root manifest's `workspace.dependencies.foo`

Caused by:
  `workspace.dependencies` was not defined
",
        )
        .run();
}

#[cargo_test]
fn warn_inherit_def_feat_true_member_def_feat_false() {
    Package::new("dep", "0.1.0")
        .feature("default", &["fancy_dep"])
        .add_dep(Dependency::new("fancy_dep", "0.2").optional(true))
        .file("src/lib.rs", "")
        .publish();

    Package::new("fancy_dep", "0.2.4").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "bar"
            version = "0.2.0"
            authors = []
            [dependencies]
            dep = { workspace = true, default-features = false }

            [workspace]
            members = []
            [workspace.dependencies]
            dep = { version = "0.1.0", default-features = true }
        "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("check")
        .with_stderr(
            "\
[WARNING] [CWD]/Cargo.toml: `default-features` is ignored for dep, since `default-features` was \
true for `workspace.dependencies.dep`, this could become a hard error in the future
[UPDATING] `dummy-registry` index
[DOWNLOADING] crates ...
[DOWNLOADED] fancy_dep v0.2.4 ([..])
[DOWNLOADED] dep v0.1.0 ([..])
[CHECKING] fancy_dep v0.2.4
[CHECKING] dep v0.1.0
[CHECKING] bar v0.2.0 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cargo_test]
fn warn_inherit_simple_member_def_feat_false() {
    Package::new("dep", "0.1.0")
        .feature("default", &["fancy_dep"])
        .add_dep(Dependency::new("fancy_dep", "0.2").optional(true))
        .file("src/lib.rs", "")
        .publish();

    Package::new("fancy_dep", "0.2.4").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "bar"
            version = "0.2.0"
            authors = []
            [dependencies]
            dep = { workspace = true, default-features = false }

            [workspace]
            members = []
            [workspace.dependencies]
            dep = "0.1.0"
        "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("check")
        .with_stderr(
            "\
[WARNING] [CWD]/Cargo.toml: `default-features` is ignored for dep, since `default-features` was \
not specified for `workspace.dependencies.dep`, this could become a hard error in the future
[UPDATING] `dummy-registry` index
[DOWNLOADING] crates ...
[DOWNLOADED] fancy_dep v0.2.4 ([..])
[DOWNLOADED] dep v0.1.0 ([..])
[CHECKING] fancy_dep v0.2.4
[CHECKING] dep v0.1.0
[CHECKING] bar v0.2.0 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cargo_test]
fn inherit_def_feat_false_member_def_feat_true() {
    Package::new("dep", "0.1.0")
        .feature("default", &["fancy_dep"])
        .add_dep(Dependency::new("fancy_dep", "0.2").optional(true))
        .file("src/lib.rs", "")
        .publish();

    Package::new("fancy_dep", "0.2.4").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "bar"
            version = "0.2.0"
            authors = []
            [dependencies]
            dep = { workspace = true, default-features = true }

            [workspace]
            members = []
            [workspace.dependencies]
            dep = { version = "0.1.0", default-features = false }
        "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("check")
        .with_stderr(
            "\
[UPDATING] `dummy-registry` index
[DOWNLOADING] crates ...
[DOWNLOADED] fancy_dep v0.2.4 ([..])
[DOWNLOADED] dep v0.1.0 ([..])
[CHECKING] fancy_dep v0.2.4
[CHECKING] dep v0.1.0
[CHECKING] bar v0.2.0 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cargo_test]
fn cannot_inherit_in_patch() {
    Package::new("bar", "0.1.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = []

            [workspace.dependencies]
            bar = { path = "bar" }

            [package]
            name = "foo"
            version = "0.2.0"

            [patch.crates-io]
            bar.workspace = true

            [dependencies]
            bar = "0.1.0"

        "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr(
            "\
[WARNING] [CWD]/Cargo.toml: unused manifest key: patch.crates-io.bar.workspace
[WARNING] [CWD]/Cargo.toml: dependency (bar) specified without providing a local path, Git repository, version, \
or workspace dependency to use. \
This will be considered an error in future versions
[UPDATING] `dummy-registry` index
[ERROR] failed to resolve patches for `https://github.com/rust-lang/crates.io-index`

Caused by:
  patch for `bar` in `https://github.com/rust-lang/crates.io-index` points to the same source, but patches must point to different sources
",
        )
        .run();
}

#[cargo_test]
fn warn_inherit_unused_manifest_key_dep() {
    Package::new("dep", "0.1.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = []
            [workspace.dependencies]
            dep = { version = "0.1", wxz = "wxz" }

            [package]
            name = "bar"
            version = "0.2.0"
            authors = []

            [dependencies]
            dep = { workspace = true, wxz = "wxz" }
        "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("check")
        .with_stderr(
            "\
[WARNING] [CWD]/Cargo.toml: unused manifest key: workspace.dependencies.dep.wxz
[WARNING] [CWD]/Cargo.toml: unused manifest key: dependencies.dep.wxz
[UPDATING] `[..]` index
[DOWNLOADING] crates ...
[DOWNLOADED] dep v0.1.0 ([..])
[CHECKING] [..]
[CHECKING] bar v0.2.0 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}

#[cargo_test]
fn warn_inherit_unused_manifest_key_package() {
    Package::new("dep", "0.1.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            badges = { workspace = true, xyz = "abc"}

            [workspace]
            members = []
            [workspace.package]
            version = "1.2.3"
            authors = ["Rustaceans"]
            description = "This is a crate"
            documentation = "https://www.rust-lang.org/learn"
            homepage = "https://www.rust-lang.org"
            repository = "https://github.com/example/example"
            license = "MIT"
            keywords = ["cli"]
            categories = ["development-tools"]
            publish = true
            edition = "2018"
            rust-version = "1.60"
            exclude = ["foo.txt"]
            include = ["bar.txt", "**/*.rs", "Cargo.toml"]
            [workspace.package.badges]
            gitlab = { repository = "https://gitlab.com/rust-lang/rust", branch = "master" }

            [package]
            name = "bar"
            version = { workspace = true, xyz = "abc"}
            authors = { workspace = true, xyz = "abc"}
            description = { workspace = true, xyz = "abc"}
            documentation = { workspace = true, xyz = "abc"}
            homepage = { workspace = true, xyz = "abc"}
            repository = { workspace = true, xyz = "abc"}
            license = { workspace = true, xyz = "abc"}
            keywords = { workspace = true, xyz = "abc"}
            categories = { workspace = true, xyz = "abc"}
            publish = { workspace = true, xyz = "abc"}
            edition = { workspace = true, xyz = "abc"}
            rust-version = { workspace = true, xyz = "abc"}
            exclude = { workspace = true, xyz = "abc"}
            include = { workspace = true, xyz = "abc"}
        "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("check")
        .with_stderr(
            "\
[WARNING] [CWD]/Cargo.toml: unused manifest key: package.authors.xyz
[WARNING] [CWD]/Cargo.toml: unused manifest key: package.categories.xyz
[WARNING] [CWD]/Cargo.toml: unused manifest key: package.description.xyz
[WARNING] [CWD]/Cargo.toml: unused manifest key: package.documentation.xyz
[WARNING] [CWD]/Cargo.toml: unused manifest key: package.edition.xyz
[WARNING] [CWD]/Cargo.toml: unused manifest key: package.exclude.xyz
[WARNING] [CWD]/Cargo.toml: unused manifest key: package.homepage.xyz
[WARNING] [CWD]/Cargo.toml: unused manifest key: package.include.xyz
[WARNING] [CWD]/Cargo.toml: unused manifest key: package.keywords.xyz
[WARNING] [CWD]/Cargo.toml: unused manifest key: package.license.xyz
[WARNING] [CWD]/Cargo.toml: unused manifest key: package.publish.xyz
[WARNING] [CWD]/Cargo.toml: unused manifest key: package.repository.xyz
[WARNING] [CWD]/Cargo.toml: unused manifest key: package.rust-version.xyz
[WARNING] [CWD]/Cargo.toml: unused manifest key: package.version.xyz
[CHECKING] bar v1.2.3 ([CWD])
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
",
        )
        .run();
}
