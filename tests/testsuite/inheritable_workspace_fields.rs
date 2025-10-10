//! Tests for inheriting Cargo.toml fields with field.workspace = true

use crate::prelude::*;
use cargo_test_support::registry::{Dependency, Package, RegistryBuilder};
use cargo_test_support::{
    basic_lib_manifest, basic_manifest, git, paths, project, publish, registry, str,
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
              edition = "2015"
              authors = []
              workspace = ".."
              "#,
        )
        .file("bar/src/main.rs", "fn main() {}")
        .build();

    p.cargo("check")
        // Should not warn about unused fields.
        .with_stderr_data(str![[r#"
[CHECKING] bar v0.1.0 ([ROOT]/foo/bar)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
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
              edition = "2015"
              authors = []
              workspace = ".."
              "#,
        )
        .file("bar/src/main.rs", "fn main() {}")
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  dep1 is optional, but workspace dependencies cannot be optional

"#]])
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
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("foo.txt", "") // should be ignored when packaging
        .file("bar.txt", "") // should be included when packaging
        .build();

    p.cargo("publish")
        .replace_crates_io(registry.index_url())
        .with_stderr_data(str![[r#"
[UPDATING] crates.io index
[WARNING] both package.include and package.exclude are specified; the exclude list will be ignored
[PACKAGING] foo v1.2.3 ([ROOT]/foo)
[PACKAGED] 6 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] foo v1.2.3 ([ROOT]/foo)
[COMPILING] foo v1.2.3 ([ROOT]/foo/target/package/foo-1.2.3)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[UPLOADING] foo v1.2.3 ([ROOT]/foo)
[UPLOADED] foo v1.2.3 to registry `crates-io`
[NOTE] waiting for foo v1.2.3 to be available at registry `crates-io`
[HELP] you may press ctrl-c to skip waiting; the crate should be available shortly
[PUBLISHED] foo v1.2.3 at registry `crates-io`

"#]])
        .run();

    publish::validate_upload_with_contents(
        r#"
        {
          "authors": ["Rustaceans"],
          "badges": {},
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
        [(
            "Cargo.toml",
            str![[r##"
# THIS FILE IS AUTOMATICALLY GENERATED BY CARGO
#
# When uploading crates to the registry Cargo will automatically
# "normalize" Cargo.toml files for maximal compatibility
# with all versions of Cargo and also rewrite `path` dependencies
# to registry (e.g., crates.io) dependencies.
#
# If you are reading this file be aware that the original Cargo.toml
# will likely look very different (and much more reasonable).
# See Cargo.toml.orig for the original contents.

[package]
edition = "2018"
rust-version = "1.60"
name = "foo"
version = "1.2.3"
authors = ["Rustaceans"]
build = false
exclude = ["foo.txt"]
include = [
    "bar.txt",
    "**/*.rs",
    "Cargo.toml",
]
publish = true
autolib = false
autobins = false
autoexamples = false
autotests = false
autobenches = false
description = "This is a crate"
homepage = "https://www.rust-lang.org"
documentation = "https://www.rust-lang.org/learn"
readme = false
keywords = ["cli"]
categories = ["development-tools"]
license = "MIT"
repository = "https://github.com/example/example"

[[bin]]
name = "foo"
path = "src/main.rs"

"##]],
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
            edition = "2015"
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
        .with_stderr_data(
            str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 3 packages to latest compatible versions
[DOWNLOADING] crates ...
[DOWNLOADED] dep v0.1.2 (registry `dummy-registry`)
[DOWNLOADED] dep-build v0.8.2 (registry `dummy-registry`)
[CHECKING] dep v0.1.2
[CHECKING] bar v0.2.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]
            .unordered(),
        )
        .run();

    p.cargo("check").run();
    let lockfile = p.read_lockfile();
    assert!(lockfile.contains("dep"));
    assert!(lockfile.contains("dep-dev"));
    assert!(lockfile.contains("dep-build"));

    p.cargo("publish")
        .replace_crates_io(registry.index_url())
        .with_stderr_data(str![[r#"
[UPDATING] crates.io index
[WARNING] manifest has no description, license, license-file, documentation, homepage or repository
  |
  = [NOTE] see https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info
[PACKAGING] bar v0.2.0 ([ROOT]/foo)
[UPDATING] crates.io index
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] bar v0.2.0 ([ROOT]/foo)
[COMPILING] dep v0.1.2
[COMPILING] bar v0.2.0 ([ROOT]/foo/target/package/bar-0.2.0)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[UPLOADING] bar v0.2.0 ([ROOT]/foo)
[UPLOADED] bar v0.2.0 to registry `crates-io`
[NOTE] waiting for bar v0.2.0 to be available at registry `crates-io`
[HELP] you may press ctrl-c to skip waiting; the crate should be available shortly
[PUBLISHED] bar v0.2.0 at registry `crates-io`

"#]])
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
        [(
            "Cargo.toml",
            str![[r##"
# THIS FILE IS AUTOMATICALLY GENERATED BY CARGO
#
# When uploading crates to the registry Cargo will automatically
# "normalize" Cargo.toml files for maximal compatibility
# with all versions of Cargo and also rewrite `path` dependencies
# to registry (e.g., crates.io) dependencies.
#
# If you are reading this file be aware that the original Cargo.toml
# will likely look very different (and much more reasonable).
# See Cargo.toml.orig for the original contents.

[package]
edition = "2015"
name = "bar"
version = "0.2.0"
authors = []
build = false
autolib = false
autobins = false
autoexamples = false
autotests = false
autobenches = false
readme = false

[[bin]]
name = "bar"
path = "src/main.rs"

[dependencies.dep]
version = "0.1"

[dev-dependencies.dep-dev]
version = "0.5.2"

[build-dependencies.dep-build]
version = "0.8"

"##]],
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
            edition = "2015"
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
        .feature("testing", &[])
        .publish();

    p.cargo("check")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] dep v0.1.2 (registry `dummy-registry`)
[CHECKING] dep v0.1.2
[CHECKING] bar v0.2.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    p.cargo("check").run();
    let lockfile = p.read_lockfile();
    assert!(lockfile.contains("dep"));

    p.cargo("publish")
        .replace_crates_io(registry.index_url())
        .with_stderr_data(str![[r#"
[UPDATING] crates.io index
[WARNING] manifest has no description, license, license-file, documentation, homepage or repository
  |
  = [NOTE] see https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info
[PACKAGING] bar v0.2.0 ([ROOT]/foo)
[UPDATING] crates.io index
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] bar v0.2.0 ([ROOT]/foo)
[COMPILING] dep v0.1.2
[COMPILING] bar v0.2.0 ([ROOT]/foo/target/package/bar-0.2.0)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[UPLOADING] bar v0.2.0 ([ROOT]/foo)
[UPLOADED] bar v0.2.0 to registry `crates-io`
[NOTE] waiting for bar v0.2.0 to be available at registry `crates-io`
[HELP] you may press ctrl-c to skip waiting; the crate should be available shortly
[PUBLISHED] bar v0.2.0 at registry `crates-io`

"#]])
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
        [(
            "Cargo.toml",
            str![[r##"
# THIS FILE IS AUTOMATICALLY GENERATED BY CARGO
#
# When uploading crates to the registry Cargo will automatically
# "normalize" Cargo.toml files for maximal compatibility
# with all versions of Cargo and also rewrite `path` dependencies
# to registry (e.g., crates.io) dependencies.
#
# If you are reading this file be aware that the original Cargo.toml
# will likely look very different (and much more reasonable).
# See Cargo.toml.orig for the original contents.

[package]
edition = "2015"
name = "bar"
version = "0.2.0"
authors = []
build = false
autolib = false
autobins = false
autoexamples = false
autotests = false
autobenches = false
readme = false

[[bin]]
name = "bar"
path = "src/main.rs"

[dependencies.dep]
version = "0.1.2"
features = ["testing"]

"##]],
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
            edition = "2015"
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
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  error inheriting `description` from workspace root manifest's `workspace.package.description`

Caused by:
  `workspace.package.description` was not defined

"#]])
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
            edition = "2015"
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
        .with_stderr_data(
            str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 3 packages to latest compatible versions
[DOWNLOADING] crates ...
[DOWNLOADED] fancy_dep v0.2.4 (registry `dummy-registry`)
[DOWNLOADED] dep v0.1.0 (registry `dummy-registry`)
[DOWNLOADED] dancy_dep v0.6.8 (registry `dummy-registry`)
[CHECKING] fancy_dep v0.2.4
[CHECKING] dancy_dep v0.6.8
[CHECKING] dep v0.1.0
[CHECKING] bar v0.2.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]
            .unordered(),
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
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "bar/Cargo.toml",
            r#"
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
        .with_stderr_data(str![[r#"
[UPDATING] crates.io index
[WARNING] both package.include and package.exclude are specified; the exclude list will be ignored
[PACKAGING] bar v1.2.3 ([ROOT]/foo/bar)
[PACKAGED] 8 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] bar v1.2.3 ([ROOT]/foo/bar)
[WARNING] only one of `license` or `license-file` is necessary
`license` should be used if the package license can be expressed with a standard SPDX expression.
`license-file` should be used if the package uses a non-standard license.
See https://doc.rust-lang.org/cargo/reference/manifest.html#the-license-and-license-file-fields for more information.
[COMPILING] bar v1.2.3 ([ROOT]/foo/target/package/bar-1.2.3)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[UPLOADING] bar v1.2.3 ([ROOT]/foo/bar)
[UPLOADED] bar v1.2.3 to registry `crates-io`
[NOTE] waiting for bar v1.2.3 to be available at registry `crates-io`
[HELP] you may press ctrl-c to skip waiting; the crate should be available shortly
[PUBLISHED] bar v1.2.3 at registry `crates-io`

"#]])
        .run();

    publish::validate_upload_with_contents(
        r#"
        {
          "authors": ["Rustaceans"],
          "badges": {},
          "categories": ["development-tools"],
          "deps": [],
          "description": "This is a crate",
          "documentation": "https://www.rust-lang.org/learn",
          "features": {},
          "homepage": "https://www.rust-lang.org",
          "keywords": ["cli"],
          "license": "MIT",
          "license_file": "LICENSE",
          "links": null,
          "name": "bar",
          "readme": "README.md",
          "readme_file": "README.md",
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
        [(
            "Cargo.toml",
            str![[r##"
# THIS FILE IS AUTOMATICALLY GENERATED BY CARGO
#
# When uploading crates to the registry Cargo will automatically
# "normalize" Cargo.toml files for maximal compatibility
# with all versions of Cargo and also rewrite `path` dependencies
# to registry (e.g., crates.io) dependencies.
#
# If you are reading this file be aware that the original Cargo.toml
# will likely look very different (and much more reasonable).
# See Cargo.toml.orig for the original contents.

[package]
edition = "2018"
rust-version = "1.60"
name = "bar"
version = "1.2.3"
authors = ["Rustaceans"]
build = false
exclude = ["foo.txt"]
include = [
    "bar.txt",
    "**/*.rs",
    "Cargo.toml",
    "LICENSE",
    "README.md",
]
publish = true
autolib = false
autobins = false
autoexamples = false
autotests = false
autobenches = false
description = "This is a crate"
homepage = "https://www.rust-lang.org"
documentation = "https://www.rust-lang.org/learn"
readme = "README.md"
keywords = ["cli"]
categories = ["development-tools"]
license = "MIT"
license-file = "LICENSE"
repository = "https://github.com/example/example"

[[bin]]
name = "bar"
path = "src/main.rs"

"##]],
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
            edition = "2015"
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
        .with_stderr_data(
            str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 3 packages to latest compatible versions
[DOWNLOADING] crates ...
[DOWNLOADED] dep v0.1.2 (registry `dummy-registry`)
[DOWNLOADED] dep-build v0.8.2 (registry `dummy-registry`)
[CHECKING] dep v0.1.2
[CHECKING] bar v0.2.0 ([ROOT]/foo/bar)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]
            .unordered(),
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
        .with_stderr_data(str![[r#"
[UPDATING] crates.io index
[WARNING] manifest has no description, license, license-file, documentation, homepage or repository
  |
  = [NOTE] see https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info
[PACKAGING] bar v0.2.0 ([ROOT]/foo/bar)
[UPDATING] crates.io index
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] bar v0.2.0 ([ROOT]/foo/bar)
[COMPILING] dep v0.1.2
[COMPILING] bar v0.2.0 ([ROOT]/foo/target/package/bar-0.2.0)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[UPLOADING] bar v0.2.0 ([ROOT]/foo/bar)
[UPLOADED] bar v0.2.0 to registry `crates-io`
[NOTE] waiting for bar v0.2.0 to be available at registry `crates-io`
[HELP] you may press ctrl-c to skip waiting; the crate should be available shortly
[PUBLISHED] bar v0.2.0 at registry `crates-io`

"#]])
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
        [(
            "Cargo.toml",
            str![[r##"
# THIS FILE IS AUTOMATICALLY GENERATED BY CARGO
#
# When uploading crates to the registry Cargo will automatically
# "normalize" Cargo.toml files for maximal compatibility
# with all versions of Cargo and also rewrite `path` dependencies
# to registry (e.g., crates.io) dependencies.
#
# If you are reading this file be aware that the original Cargo.toml
# will likely look very different (and much more reasonable).
# See Cargo.toml.orig for the original contents.

[package]
edition = "2015"
name = "bar"
version = "0.2.0"
authors = []
build = false
autolib = false
autobins = false
autoexamples = false
autotests = false
autobenches = false
readme = false

[[bin]]
name = "bar"
path = "src/main.rs"

[dependencies.dep]
version = "0.1"

[dev-dependencies.dep-dev]
version = "0.5.2"

[build-dependencies.dep-build]
version = "0.8"

"##]],
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
            edition = "2015"
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
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] dep v0.1.2 (registry `dummy-registry`)
[CHECKING] dep v0.1.2
[CHECKING] bar v0.2.0 ([ROOT]/foo/bar)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
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
            edition = "2015"
            authors = []
            [dependencies]
            dep = { workspace = true, optional = true }
        "#,
        )
        .file("bar/src/main.rs", "fn main() {}")
        .build();

    p.cargo("check")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[CHECKING] bar v0.2.0 ([ROOT]/foo/bar)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
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
            edition = "2015"
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
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 2 packages to latest compatible versions
[DOWNLOADING] crates ...
[DOWNLOADED] fancy_dep v0.2.4 (registry `dummy-registry`)
[DOWNLOADED] dep v0.1.0 (registry `dummy-registry`)
[CHECKING] fancy_dep v0.2.4
[CHECKING] dep v0.1.0
[CHECKING] bar v0.2.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
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
            edition = "2015"
            authors = []
            [dependencies]
            detailed.workspace = true
        "#,
        )
        .file("bar/src/main.rs", "fn main() {}")
        .build();

    p.cargo("check")
        .with_stderr_data(str![[r#"
[UPDATING] git repository `[ROOTURL]/detailed`
[LOCKING] 1 package to latest compatible version
[CHECKING] detailed v0.5.0 ([ROOTURL]/detailed?branch=branchy#[..])
[CHECKING] bar v0.2.0 ([ROOT]/foo/bar)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
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
            edition = "2015"
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
        .with_stderr_data(str![[r#"
[CHECKING] dep v0.9.0 ([ROOT]/foo/dep)
[CHECKING] bar v0.2.0 ([ROOT]/foo/bar)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
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
            edition = "2015"
            authors = ["rustaceans"]
            description = { workspace = false }
        "#,
        )
        .file("bar/src/main.rs", "fn main() {}")
        .build();

    p.cargo("check")
        .cwd("bar")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] `workspace` cannot be false
 --> Cargo.toml:8:41
  |
8 |             description = { workspace = false }
  |                                         ^^^^^

"#]])
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
            edition = "2015"

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
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  dependency (dep) specified without providing a local path, Git repository, version, or workspace dependency to use

"#]])
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
            edition = "2015"
            authors = ["rustaceans"]
        "#,
        )
        .file("bar/src/main.rs", "fn main() {}")
        .build();

    p.cargo("check")
        .cwd("bar")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] unclosed array, expected `]`
 --> ../Cargo.toml:4:13
  |
4 |             
  |             ^

"#]])
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
            edition = "2015"
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
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/bar/Cargo.toml`

Caused by:
  error inheriting `description` from workspace root manifest's `workspace.package.description`

Caused by:
  root of a workspace inferred but wasn't a root: [ROOT]/foo/Cargo.toml

"#]])
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
            edition = "2015"
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
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/bar/Cargo.toml`

Caused by:
  error inheriting `foo` from workspace root manifest's `workspace.dependencies.foo`

Caused by:
  `workspace.dependencies` was not defined

"#]])
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
            edition = "2015"
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

    p.cargo("check").with_stderr_data(str![[r#"
[WARNING] [ROOT]/foo/Cargo.toml: `default-features` is ignored for dep, since `default-features` was true for `workspace.dependencies.dep`, this could become a hard error in the future
[UPDATING] `dummy-registry` index
[LOCKING] 2 packages to latest compatible versions
[DOWNLOADING] crates ...
[DOWNLOADED] fancy_dep v0.2.4 (registry `dummy-registry`)
[DOWNLOADED] dep v0.1.0 (registry `dummy-registry`)
[CHECKING] fancy_dep v0.2.4
[CHECKING] dep v0.1.0
[CHECKING] bar v0.2.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]).run();
}

#[cargo_test]
fn warn_inherit_def_feat_true_member_def_feat_false_2024_edition() {
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
            edition = "2024"
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
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  error inheriting `dep` from workspace root manifest's `workspace.dependencies.dep`

Caused by:
  `default-features = false` cannot override workspace's `default-features`

"#]])
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
            edition = "2015"
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

    p.cargo("check").with_stderr_data(str![[r#"
[WARNING] [ROOT]/foo/Cargo.toml: `default-features` is ignored for dep, since `default-features` was not specified for `workspace.dependencies.dep`, this could become a hard error in the future
[UPDATING] `dummy-registry` index
[LOCKING] 2 packages to latest compatible versions
[DOWNLOADING] crates ...
[DOWNLOADED] fancy_dep v0.2.4 (registry `dummy-registry`)
[DOWNLOADED] dep v0.1.0 (registry `dummy-registry`)
[CHECKING] fancy_dep v0.2.4
[CHECKING] dep v0.1.0
[CHECKING] bar v0.2.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]).run();
}

#[cargo_test]
fn warn_inherit_simple_member_def_feat_false_2024_edition() {
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
            edition = "2024"
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
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  error inheriting `dep` from workspace root manifest's `workspace.dependencies.dep`

Caused by:
  `default-features = false` cannot override workspace's `default-features`

"#]])
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
            edition = "2015"
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
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 2 packages to latest compatible versions
[DOWNLOADING] crates ...
[DOWNLOADED] fancy_dep v0.2.4 (registry `dummy-registry`)
[DOWNLOADED] dep v0.1.0 (registry `dummy-registry`)
[CHECKING] fancy_dep v0.2.4
[CHECKING] dep v0.1.0
[CHECKING] bar v0.2.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
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
            edition = "2015"

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
        .with_stderr_data(str![[r#"
[ERROR] failed to parse manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  dependency (bar) specified without providing a local path, Git repository, version, or workspace dependency to use

"#]])
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
            edition = "2015"
            authors = []

            [dependencies]
            dep = { workspace = true, wxz = "wxz" }
        "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("check")
        .with_stderr_data(str![[r#"
[WARNING] [ROOT]/foo/Cargo.toml: unused manifest key: workspace.dependencies.dep.wxz
[WARNING] [ROOT]/foo/Cargo.toml: unused manifest key: dependencies.dep.wxz
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] dep v0.1.0 (registry `dummy-registry`)
[CHECKING] dep v0.1.0
[CHECKING] bar v0.2.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn warn_unused_workspace_package_field() {
    Package::new("dep", "0.1.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = []
            [workspace.package]
            name = "unused"

            [package]
            name = "foo"
            edition = "2015"
        "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("check")
        .with_stderr_data(str![[r#"
[WARNING] [ROOT]/foo/Cargo.toml: unused manifest key: workspace.package.name
[CHECKING] foo v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn warn_inherit_unused_manifest_key_package() {
    Package::new("dep", "0.1.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
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
        .with_stderr_data(str![[r#"
[WARNING] [ROOT]/foo/Cargo.toml: unused manifest key: package.authors.xyz
[WARNING] [ROOT]/foo/Cargo.toml: unused manifest key: package.categories.xyz
[WARNING] [ROOT]/foo/Cargo.toml: unused manifest key: package.description.xyz
[WARNING] [ROOT]/foo/Cargo.toml: unused manifest key: package.documentation.xyz
[WARNING] [ROOT]/foo/Cargo.toml: unused manifest key: package.edition.xyz
[WARNING] [ROOT]/foo/Cargo.toml: unused manifest key: package.exclude.xyz
[WARNING] [ROOT]/foo/Cargo.toml: unused manifest key: package.homepage.xyz
[WARNING] [ROOT]/foo/Cargo.toml: unused manifest key: package.include.xyz
[WARNING] [ROOT]/foo/Cargo.toml: unused manifest key: package.keywords.xyz
[WARNING] [ROOT]/foo/Cargo.toml: unused manifest key: package.license.xyz
[WARNING] [ROOT]/foo/Cargo.toml: unused manifest key: package.publish.xyz
[WARNING] [ROOT]/foo/Cargo.toml: unused manifest key: package.repository.xyz
[WARNING] [ROOT]/foo/Cargo.toml: unused manifest key: package.rust-version.xyz
[WARNING] [ROOT]/foo/Cargo.toml: unused manifest key: package.version.xyz
[CHECKING] bar v1.2.3 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}
