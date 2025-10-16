//! Tests for the `cargo publish` command.

use std::fs;
use std::sync::{Arc, Mutex};

use crate::prelude::*;
use cargo_test_support::git::{self, repo};
use cargo_test_support::registry::{self, Package, RegistryBuilder, Response};
use cargo_test_support::{Project, paths};
use cargo_test_support::{basic_manifest, project, publish, str};

const CLEAN_FOO_JSON: &str = r#"
    {
        "authors": [],
        "badges": {},
        "categories": [],
        "deps": [],
        "description": "foo",
        "documentation": "foo",
        "features": {},
        "homepage": "foo",
        "keywords": [],
        "license": "MIT",
        "license_file": null,
        "links": null,
        "name": "foo",
        "readme": null,
        "readme_file": null,
        "repository": "foo",
        "rust_version": null,
        "vers": "0.0.1"
    }
"#;

fn validate_upload_foo() {
    publish::validate_upload(
        r#"
        {
          "authors": [],
          "badges": {},
          "categories": [],
          "deps": [],
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
          "rust_version": null,
          "vers": "0.0.1"
          }
        "#,
        "foo-0.0.1.crate",
        &["Cargo.lock", "Cargo.toml", "Cargo.toml.orig", "src/main.rs"],
    );
}

fn validate_upload_li() {
    publish::validate_upload(
        r#"
        {
          "authors": [],
          "badges": {},
          "categories": [],
          "deps": [],
          "description": "li",
          "documentation": null,
          "features": {},
          "homepage": null,
          "keywords": [],
          "license": "MIT",
          "license_file": null,
          "links": null,
          "name": "li",
          "readme": null,
          "readme_file": null,
          "repository": null,
          "rust_version": "1.69",
          "vers": "0.0.1"
          }
        "#,
        "li-0.0.1.crate",
        &["Cargo.lock", "Cargo.toml", "Cargo.toml.orig", "src/main.rs"],
    );
}

#[cargo_test]
fn simple() {
    let registry = RegistryBuilder::new().http_api().http_index().build();

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
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("publish --no-verify")
        .replace_crates_io(registry.index_url())
        .with_stderr_data(str![[r#"
[UPDATING] crates.io index
[WARNING] manifest has no documentation, homepage or repository
  |
  = [NOTE] see https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info
[PACKAGING] foo v0.0.1 ([ROOT]/foo)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[UPLOADING] foo v0.0.1 ([ROOT]/foo)
[UPLOADED] foo v0.0.1 to registry `crates-io`
[NOTE] waiting for foo v0.0.1 to be available at registry `crates-io`
[HELP] you may press ctrl-c to skip waiting; the crate should be available shortly
[PUBLISHED] foo v0.0.1 at registry `crates-io`

"#]])
        .run();

    validate_upload_foo();
}

#[cargo_test]
fn duplicate_version() {
    let registry_dupl = RegistryBuilder::new().http_api().http_index().build();
    Package::new("foo", "0.0.1").publish();

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
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("publish --dry-run")
        .replace_crates_io(registry_dupl.index_url())
        .with_stderr_data(str![[r#"
[UPDATING] crates.io index
[WARNING] crate foo@0.0.1 already exists on crates.io index
[WARNING] manifest has no documentation, homepage or repository
  |
  = [NOTE] see https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info
[PACKAGING] foo v0.0.1 ([ROOT]/foo)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] foo v0.0.1 ([ROOT]/foo)
[COMPILING] foo v0.0.1 ([ROOT]/foo/target/package/foo-0.0.1)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[UPLOADING] foo v0.0.1 ([ROOT]/foo)
[WARNING] aborting upload due to dry run

"#]])
        .run();

    p.cargo("publish")
        .replace_crates_io(registry_dupl.index_url())
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] crates.io index
[ERROR] crate foo@0.0.1 already exists on crates.io index

"#]])
        .run();
}

// Check that the `token` key works at the root instead of under a
// `[registry]` table.
#[cargo_test]
fn simple_publish_with_http() {
    let _reg = registry::RegistryBuilder::new()
        .http_api()
        .token(registry::Token::Plaintext("sekrit".to_string()))
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
                license = "MIT"
                description = "foo"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("publish --no-verify --token sekrit --registry dummy-registry")
        .with_stderr_data(str![[r#"
[WARNING] `cargo publish --token` is deprecated in favor of using `cargo login` and environment variables
[UPDATING] `dummy-registry` index
[WARNING] manifest has no documentation, homepage or repository
  |
  = [NOTE] see https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info
[PACKAGING] foo v0.0.1 ([ROOT]/foo)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[UPLOADING] foo v0.0.1 ([ROOT]/foo)
[UPLOADED] foo v0.0.1 to registry `dummy-registry`
[NOTE] waiting for foo v0.0.1 to be available at registry `dummy-registry`
[HELP] you may press ctrl-c to skip waiting; the crate should be available shortly
[PUBLISHED] foo v0.0.1 at registry `dummy-registry`

"#]])
        .run();
}

#[cargo_test]
fn simple_publish_with_asymmetric() {
    let _reg = registry::RegistryBuilder::new()
        .http_api()
        .http_index()
        .alternative_named("dummy-registry")
        .token(registry::Token::rfc_key())
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
                license = "MIT"
                description = "foo"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("publish --no-verify -Zasymmetric-token --registry dummy-registry")
        .masquerade_as_nightly_cargo(&["asymmetric-token"])
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[WARNING] manifest has no documentation, homepage or repository
  |
  = [NOTE] see https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info
[PACKAGING] foo v0.0.1 ([ROOT]/foo)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[UPLOADING] foo v0.0.1 ([ROOT]/foo)
[UPLOADED] foo v0.0.1 to registry `dummy-registry`
[NOTE] waiting for foo v0.0.1 to be available at registry `dummy-registry`
[HELP] you may press ctrl-c to skip waiting; the crate should be available shortly
[PUBLISHED] foo v0.0.1 at registry `dummy-registry`

"#]])
        .run();
}

#[cargo_test]
fn old_token_location() {
    // `publish` generally requires a remote registry
    let registry = registry::RegistryBuilder::new().http_api().build();

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
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    let credentials = paths::home().join(".cargo/credentials.toml");
    fs::remove_file(&credentials).unwrap();

    // Verify can't publish without a token.
    p.cargo("publish --no-verify")
        .replace_crates_io(registry.index_url())
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] crates.io index
[ERROR] no token found, please run `cargo login`
or use environment variable CARGO_REGISTRY_TOKEN

"#]])
        .run();

    fs::write(&credentials, format!(r#"token = "{}""#, registry.token())).unwrap();

    p.cargo("publish --no-verify")
        .replace_crates_io(registry.index_url())
        .with_stderr_data(str![[r#"
[UPDATING] crates.io index
[WARNING] manifest has no documentation, homepage or repository
  |
  = [NOTE] see https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info
[PACKAGING] foo v0.0.1 ([ROOT]/foo)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[UPLOADING] foo v0.0.1 ([ROOT]/foo)
[UPLOADED] foo v0.0.1 to registry `crates-io`
[NOTE] waiting for foo v0.0.1 to be available at registry `crates-io`
[HELP] you may press ctrl-c to skip waiting; the crate should be available shortly
[PUBLISHED] foo v0.0.1 at registry `crates-io`

"#]])
        .run();

    // Skip `validate_upload_foo` as we just cared we got far enough for verify the token behavior.
    // Other tests will verify the endpoint gets the right payload.
}

#[cargo_test]
fn simple_with_index() {
    // `publish` generally requires a remote registry
    let registry = registry::RegistryBuilder::new().http_api().build();

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
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("publish --no-verify")
        .arg("--token")
        .arg(registry.token())
        .arg("--index")
        .arg(registry.index_url().as_str())
        .with_stderr_data(str![[r#"
[WARNING] `cargo publish --token` is deprecated in favor of using `cargo login` and environment variables
[UPDATING] `[ROOT]/registry` index
[WARNING] manifest has no documentation, homepage or repository
  |
  = [NOTE] see https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info
[PACKAGING] foo v0.0.1 ([ROOT]/foo)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[UPLOADING] foo v0.0.1 ([ROOT]/foo)
[UPLOADED] foo v0.0.1 to registry `[ROOT]/registry`
[NOTE] waiting for foo v0.0.1 to be available at registry `[ROOT]/registry`
[HELP] you may press ctrl-c to skip waiting; the crate should be available shortly
[PUBLISHED] foo v0.0.1 at registry `[ROOT]/registry`

"#]])
        .run();

    // Skip `validate_upload_foo` as we just cared we got far enough for verify the VCS behavior.
    // Other tests will verify the endpoint gets the right payload.
}

#[cargo_test]
fn git_deps() {
    // Use local registry for faster test times since no publish will occur
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
                license = "MIT"
                description = "foo"

                [dependencies.foo]
                git = "git://path/to/nowhere"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("publish --no-verify")
        .replace_crates_io(registry.index_url())
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] crates.io index
[ERROR] failed to verify manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  all dependencies must have a version requirement specified when publishing.
  dependency `foo` does not specify a version
  Note: The published dependency will use the version from crates.io,
  the `git` specification will be removed from the dependency declaration.

"#]])
        .run();
}

#[cargo_test]
fn path_dependency_no_version() {
    // Use local registry for faster test times since no publish will occur
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
                license = "MIT"
                description = "foo"

                [dependencies.bar]
                path = "bar"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file("bar/src/lib.rs", "")
        .build();

    p.cargo("publish")
        .replace_crates_io(registry.index_url())
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] crates.io index
[ERROR] failed to verify manifest at `[ROOT]/foo/Cargo.toml`

Caused by:
  all dependencies must have a version requirement specified when publishing.
  dependency `bar` does not specify a version
  Note: The published dependency will use the version from crates.io,
  the `path` specification will be removed from the dependency declaration.

"#]])
        .run();
}

#[cargo_test]
fn unpublishable_crate() {
    // Use local registry for faster test times since no publish will occur
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
                license = "MIT"
                description = "foo"
                publish = false
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("publish --index")
        .arg(registry.index_url().as_str())
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] `foo` cannot be published.
`package.publish` must be set to `true` or a non-empty list in Cargo.toml to publish.

"#]])
        .run();
}

#[cargo_test]
fn dont_publish_dirty() {
    // Use local registry for faster test times since no publish will occur
    let registry = registry::init();

    let p = project().file("bar", "").build();

    let _ = git::repo(&paths::root().join("foo"))
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
                documentation = "foo"
                homepage = "foo"
                repository = "foo"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("publish")
        .replace_crates_io(registry.index_url())
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] crates.io index
[ERROR] 1 files in the working directory contain changes that were not yet committed into git:

bar

to proceed despite this and include the uncommitted changes, pass the `--allow-dirty` flag

"#]])
        .run();
}

#[cargo_test]
fn publish_clean() {
    // `publish` generally requires a remote registry
    let registry = registry::RegistryBuilder::new().http_api().build();

    let p = project().build();

    let _ = repo(&paths::root().join("foo"))
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
                documentation = "foo"
                homepage = "foo"
                repository = "foo"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("publish")
        .replace_crates_io(registry.index_url())
        .with_stderr_data(str![[r#"
[UPDATING] crates.io index
[PACKAGING] foo v0.0.1 ([ROOT]/foo)
[PACKAGED] 5 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] foo v0.0.1 ([ROOT]/foo)
[COMPILING] foo v0.0.1 ([ROOT]/foo/target/package/foo-0.0.1)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[UPLOADING] foo v0.0.1 ([ROOT]/foo)
[UPLOADED] foo v0.0.1 to registry `crates-io`
[NOTE] waiting for foo v0.0.1 to be available at registry `crates-io`
[HELP] you may press ctrl-c to skip waiting; the crate should be available shortly
[PUBLISHED] foo v0.0.1 at registry `crates-io`

"#]])
        .run();

    // Skip `validate_upload_foo_clean` as we just cared we got far enough for verify the VCS behavior.
    // Other tests will verify the endpoint gets the right payload.
}

#[cargo_test]
fn publish_in_sub_repo() {
    // `publish` generally requires a remote registry
    let registry = registry::RegistryBuilder::new().http_api().build();

    let p = project().no_manifest().file("baz", "").build();

    let _ = repo(&paths::root().join("foo"))
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []
                license = "MIT"
                description = "foo"
                documentation = "foo"
                homepage = "foo"
                repository = "foo"
            "#,
        )
        .file("bar/src/main.rs", "fn main() {}")
        .build();

    p.cargo("publish")
        .replace_crates_io(registry.index_url())
        .cwd("bar")
        .with_stderr_data(str![[r#"
[UPDATING] crates.io index
[PACKAGING] foo v0.0.1 ([ROOT]/foo/bar)
[PACKAGED] 5 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] foo v0.0.1 ([ROOT]/foo/bar)
[COMPILING] foo v0.0.1 ([ROOT]/foo/bar/target/package/foo-0.0.1)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[UPLOADING] foo v0.0.1 ([ROOT]/foo/bar)
[UPLOADED] foo v0.0.1 to registry `crates-io`
[NOTE] waiting for foo v0.0.1 to be available at registry `crates-io`
[HELP] you may press ctrl-c to skip waiting; the crate should be available shortly
[PUBLISHED] foo v0.0.1 at registry `crates-io`

"#]])
        .run();

    // Skip `validate_upload_foo_clean` as we just cared we got far enough for verify the VCS behavior.
    // Other tests will verify the endpoint gets the right payload.
}

#[cargo_test]
fn publish_when_ignored() {
    // `publish` generally requires a remote registry
    let registry = registry::RegistryBuilder::new().http_api().build();

    let p = project().file("baz", "").build();

    let _ = repo(&paths::root().join("foo"))
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
                documentation = "foo"
                homepage = "foo"
                repository = "foo"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(".gitignore", "baz")
        .build();

    p.cargo("publish")
        .replace_crates_io(registry.index_url())
        .with_stderr_data(str![[r#"
[UPDATING] crates.io index
[PACKAGING] foo v0.0.1 ([ROOT]/foo)
[PACKAGED] 6 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] foo v0.0.1 ([ROOT]/foo)
[COMPILING] foo v0.0.1 ([ROOT]/foo/target/package/foo-0.0.1)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[UPLOADING] foo v0.0.1 ([ROOT]/foo)
[UPLOADED] foo v0.0.1 to registry `crates-io`
[NOTE] waiting for foo v0.0.1 to be available at registry `crates-io`
[HELP] you may press ctrl-c to skip waiting; the crate should be available shortly
[PUBLISHED] foo v0.0.1 at registry `crates-io`

"#]])
        .run();

    // Skip `validate_upload` as we just cared we got far enough for verify the VCS behavior.
    // Other tests will verify the endpoint gets the right payload.
}

#[cargo_test]
fn ignore_when_crate_ignored() {
    // `publish` generally requires a remote registry
    let registry = registry::RegistryBuilder::new().http_api().build();

    let p = project().no_manifest().file("bar/baz", "").build();

    let _ = repo(&paths::root().join("foo"))
        .file(".gitignore", "bar")
        .nocommit_file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []
                license = "MIT"
                description = "foo"
                documentation = "foo"
                homepage = "foo"
                repository = "foo"
            "#,
        )
        .nocommit_file("bar/src/main.rs", "fn main() {}");
    p.cargo("publish")
        .replace_crates_io(registry.index_url())
        .cwd("bar")
        .with_stderr_data(str![[r#"
[UPDATING] crates.io index
[PACKAGING] foo v0.0.1 ([ROOT]/foo/bar)
[PACKAGED] 5 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] foo v0.0.1 ([ROOT]/foo/bar)
[COMPILING] foo v0.0.1 ([ROOT]/foo/bar/target/package/foo-0.0.1)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[UPLOADING] foo v0.0.1 ([ROOT]/foo/bar)
[UPLOADED] foo v0.0.1 to registry `crates-io`
[NOTE] waiting for foo v0.0.1 to be available at registry `crates-io`
[HELP] you may press ctrl-c to skip waiting; the crate should be available shortly
[PUBLISHED] foo v0.0.1 at registry `crates-io`

"#]])
        .run();

    // Skip `validate_upload` as we just cared we got far enough for verify the VCS behavior.
    // Other tests will verify the endpoint gets the right payload.
}

#[cargo_test]
fn new_crate_rejected() {
    // Use local registry for faster test times since no publish will occur
    let registry = registry::init();

    let p = project().file("baz", "").build();

    let _ = repo(&paths::root().join("foo"))
        .nocommit_file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []
                license = "MIT"
                description = "foo"
                documentation = "foo"
                homepage = "foo"
                repository = "foo"
            "#,
        )
        .nocommit_file("src/main.rs", "fn main() {}");
    p.cargo("publish")
        .replace_crates_io(registry.index_url())
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] crates.io index
[ERROR] 3 files in the working directory contain changes that were not yet committed into git:
...
"#]])
        .run();
}

#[cargo_test]
fn dry_run() {
    // Use local registry for faster test times since no publish will occur
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
                license = "MIT"
                description = "foo"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("publish --dry-run --index")
        .arg(registry.index_url().as_str())
        .with_stderr_data(str![[r#"
[UPDATING] `[ROOT]/registry` index
[WARNING] manifest has no documentation, homepage or repository
  |
  = [NOTE] see https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info
[PACKAGING] foo v0.0.1 ([ROOT]/foo)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] foo v0.0.1 ([ROOT]/foo)
[COMPILING] foo v0.0.1 ([ROOT]/foo/target/package/foo-0.0.1)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[UPLOADING] foo v0.0.1 ([ROOT]/foo)
[WARNING] aborting upload due to dry run

"#]])
        .run();

    // Ensure the API request wasn't actually made
    assert!(registry::api_path().join("api/v1/crates").exists());
    assert!(!registry::api_path().join("api/v1/crates/new").exists());
}

#[cargo_test]
fn registry_not_in_publish_list() {
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
                publish = [
                    "test"
                ]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("publish")
        .arg("--registry")
        .arg("alternative")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] `foo` cannot be published.
The registry `alternative` is not listed in the `package.publish` value in Cargo.toml.

"#]])
        .run();
}

#[cargo_test]
fn publish_empty_list() {
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
                publish = []
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("publish --registry alternative")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] `foo` cannot be published.
`package.publish` must be set to `true` or a non-empty list in Cargo.toml to publish.

"#]])
        .run();
}

#[cargo_test]
fn publish_allowed_registry() {
    let _registry = RegistryBuilder::new()
        .http_api()
        .http_index()
        .alternative()
        .build();

    let p = project().build();

    let _ = repo(&paths::root().join("foo"))
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
                documentation = "foo"
                homepage = "foo"
                repository = "foo"
                publish = ["alternative"]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("publish --registry alternative")
        .with_stderr_data(str![[r#"
[UPDATING] `alternative` index
[PACKAGING] foo v0.0.1 ([ROOT]/foo)
[PACKAGED] 5 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
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

    publish::validate_alt_upload(
        CLEAN_FOO_JSON,
        "foo-0.0.1.crate",
        &[
            "Cargo.lock",
            "Cargo.toml",
            "Cargo.toml.orig",
            "src/main.rs",
            ".cargo_vcs_info.json",
        ],
    );
}

#[cargo_test]
fn publish_implicitly_to_only_allowed_registry() {
    let _registry = RegistryBuilder::new()
        .http_api()
        .http_index()
        .alternative()
        .build();

    let p = project().build();

    let _ = repo(&paths::root().join("foo"))
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
                documentation = "foo"
                homepage = "foo"
                repository = "foo"
                publish = ["alternative"]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("publish")
        .with_stderr_data(str![[r#"
[NOTE] found `alternative` as only allowed registry. Publishing to it automatically.
[UPDATING] `alternative` index
[PACKAGING] foo v0.0.1 ([ROOT]/foo)
[PACKAGED] 5 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
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

    publish::validate_alt_upload(
        CLEAN_FOO_JSON,
        "foo-0.0.1.crate",
        &[
            "Cargo.lock",
            "Cargo.toml",
            "Cargo.toml.orig",
            "src/main.rs",
            ".cargo_vcs_info.json",
        ],
    );
}

#[cargo_test]
fn publish_failed_with_index_and_only_allowed_registry() {
    let registry = RegistryBuilder::new()
        .http_api()
        .http_index()
        .alternative()
        .build();

    let p = project().build();

    let _ = repo(&paths::root().join("foo"))
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
                documentation = "foo"
                homepage = "foo"
                repository = "foo"
                publish = ["alternative"]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("publish")
        .arg("--index")
        .arg(registry.index_url().as_str())
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] command-line argument --index requires --token to be specified

"#]])
        .run();
}

#[cargo_test]
fn publish_fail_with_no_registry_specified() {
    let p = project().build();

    let _ = repo(&paths::root().join("foo"))
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
                documentation = "foo"
                homepage = "foo"
                repository = "foo"
                publish = ["alternative", "test"]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("publish")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] --registry is required to disambiguate between "alternative" or "test" registries

"#]])
        .run();
}

#[cargo_test]
fn block_publish_no_registry() {
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
                publish = []
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("publish --registry alternative")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] `foo` cannot be published.
`package.publish` must be set to `true` or a non-empty list in Cargo.toml to publish.

"#]])
        .run();
}

// Explicitly setting `crates-io` in the publish list.
#[cargo_test]
fn publish_with_crates_io_explicit() {
    // `publish` generally requires a remote registry
    let registry = registry::RegistryBuilder::new().http_api().build();

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
                publish = ["crates-io"]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("publish --registry alternative")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] `foo` cannot be published.
The registry `alternative` is not listed in the `package.publish` value in Cargo.toml.

"#]])
        .run();

    p.cargo("publish")
        .replace_crates_io(registry.index_url())
        .with_stderr_data(str![[r#"
[UPDATING] crates.io index
[WARNING] manifest has no documentation, homepage or repository
  |
  = [NOTE] see https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info
[PACKAGING] foo v0.0.1 ([ROOT]/foo)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] foo v0.0.1 ([ROOT]/foo)
[COMPILING] foo v0.0.1 ([ROOT]/foo/target/package/foo-0.0.1)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[UPLOADING] foo v0.0.1 ([ROOT]/foo)
[UPLOADED] foo v0.0.1 to registry `crates-io`
[NOTE] waiting for foo v0.0.1 to be available at registry `crates-io`
[HELP] you may press ctrl-c to skip waiting; the crate should be available shortly
[PUBLISHED] foo v0.0.1 at registry `crates-io`

"#]])
        .run();
}

#[cargo_test]
fn publish_with_select_features() {
    // `publish` generally requires a remote registry
    let registry = registry::RegistryBuilder::new().http_api().build();

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

                [features]
                required = []
                optional = []
            "#,
        )
        .file(
            "src/main.rs",
            "#[cfg(not(feature = \"required\"))]
             compile_error!(\"This crate requires `required` feature!\");
             fn main() {}",
        )
        .build();

    p.cargo("publish --features required")
        .replace_crates_io(registry.index_url())
        .with_stderr_data(str![[r#"
[UPDATING] crates.io index
[WARNING] manifest has no documentation, homepage or repository
  |
  = [NOTE] see https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info
[PACKAGING] foo v0.0.1 ([ROOT]/foo)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] foo v0.0.1 ([ROOT]/foo)
[COMPILING] foo v0.0.1 ([ROOT]/foo/target/package/foo-0.0.1)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[UPLOADING] foo v0.0.1 ([ROOT]/foo)
[UPLOADED] foo v0.0.1 to registry `crates-io`
[NOTE] waiting for foo v0.0.1 to be available at registry `crates-io`
[HELP] you may press ctrl-c to skip waiting; the crate should be available shortly
[PUBLISHED] foo v0.0.1 at registry `crates-io`

"#]])
        .run();
}

#[cargo_test]
fn publish_with_all_features() {
    // `publish` generally requires a remote registry
    let registry = registry::RegistryBuilder::new().http_api().build();

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

                [features]
                required = []
                optional = []
            "#,
        )
        .file(
            "src/main.rs",
            "#[cfg(not(feature = \"required\"))]
             compile_error!(\"This crate requires `required` feature!\");
             fn main() {}",
        )
        .build();

    p.cargo("publish --all-features")
        .replace_crates_io(registry.index_url())
        .with_stderr_data(str![[r#"
[UPDATING] crates.io index
[WARNING] manifest has no documentation, homepage or repository
  |
  = [NOTE] see https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info
[PACKAGING] foo v0.0.1 ([ROOT]/foo)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] foo v0.0.1 ([ROOT]/foo)
[COMPILING] foo v0.0.1 ([ROOT]/foo/target/package/foo-0.0.1)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[UPLOADING] foo v0.0.1 ([ROOT]/foo)
[UPLOADED] foo v0.0.1 to registry `crates-io`
[NOTE] waiting for foo v0.0.1 to be available at registry `crates-io`
[HELP] you may press ctrl-c to skip waiting; the crate should be available shortly
[PUBLISHED] foo v0.0.1 at registry `crates-io`

"#]])
        .run();
}

#[cargo_test]
fn publish_with_no_default_features() {
    // Use local registry for faster test times since no publish will occur
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
                license = "MIT"
                description = "foo"

                [features]
                default = ["required"]
                required = []
            "#,
        )
        .file(
            "src/main.rs",
            "#[cfg(not(feature = \"required\"))]
             compile_error!(\"This crate requires `required` feature!\");
             fn main() {}",
        )
        .build();

    p.cargo("publish --no-default-features")
        .replace_crates_io(registry.index_url())
        .with_status(101)
        .with_stderr_data(str![[r#"
...
[ERROR] This crate requires `required` feature!
...
"#]])
        .run();
}

#[cargo_test]
fn publish_with_patch() {
    let registry = RegistryBuilder::new().http_api().http_index().build();
    Package::new("bar", "1.0.0").publish();

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
                [dependencies]
                bar = "1.0"
                [patch.crates-io]
                bar = { path = "bar" }
            "#,
        )
        .file(
            "src/main.rs",
            "extern crate bar;
             fn main() {
                 bar::newfunc();
             }",
        )
        .file("bar/Cargo.toml", &basic_manifest("bar", "1.0.0"))
        .file("bar/src/lib.rs", "pub fn newfunc() {}")
        .build();

    // Check that it works with the patched crate.
    p.cargo("build").run();

    // Check that verify fails with patched crate which has new functionality.
    p.cargo("publish")
        .replace_crates_io(registry.index_url())
        .with_status(101)
        .with_stderr_data(str![[r#"
...
error[E0425]: cannot find function `newfunc` in crate `bar`
...
"#]])
        .run();

    // Remove the usage of new functionality and try again.
    p.change_file("src/main.rs", "extern crate bar; pub fn main() {}");

    p.cargo("publish")
        .replace_crates_io(registry.index_url())
        .with_stderr_data(str![[r#"
[UPDATING] crates.io index
[WARNING] manifest has no documentation, homepage or repository
  |
  = [NOTE] see https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info
[PACKAGING] foo v0.0.1 ([ROOT]/foo)
[UPDATING] crates.io index
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] foo v0.0.1 ([ROOT]/foo)
[COMPILING] foo v0.0.1 ([ROOT]/foo/target/package/foo-0.0.1)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[UPLOADING] foo v0.0.1 ([ROOT]/foo)
[UPLOADED] foo v0.0.1 to registry `crates-io`
[NOTE] waiting for foo v0.0.1 to be available at registry `crates-io`
[HELP] you may press ctrl-c to skip waiting; the crate should be available shortly
[PUBLISHED] foo v0.0.1 at registry `crates-io`

"#]])
        .run();

    publish::validate_upload(
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
              "name": "bar",
              "optional": false,
              "target": null,
              "version_req": "^1.0"
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
          "rust_version": null,
          "vers": "0.0.1"
          }
        "#,
        "foo-0.0.1.crate",
        &["Cargo.lock", "Cargo.toml", "Cargo.toml.orig", "src/main.rs"],
    );
}

#[cargo_test]
fn publish_checks_for_token_before_verify() {
    let registry = registry::RegistryBuilder::new()
        .no_configure_token()
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
                license = "MIT"
                description = "foo"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    // Assert upload token error before the package is verified
    p.cargo("publish")
        .replace_crates_io(registry.index_url())
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] crates.io index
[ERROR] no token found, please run `cargo login`
or use environment variable CARGO_REGISTRY_TOKEN

"#]])
        .with_stderr_does_not_contain("[VERIFYING] foo v0.0.1 ([CWD])")
        .run();

    // Assert package verified successfully on dry run
    p.cargo("publish --dry-run")
        .replace_crates_io(registry.index_url())
        .with_stderr_data(str![[r#"
[UPDATING] crates.io index
[WARNING] manifest has no documentation, homepage or repository
  |
  = [NOTE] see https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info
[PACKAGING] foo v0.0.1 ([ROOT]/foo)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] foo v0.0.1 ([ROOT]/foo)
[COMPILING] foo v0.0.1 ([ROOT]/foo/target/package/foo-0.0.1)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[UPLOADING] foo v0.0.1 ([ROOT]/foo)
[WARNING] aborting upload due to dry run

"#]])
        .run();
}

#[cargo_test]
fn publish_with_bad_source() {
    let p = project()
        .file(
            ".cargo/config.toml",
            r#"
            [source.crates-io]
            replace-with = 'local-registry'

            [source.local-registry]
            local-registry = 'registry'
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("publish")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] crates-io is replaced with non-remote-registry source registry `[ROOT]/foo/registry`;
include `--registry crates-io` to use crates.io

"#]])
        .run();

    p.change_file(
        ".cargo/config.toml",
        r#"
        [source.crates-io]
        replace-with = "vendored-sources"

        [source.vendored-sources]
        directory = "vendor"
        "#,
    );

    p.cargo("publish")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] crates-io is replaced with non-remote-registry source dir [ROOT]/foo/vendor;
include `--registry crates-io` to use crates.io

"#]])
        .run();
}

// A dependency with both `git` and `version`.
#[cargo_test]
fn publish_git_with_version() {
    let registry = RegistryBuilder::new().http_api().http_index().build();

    Package::new("dep1", "1.0.1")
        .file("src/lib.rs", "pub fn f() -> i32 {1}")
        .publish();

    let git_project = git::new("dep1", |project| {
        project
            .file("Cargo.toml", &basic_manifest("dep1", "1.0.0"))
            .file("src/lib.rs", "pub fn f() -> i32 {2}")
    });

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                [package]
                name = "foo"
                version = "0.1.0"
                authors = []
                edition = "2018"
                license = "MIT"
                description = "foo"

                [dependencies]
                dep1 = {{version = "1.0", git="{}"}}
                "#,
                git_project.url()
            ),
        )
        .file(
            "src/main.rs",
            r#"
            pub fn main() {
                println!("{}", dep1::f());
            }
            "#,
        )
        .build();

    p.cargo("run")
        .with_stdout_data(str![[r#"
2

"#]])
        .run();

    p.cargo("publish --no-verify")
        .replace_crates_io(registry.index_url())
        .with_stderr_data(str![[r#"
[UPDATING] crates.io index
[WARNING] manifest has no documentation, homepage or repository
  |
  = [NOTE] see https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info
[PACKAGING] foo v0.1.0 ([ROOT]/foo)
[UPDATING] crates.io index
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[UPLOADING] foo v0.1.0 ([ROOT]/foo)
[UPLOADED] foo v0.1.0 to registry `crates-io`
[NOTE] waiting for foo v0.1.0 to be available at registry `crates-io`
[HELP] you may press ctrl-c to skip waiting; the crate should be available shortly
[PUBLISHED] foo v0.1.0 at registry `crates-io`

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
              "name": "dep1",
              "optional": false,
              "target": null,
              "version_req": "^1.0"
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
          "rust_version": null,
          "vers": "0.1.0"
          }
        "#,
        "foo-0.1.0.crate",
        &["Cargo.lock", "Cargo.toml", "Cargo.toml.orig", "src/main.rs"],
        [
            (
                "Cargo.toml",
                // Check that only `version` is included in Cargo.toml.
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
name = "foo"
version = "0.1.0"
authors = []
build = false
autolib = false
autobins = false
autoexamples = false
autotests = false
autobenches = false
description = "foo"
readme = false
license = "MIT"

[[bin]]
name = "foo"
path = "src/main.rs"

[dependencies.dep1]
version = "1.0"

"##]],
            ),
            (
                "Cargo.lock",
                // The important check here is that it is 1.0.1 in the registry.
                str![[r##"
# This file is automatically @generated by Cargo.
# It is not intended for manual editing.
version = 4

[[package]]
name = "dep1"
version = "1.0.1"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "[..]"

[[package]]
name = "foo"
version = "0.1.0"
dependencies = [
 "dep1",
]

"##]],
            ),
        ],
    );
}

#[cargo_test]
fn publish_dev_dep_stripping() {
    let registry = RegistryBuilder::new().http_api().http_index().build();
    Package::new("normal-only", "1.0.0")
        .feature("cat", &[])
        .publish();
    Package::new("optional-dep-feature", "1.0.0")
        .feature("cat", &[])
        .publish();
    Package::new("optional-namespaced", "1.0.0")
        .feature("cat", &[])
        .publish();
    Package::new("optional-renamed-dep-feature", "1.0.0")
        .feature("cat", &[])
        .publish();
    Package::new("optional-renamed-namespaced", "1.0.0")
        .feature("cat", &[])
        .publish();
    Package::new("build-only", "1.0.0")
        .feature("cat", &[])
        .publish();
    Package::new("normal-and-dev", "1.0.0")
        .feature("cat", &[])
        .publish();
    Package::new("target-normal-only", "1.0.0")
        .feature("cat", &[])
        .publish();
    Package::new("target-build-only", "1.0.0")
        .feature("cat", &[])
        .publish();
    Package::new("target-normal-and-dev", "1.0.0")
        .feature("cat", &[])
        .publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            edition = "2015"
            authors = []
            license = "MIT"
            description = "foo"
            documentation = "foo"
            homepage = "foo"
            repository = "foo"


            [features]
            foo_feature = [
                "normal-only/cat",
                "build-only/cat",
                "dev-only/cat",
                "renamed-dev-only01/cat",
                "normal-and-dev/cat",
                "target-normal-only/cat",
                "target-build-only/cat",
                "target-dev-only/cat",
                "target-normal-and-dev/cat",
                "optional-dep-feature/cat",
                "dep:optional-namespaced",
                "optional-renamed-dep-feature10/cat",
                "dep:optional-renamed-namespaced10",
            ]

            [dependencies]
            normal-only = { version = "1.0", features = ["cat"] }
            normal-and-dev = { version = "1.0", features = ["cat"] }
            optional-dep-feature = { version = "1.0", features = ["cat"], optional = true }
            optional-namespaced = { version = "1.0", features = ["cat"], optional = true }
            optional-renamed-dep-feature10 = { version = "1.0", features = ["cat"], optional = true, package = "optional-renamed-dep-feature" }
            optional-renamed-namespaced10 = { version = "1.0", features = ["cat"], optional = true, package = "optional-renamed-namespaced" }

            [build-dependencies]
            build-only = { version = "1.0", features = ["cat"] }

            [dev-dependencies]
            dev-only = { path = "../dev-only", features = ["cat"] }
            renamed-dev-only01 = { path = "../renamed-dev-only", features = ["cat"], package = "renamed-dev-only" }
            normal-and-dev = { version = "1.0", features = ["cat"] }

            [target.'cfg(unix)'.dependencies]
            target-normal-only = { version = "1.0", features = ["cat"] }
            target-normal-and-dev = { version = "1.0", features = ["cat"] }

            [target.'cfg(unix)'.build-dependencies]
            target-build-only = { version = "1.0", features = ["cat"] }

            [target.'cfg(unix)'.dev-dependencies]
            target-dev-only = { path = "../dev-only", features = ["cat"] }
            target-normal-and-dev = { version = "1.0", features = ["cat"] }
            "#,
        )
        .file("src/main.rs", "")
        .file(
            "dev-only/Cargo.toml",
            r#"
            [package]
            name = "dev-only"
            version = "0.1.0"
            edition = "2015"
            authors = []

            [features]
            cat = []
            "#,
        )
        .file(
            "dev-only/src/lib.rs",
            r#"
                #[cfg(feature = "cat")]
                pub fn cat() {}
            "#,
        )
        .file(
            "renamed-dev-only/Cargo.toml",
            r#"
            [package]
            name = "renamed-dev-only"
            version = "0.1.0"
            edition = "2015"
            authors = []

            [features]
            cat = []
            "#,
        )
        .file(
            "renamed-dev-only/src/lib.rs",
            r#"
                #[cfg(feature = "cat")]
                pub fn cat() {}
            "#,
        )
        .build();

    p.cargo("publish --no-verify")
        .env("RUSTFLAGS", "--cfg unix")
        .replace_crates_io(registry.index_url())
        .with_stderr_data(str![[r#"
[UPDATING] crates.io index
[PACKAGING] foo v0.1.0 ([ROOT]/foo)
[UPDATING] crates.io index
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[UPLOADING] foo v0.1.0 ([ROOT]/foo)
[UPLOADED] foo v0.1.0 to registry `crates-io`
[NOTE] waiting for foo v0.1.0 to be available at registry `crates-io`
[HELP] you may press ctrl-c to skip waiting; the crate should be available shortly
[PUBLISHED] foo v0.1.0 at registry `crates-io`

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
              "features": [
                "cat"
              ],
              "kind": "normal",
              "name": "normal-and-dev",
              "optional": false,
              "target": null,
              "version_req": "^1.0"
            },
            {
              "default_features": true,
              "features": [
                "cat"
              ],
              "kind": "normal",
              "name": "normal-only",
              "optional": false,
              "target": null,
              "version_req": "^1.0"
            },
            {
              "default_features": true,
              "features": [
                "cat"
              ],
              "kind": "normal",
              "name": "optional-dep-feature",
              "optional": true,
              "target": null,
              "version_req": "^1.0"
            },
            {
              "default_features": true,
              "features": [
                "cat"
              ],
              "kind": "normal",
              "name": "optional-namespaced",
              "optional": true,
              "target": null,
              "version_req": "^1.0"
            },
            {
              "default_features": true,
              "explicit_name_in_toml": "optional-renamed-dep-feature10",
              "features": [
                "cat"
              ],
              "kind": "normal",
              "name": "optional-renamed-dep-feature",
              "optional": true,
              "target": null,
              "version_req": "^1.0"
            },
            {
              "default_features": true,
              "explicit_name_in_toml": "optional-renamed-namespaced10",
              "features": [
                "cat"
              ],
              "kind": "normal",
              "name": "optional-renamed-namespaced",
              "optional": true,
              "target": null,
              "version_req": "^1.0"
            },
            {
              "default_features": true,
              "features": [
                "cat"
              ],
              "kind": "dev",
              "name": "normal-and-dev",
              "optional": false,
              "target": null,
              "version_req": "^1.0"
            },
            {
              "default_features": true,
              "features": [
                "cat"
              ],
              "kind": "build",
              "name": "build-only",
              "optional": false,
              "target": null,
              "version_req": "^1.0"
            },
            {
              "default_features": true,
              "features": [
                "cat"
              ],
              "kind": "normal",
              "name": "target-normal-and-dev",
              "optional": false,
              "target": "cfg(unix)",
              "version_req": "^1.0"
            },
            {
              "default_features": true,
              "features": [
                "cat"
              ],
              "kind": "normal",
              "name": "target-normal-only",
              "optional": false,
              "target": "cfg(unix)",
              "version_req": "^1.0"
            },
            {
              "default_features": true,
              "features": [
                "cat"
              ],
              "kind": "build",
              "name": "target-build-only",
              "optional": false,
              "target": "cfg(unix)",
              "version_req": "^1.0"
            },
            {
              "default_features": true,
              "features": [
                "cat"
              ],
              "kind": "dev",
              "name": "target-normal-and-dev",
              "optional": false,
              "target": "cfg(unix)",
              "version_req": "^1.0"
            }
          ],
          "description": "foo",
          "documentation": "foo",
          "features": {
            "foo_feature": [
              "normal-only/cat",
              "build-only/cat",
              "normal-and-dev/cat",
              "target-normal-only/cat",
              "target-build-only/cat",
              "target-normal-and-dev/cat",
              "optional-dep-feature/cat",
              "dep:optional-namespaced",
              "optional-renamed-dep-feature10/cat",
              "dep:optional-renamed-namespaced10"
            ]
          },
          "homepage": "foo",
          "keywords": [],
          "license": "MIT",
          "license_file": null,
          "links": null,
          "name": "foo",
          "readme": null,
          "readme_file": null,
          "repository": "foo",
          "rust_version": null,
          "vers": "0.1.0"
        }
        "#,
        "foo-0.1.0.crate",
        &["Cargo.lock", "Cargo.toml", "Cargo.toml.orig", "src/main.rs"],
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
name = "foo"
version = "0.1.0"
authors = []
build = false
autolib = false
autobins = false
autoexamples = false
autotests = false
autobenches = false
description = "foo"
homepage = "foo"
documentation = "foo"
readme = false
license = "MIT"
repository = "foo"

[features]
foo_feature = [
    "normal-only/cat",
    "build-only/cat",
    "normal-and-dev/cat",
    "target-normal-only/cat",
    "target-build-only/cat",
    "target-normal-and-dev/cat",
    "optional-dep-feature/cat",
    "dep:optional-namespaced",
    "optional-renamed-dep-feature10/cat",
    "dep:optional-renamed-namespaced10",
]

[[bin]]
name = "foo"
path = "src/main.rs"

[dependencies.normal-and-dev]
version = "1.0"
features = ["cat"]

[dependencies.normal-only]
version = "1.0"
features = ["cat"]

[dependencies.optional-dep-feature]
version = "1.0"
features = ["cat"]
optional = true

[dependencies.optional-namespaced]
version = "1.0"
features = ["cat"]
optional = true

[dependencies.optional-renamed-dep-feature10]
version = "1.0"
features = ["cat"]
optional = true
package = "optional-renamed-dep-feature"

[dependencies.optional-renamed-namespaced10]
version = "1.0"
features = ["cat"]
optional = true
package = "optional-renamed-namespaced"

[dev-dependencies.normal-and-dev]
version = "1.0"
features = ["cat"]

[build-dependencies.build-only]
version = "1.0"
features = ["cat"]

[target."cfg(unix)".dependencies.target-normal-and-dev]
version = "1.0"
features = ["cat"]

[target."cfg(unix)".dependencies.target-normal-only]
version = "1.0"
features = ["cat"]

[target."cfg(unix)".build-dependencies.target-build-only]
version = "1.0"
features = ["cat"]

[target."cfg(unix)".dev-dependencies.target-normal-and-dev]
version = "1.0"
features = ["cat"]

"##]],
        )],
    );
}

#[cargo_test]
fn credentials_ambiguous_filename() {
    // `publish` generally requires a remote registry
    let registry = registry::RegistryBuilder::new().http_api().build();

    // Make token in `credentials.toml` incorrect to ensure it is not read.
    let credentials_toml = paths::home().join(".cargo/credentials.toml");
    fs::write(credentials_toml, r#"token = "wrong-token""#).unwrap();

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
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("publish --no-verify")
        .replace_crates_io(registry.index_url())
        .with_status(101)
        .with_stderr_data(str![[r#"
...
  Unauthorized message from server.

"#]])
        .run();

    // Favor `credentials` if exists.
    let credentials = paths::home().join(".cargo/credentials");
    fs::write(credentials, r#"token = "sekrit""#).unwrap();

    p.cargo("publish --no-verify")
        .replace_crates_io(registry.index_url())
        .with_stderr_data(str![[r#"
[UPDATING] crates.io index
[WARNING] both `[ROOT]/home/.cargo/credentials` and `[ROOT]/home/.cargo/credentials.toml` exist. Using `[ROOT]/home/.cargo/credentials`
[WARNING] manifest has no documentation, homepage or repository
  |
  = [NOTE] see https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info
[PACKAGING] foo v0.0.1 ([ROOT]/foo)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[UPLOADING] foo v0.0.1 ([ROOT]/foo)
[UPLOADED] foo v0.0.1 to registry `crates-io`
[NOTE] waiting for foo v0.0.1 to be available at registry `crates-io`
[HELP] you may press ctrl-c to skip waiting; the crate should be available shortly
[PUBLISHED] foo v0.0.1 at registry `crates-io`

"#]])
        .run();
}

// --index will not load registry.token to avoid possibly leaking
// crates.io token to another server.
#[cargo_test]
fn index_requires_token() {
    // Use local registry for faster test times since no publish will occur
    let registry = registry::init();

    let credentials = paths::home().join(".cargo/credentials.toml");
    fs::remove_file(&credentials).unwrap();

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
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("publish --no-verify --index")
        .arg(registry.index_url().as_str())
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] command-line argument --index requires --token to be specified

"#]])
        .run();
}

// publish with source replacement without --registry
#[cargo_test]
fn cratesio_source_replacement() {
    registry::init();
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
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("publish --no-verify")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] crates-io is replaced with remote registry dummy-registry;
include `--registry dummy-registry` or `--registry crates-io`

"#]])
        .run();
}

// Registry returns an API error.
#[cargo_test]
fn api_error_json() {
    let _registry = registry::RegistryBuilder::new()
        .alternative()
        .http_api()
        .add_responder("/api/v1/crates/new", |_, _| Response {
            body: br#"{"errors": [{"detail": "you must be logged in"}]}"#.to_vec(),
            code: 403,
            headers: vec![],
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
                license = "MIT"
                description = "foo"
                documentation = "foo"
                homepage = "foo"
                repository = "foo"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("publish --no-verify --registry alternative")
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] `alternative` index
[PACKAGING] foo v0.0.1 ([ROOT]/foo)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[UPLOADING] foo v0.0.1 ([ROOT]/foo)
[ERROR] failed to publish foo v0.0.1 to registry at http://127.0.0.1:[..]/

Caused by:
  the remote server responded with an error (status 403 Forbidden): you must be logged in

"#]])
        .run();
}

// Registry returns an API error with a 200 status code.
#[cargo_test]
fn api_error_200() {
    let _registry = registry::RegistryBuilder::new()
        .alternative()
        .http_api()
        .add_responder("/api/v1/crates/new", |_, _| Response {
            body: br#"{"errors": [{"detail": "max upload size is 123"}]}"#.to_vec(),
            code: 200,
            headers: vec![],
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
                license = "MIT"
                description = "foo"
                documentation = "foo"
                homepage = "foo"
                repository = "foo"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("publish --no-verify --registry alternative")
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] `alternative` index
[PACKAGING] foo v0.0.1 ([ROOT]/foo)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[UPLOADING] foo v0.0.1 ([ROOT]/foo)
[ERROR] failed to publish foo v0.0.1 to registry at http://127.0.0.1:[..]/

Caused by:
  the remote server responded with an [ERROR] max upload size is 123

"#]])
        .run();
}

// Registry returns an error code without a JSON message.
#[cargo_test]
fn api_error_code() {
    let _registry = registry::RegistryBuilder::new()
        .alternative()
        .http_api()
        .add_responder("/api/v1/crates/new", |_, _| Response {
            body: br#"go away"#.to_vec(),
            code: 400,
            headers: vec![],
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
                license = "MIT"
                description = "foo"
                documentation = "foo"
                homepage = "foo"
                repository = "foo"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("publish --no-verify --registry alternative")
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] `alternative` index
[PACKAGING] foo v0.0.1 ([ROOT]/foo)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[UPLOADING] foo v0.0.1 ([ROOT]/foo)
[ERROR] failed to publish foo v0.0.1 to registry at http://127.0.0.1:[..]/

Caused by:
  failed to get a 200 OK response, got 400
  headers:
  	HTTP/1.1 400
  	Content-Length: 7
  	Connection: close
  	
  body:
  go away

"#]])
        .run();
}

// Registry has a network error.
#[cargo_test]
fn api_curl_error() {
    let _registry = registry::RegistryBuilder::new()
        .alternative()
        .http_api()
        .add_responder("/api/v1/crates/new", |_, _| {
            panic!("broke");
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
                license = "MIT"
                description = "foo"
                documentation = "foo"
                homepage = "foo"
                repository = "foo"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    // This doesn't check for the exact text of the error in the remote
    // possibility that cargo is linked with a weird version of libcurl, or
    // curl changes the text of the message. Currently the message 52
    // (CURLE_GOT_NOTHING) is:
    //    Server returned nothing (no headers, no data) (Empty reply from server)
    p.cargo("publish --no-verify --registry alternative")
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] `alternative` index
[PACKAGING] foo v0.0.1 ([ROOT]/foo)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[UPLOADING] foo v0.0.1 ([ROOT]/foo)
[ERROR] failed to publish foo v0.0.1 to registry at http://127.0.0.1:[..]/

Caused by:
  [52] Server returned nothing (no headers, no data) (Empty reply from server)

"#]])
        .run();
}

// Registry returns an invalid response.
#[cargo_test]
fn api_other_error() {
    let _registry = registry::RegistryBuilder::new()
        .alternative()
        .http_api()
        .add_responder("/api/v1/crates/new", |_, _| Response {
            body: b"\xff".to_vec(),
            code: 200,
            headers: vec![],
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
                license = "MIT"
                description = "foo"
                documentation = "foo"
                homepage = "foo"
                repository = "foo"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("publish --no-verify --registry alternative")
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] `alternative` index
[PACKAGING] foo v0.0.1 ([ROOT]/foo)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[UPLOADING] foo v0.0.1 ([ROOT]/foo)
[ERROR] failed to publish foo v0.0.1 to registry at http://127.0.0.1:[..]/

Caused by:
  invalid response body from server

Caused by:
  invalid utf-8 sequence of 1 bytes from index 0

"#]])
        .run();
}

#[cargo_test]
fn in_package_workspace() {
    let registry = RegistryBuilder::new().http_api().http_index().build();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2021"
                [workspace]
                members = ["li"]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "li/Cargo.toml",
            r#"
                [package]
                name = "li"
                version = "0.0.1"
                edition = "2015"
                rust-version = "1.69"
                description = "li"
                license = "MIT"
            "#,
        )
        .file("li/src/main.rs", "fn main() {}")
        .build();

    p.cargo("publish -p li --no-verify")
        .replace_crates_io(registry.index_url())
        .with_stderr_data(str![[r#"
[UPDATING] crates.io index
[WARNING] manifest has no documentation, homepage or repository
  |
  = [NOTE] see https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info
[PACKAGING] li v0.0.1 ([ROOT]/foo/li)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[UPLOADING] li v0.0.1 ([ROOT]/foo/li)
[UPLOADED] li v0.0.1 to registry `crates-io`
[NOTE] waiting for li v0.0.1 to be available at registry `crates-io`
[HELP] you may press ctrl-c to skip waiting; the crate should be available shortly
[PUBLISHED] li v0.0.1 at registry `crates-io`

"#]])
        .run();

    validate_upload_li();
}

#[cargo_test]
fn with_duplicate_spec_in_members() {
    let registry = RegistryBuilder::new().http_api().http_index().build();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"
                [workspace]
                resolver = "2"
                members = ["li","bar"]
                default-members = ["li","bar"]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "li/Cargo.toml",
            r#"
                [package]
                name = "li"
                version = "0.0.1"
                edition = "2015"
                description = "li"
                license = "MIT"
            "#,
        )
        .file("li/src/main.rs", "fn main() {}")
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.0.1"
                edition = "2015"
                description = "bar"
                license = "MIT"
            "#,
        )
        .file("bar/src/main.rs", "fn main() {}")
        .build();

    p.cargo("publish --no-verify")
        .replace_crates_io(registry.index_url())
        .with_stderr_data(str![[r#"
[UPDATING] crates.io index
[WARNING] manifest has no documentation, homepage or repository
  |
  = [NOTE] see https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info
[PACKAGING] bar v0.0.1 ([ROOT]/foo/bar)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[WARNING] manifest has no documentation, homepage or repository
  |
  = [NOTE] see https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info
[PACKAGING] li v0.0.1 ([ROOT]/foo/li)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[UPLOADING] bar v0.0.1 ([ROOT]/foo/bar)
[UPLOADED] bar v0.0.1 to registry `crates-io`
[UPLOADING] li v0.0.1 ([ROOT]/foo/li)
[UPLOADED] li v0.0.1 to registry `crates-io`
[NOTE] waiting for bar v0.0.1 or li v0.0.1 to be available at registry `crates-io`
[HELP] you may press ctrl-c to skip waiting; the crates should be available shortly
[PUBLISHED] bar v0.0.1 and li v0.0.1 at registry `crates-io`

"#]])
        .run();
}

#[cargo_test]
fn in_package_workspace_with_members_with_features_old() {
    let registry = RegistryBuilder::new().http_api().http_index().build();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"
                [workspace]
                members = ["li"]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "li/Cargo.toml",
            r#"
                [package]
                name = "li"
                version = "0.0.1"
                edition = "2015"
                rust-version = "1.69"
                description = "li"
                license = "MIT"
            "#,
        )
        .file("li/src/main.rs", "fn main() {}")
        .build();

    p.cargo("publish -p li --no-verify")
        .replace_crates_io(registry.index_url())
        .with_stderr_data(str![[r#"
[UPDATING] crates.io index
[WARNING] manifest has no documentation, homepage or repository
  |
  = [NOTE] see https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info
[PACKAGING] li v0.0.1 ([ROOT]/foo/li)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[UPLOADING] li v0.0.1 ([ROOT]/foo/li)
[UPLOADED] li v0.0.1 to registry `crates-io`
[NOTE] waiting for li v0.0.1 to be available at registry `crates-io`
[HELP] you may press ctrl-c to skip waiting; the crate should be available shortly
[PUBLISHED] li v0.0.1 at registry `crates-io`

"#]])
        .run();

    validate_upload_li();
}

#[cargo_test]
fn in_virtual_workspace() {
    let registry = RegistryBuilder::new().http_api().http_index().build();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["foo"]
            "#,
        )
        .file(
            "foo/Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []
                license = "MIT"
                description = "foo"
            "#,
        )
        .file("foo/src/main.rs", "fn main() {}")
        .build();

    p.cargo("publish --no-verify")
        .replace_crates_io(registry.index_url())
        .with_stderr_data(str![[r#"
[UPDATING] crates.io index
[WARNING] manifest has no documentation, homepage or repository
  |
  = [NOTE] see https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info
[PACKAGING] foo v0.0.1 ([ROOT]/foo/foo)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[UPLOADING] foo v0.0.1 ([ROOT]/foo/foo)
[UPLOADED] foo v0.0.1 to registry `crates-io`
[NOTE] waiting for foo v0.0.1 to be available at registry `crates-io`
[HELP] you may press ctrl-c to skip waiting; the crate should be available shortly
[PUBLISHED] foo v0.0.1 at registry `crates-io`

"#]])
        .run();
}

#[cargo_test]
fn in_virtual_workspace_with_p() {
    let registry = RegistryBuilder::new().http_api().http_index().build();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["foo","li"]
            "#,
        )
        .file(
            "foo/Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []
                license = "MIT"
                description = "foo"
            "#,
        )
        .file("foo/src/main.rs", "fn main() {}")
        .file(
            "li/Cargo.toml",
            r#"
                [package]
                name = "li"
                version = "0.0.1"
                edition = "2015"
                rust-version = "1.69"
                description = "li"
                license = "MIT"
            "#,
        )
        .file("li/src/main.rs", "fn main() {}")
        .build();

    p.cargo("publish -p li --no-verify")
        .replace_crates_io(registry.index_url())
        .with_stderr_data(str![[r#"
[UPDATING] crates.io index
[WARNING] manifest has no documentation, homepage or repository
  |
  = [NOTE] see https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info
[PACKAGING] li v0.0.1 ([ROOT]/foo/li)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[UPLOADING] li v0.0.1 ([ROOT]/foo/li)
[UPLOADED] li v0.0.1 to registry `crates-io`
[NOTE] waiting for li v0.0.1 to be available at registry `crates-io`
[HELP] you may press ctrl-c to skip waiting; the crate should be available shortly
[PUBLISHED] li v0.0.1 at registry `crates-io`

"#]])
        .run();
}

#[cargo_test]
fn in_package_workspace_not_found() {
    // Use local registry for faster test times since no publish will occur
    let registry = registry::init();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2021"
                [workspace]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "li/Cargo.toml",
            r#"
                [package]
                name = "li"
                version = "0.0.1"
                edition = "2021"
                authors = []
                license = "MIT"
                description = "li"
            "#,
        )
        .file("li/src/main.rs", "fn main() {}")
        .build();

    p.cargo("publish -p li --no-verify")
        .replace_crates_io(registry.index_url())
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] package ID specification `li` did not match any packages

[HELP] a package with a similar name exists: `foo`

"#]])
        .run();
}

#[cargo_test]
fn in_package_workspace_found_multiple() {
    let registry = RegistryBuilder::new().http_api().http_index().build();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2021"
                [workspace]
                members = ["li","lii"]
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "li/Cargo.toml",
            r#"
                [package]
                name = "li"
                version = "0.0.1"
                edition = "2021"
                authors = []
                license = "MIT"
                description = "li"
            "#,
        )
        .file("li/src/main.rs", "fn main() {}")
        .file(
            "lii/Cargo.toml",
            r#"
                [package]
                name = "lii"
                version = "0.0.1"
                edition = "2021"
                authors = []
                license = "MIT"
                description = "lii"
            "#,
        )
        .file("lii/src/main.rs", "fn main() {}")
        .build();

    p.cargo("publish -p li* --no-verify")
        .replace_crates_io(registry.index_url())
        .with_stderr_data(str![[r#"
[UPDATING] crates.io index
[WARNING] manifest has no documentation, homepage or repository
  |
  = [NOTE] see https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info
[PACKAGING] li v0.0.1 ([ROOT]/foo/li)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[WARNING] manifest has no documentation, homepage or repository
  |
  = [NOTE] see https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info
[PACKAGING] lii v0.0.1 ([ROOT]/foo/lii)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[UPLOADING] li v0.0.1 ([ROOT]/foo/li)
[UPLOADED] li v0.0.1 to registry `crates-io`
[UPLOADING] lii v0.0.1 ([ROOT]/foo/lii)
[UPLOADED] lii v0.0.1 to registry `crates-io`
[NOTE] waiting for li v0.0.1 or lii v0.0.1 to be available at registry `crates-io`
[HELP] you may press ctrl-c to skip waiting; the crates should be available shortly
[PUBLISHED] li v0.0.1 and lii v0.0.1 at registry `crates-io`

"#]])
        .run();
}

#[cargo_test]
fn publish_path_dependency_without_workspace() {
    let registry = RegistryBuilder::new().http_api().http_index().build();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2021"
                [dependencies.bar]
                path = "bar"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            "bar/Cargo.toml",
            r#"
                [package]
                name = "bar"
                version = "0.0.1"
                edition = "2021"
                authors = []
                license = "MIT"
                description = "bar"
            "#,
        )
        .file("bar/src/main.rs", "fn main() {}")
        .build();

    p.cargo("publish -p bar --no-verify")
        .replace_crates_io(registry.index_url())
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] package ID specification `bar` did not match any packages

[HELP] a package with a similar name exists: `foo`

"#]])
        .run();
}

#[cargo_test]
fn http_api_not_noop() {
    let registry = registry::RegistryBuilder::new().http_api().build();

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
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("publish")
        .replace_crates_io(registry.index_url())
        .with_stderr_data(str![[r#"
[UPDATING] crates.io index
[WARNING] manifest has no documentation, homepage or repository
  |
  = [NOTE] see https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info
[PACKAGING] foo v0.0.1 ([ROOT]/foo)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] foo v0.0.1 ([ROOT]/foo)
[COMPILING] foo v0.0.1 ([ROOT]/foo/target/package/foo-0.0.1)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[UPLOADING] foo v0.0.1 ([ROOT]/foo)
[UPLOADED] foo v0.0.1 to registry `crates-io`
[NOTE] waiting for foo v0.0.1 to be available at registry `crates-io`
[HELP] you may press ctrl-c to skip waiting; the crate should be available shortly
[PUBLISHED] foo v0.0.1 at registry `crates-io`

"#]])
        .run();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "bar"
                version = "0.0.1"
                edition = "2015"
                authors = []
                license = "MIT"
                description = "foo"

                [dependencies]
                foo = "0.0.1"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("build").run();
}

#[cargo_test]
fn wait_for_first_publish() {
    // Counter for number of tries before the package is "published"
    let arc: Arc<Mutex<u32>> = Arc::new(Mutex::new(0));
    let arc2 = arc.clone();

    // Registry returns an invalid response.
    let registry = registry::RegistryBuilder::new()
        .http_index()
        .http_api()
        .add_responder("/index/de/la/delay", move |req, server| {
            let mut lock = arc.lock().unwrap();
            *lock += 1;
            if *lock <= 1 {
                server.not_found(req)
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
                name = "delay"
                version = "0.0.1"
                edition = "2015"
                authors = []
                license = "MIT"
                description = "foo"

            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("publish --no-verify")
        .replace_crates_io(registry.index_url())
        .with_status(0)
        .with_stderr_data(str![[r#"
[UPDATING] crates.io index
[WARNING] manifest has no documentation, homepage or repository
  |
  = [NOTE] see https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info
[PACKAGING] delay v0.0.1 ([ROOT]/foo)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[UPLOADING] delay v0.0.1 ([ROOT]/foo)
[UPLOADED] delay v0.0.1 to registry `crates-io`
[NOTE] waiting for delay v0.0.1 to be available at registry `crates-io`
[HELP] you may press ctrl-c to skip waiting; the crate should be available shortly
[PUBLISHED] delay v0.0.1 at registry `crates-io`

"#]])
        .run();

    // Verify the responder has been pinged
    let lock = arc2.lock().unwrap();
    assert_eq!(*lock, 2);
    drop(lock);

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
                delay = "0.0.1"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("build").with_status(0).run();
}

/// A separate test is needed for package names with - or _ as they hit
/// the responder twice per cargo invocation. If that ever gets changed
/// this test will need to be changed accordingly.
#[cargo_test]
fn wait_for_first_publish_underscore() {
    // Counter for number of tries before the package is "published"
    let arc: Arc<Mutex<u32>> = Arc::new(Mutex::new(0));
    let arc2 = arc.clone();
    let misses = Arc::new(Mutex::new(Vec::new()));
    let misses2 = misses.clone();

    // Registry returns an invalid response.
    let registry = registry::RegistryBuilder::new()
        .http_index()
        .http_api()
        .add_responder("/index/de/la/delay_with_underscore", move |req, server| {
            let mut lock = arc.lock().unwrap();
            *lock += 1;
            if *lock <= 1 {
                server.not_found(req)
            } else {
                server.index(req)
            }
        })
        .not_found_handler(move |req, _| {
            misses.lock().unwrap().push(req.url.to_string());
            Response {
                body: b"not found".to_vec(),
                code: 404,
                headers: vec![],
            }
        })
        .build();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "delay_with_underscore"
                version = "0.0.1"
                edition = "2015"
                authors = []
                license = "MIT"
                description = "foo"

            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("publish --no-verify")
        .replace_crates_io(registry.index_url())
        .with_status(0)
        .with_stderr_data(str![[r#"
[UPDATING] crates.io index
[WARNING] manifest has no documentation, homepage or repository
  |
  = [NOTE] see https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info
[PACKAGING] delay_with_underscore v0.0.1 ([ROOT]/foo)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[UPLOADING] delay_with_underscore v0.0.1 ([ROOT]/foo)
[UPLOADED] delay_with_underscore v0.0.1 to registry `crates-io`
[NOTE] waiting for delay_with_underscore v0.0.1 to be available at registry `crates-io`
[HELP] you may press ctrl-c to skip waiting; the crate should be available shortly
[PUBLISHED] delay_with_underscore v0.0.1 at registry `crates-io`

"#]])
        .run();

    // Verify the repsponder has been pinged
    let lock = arc2.lock().unwrap();
    assert_eq!(*lock, 2);
    drop(lock);
    {
        let misses = misses2.lock().unwrap();
        assert!(
            misses.len() == 1,
            "should only have 1 not found URL; instead found {misses:?}"
        );
    }

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
                delay_with_underscore = "0.0.1"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("build").with_status(0).run();
}

#[cargo_test]
fn wait_for_subsequent_publish() {
    // Counter for number of tries before the package is "published"
    let arc: Arc<Mutex<u32>> = Arc::new(Mutex::new(0));
    let arc2 = arc.clone();
    let publish_req = Arc::new(Mutex::new(None));
    let publish_req2 = publish_req.clone();

    let registry = registry::RegistryBuilder::new()
        .http_index()
        .http_api()
        .add_responder("/api/v1/crates/new", move |req, server| {
            // Capture the publish request, but defer publishing
            *publish_req.lock().unwrap() = Some(req.clone());
            server.ok(req)
        })
        .add_responder("/index/de/la/delay", move |req, server| {
            let mut lock = arc.lock().unwrap();
            *lock += 1;
            if *lock == 3 {
                // Run the publish on the 3rd attempt
                let rep = server
                    .check_authorized_publish(&publish_req2.lock().unwrap().as_ref().unwrap());
                assert_eq!(rep.code, 200);
            }
            server.index(req)
        })
        .build();

    // Publish an earlier version
    Package::new("delay", "0.0.1")
        .file("src/lib.rs", "")
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "delay"
                version = "0.0.2"
                edition = "2015"
                authors = []
                license = "MIT"
                description = "foo"

            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("publish --no-verify")
        .replace_crates_io(registry.index_url())
        .with_status(0)
        .with_stderr_data(str![[r#"
[UPDATING] crates.io index
[WARNING] manifest has no documentation, homepage or repository
  |
  = [NOTE] see https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info
[PACKAGING] delay v0.0.2 ([ROOT]/foo)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[UPLOADING] delay v0.0.2 ([ROOT]/foo)
[UPLOADED] delay v0.0.2 to registry `crates-io`
[NOTE] waiting for delay v0.0.2 to be available at registry `crates-io`
[HELP] you may press ctrl-c to skip waiting; the crate should be available shortly
[PUBLISHED] delay v0.0.2 at registry `crates-io`

"#]])
        .run();

    // Verify the responder has been pinged
    let lock = arc2.lock().unwrap();
    assert_eq!(*lock, 3);
    drop(lock);

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
                delay = "0.0.2"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("check").with_status(0).run();
}

#[cargo_test]
fn skip_wait_for_publish() {
    // Intentionally using local registry so the crate never makes it to the index
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
                license = "MIT"
                description = "foo"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .file(
            ".cargo/config.toml",
            "
                [publish]
                timeout = 0
                ",
        )
        .build();

    p.cargo("publish --no-verify -Zpublish-timeout")
        .replace_crates_io(registry.index_url())
        .masquerade_as_nightly_cargo(&["publish-timeout"])
        .with_stderr_data(str![[r#"
[UPDATING] crates.io index
[WARNING] manifest has no documentation, homepage or repository
  |
  = [NOTE] see https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info
[PACKAGING] foo v0.0.1 ([ROOT]/foo)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[UPLOADING] foo v0.0.1 ([ROOT]/foo)
[UPLOADED] foo v0.0.1 to registry `crates-io`

"#]])
        .run();
}

#[cargo_test]
fn timeout_waiting_for_publish() {
    // Publish doesn't happen within the timeout window.
    let registry = registry::RegistryBuilder::new()
        .http_api()
        .delayed_index_update(20)
        .build();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "delay"
                version = "0.0.1"
                edition = "2015"
                authors = []
                license = "MIT"
                description = "foo"
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            ".cargo/config.toml",
            r#"
                [publish]
                timeout = 2
            "#,
        )
        .build();

    p.cargo("publish --no-verify -Zpublish-timeout")
        .replace_crates_io(registry.index_url())
        .masquerade_as_nightly_cargo(&["publish-timeout"])
        .with_status(0)
        .with_stderr_data(str![[r#"
[UPDATING] crates.io index
[WARNING] manifest has no documentation, homepage or repository
  |
  = [NOTE] see https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info
[PACKAGING] delay v0.0.1 ([ROOT]/foo)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[UPLOADING] delay v0.0.1 ([ROOT]/foo)
[UPLOADED] delay v0.0.1 to registry `crates-io`
[NOTE] waiting for delay v0.0.1 to be available at registry `crates-io`
[HELP] you may press ctrl-c to skip waiting; the crate should be available shortly
[WARNING] timed out waiting for delay v0.0.1 to be available in registry `crates-io`
  |
  = [NOTE] the registry may have a backlog that is delaying making the crate available. The crate should be available soon.

"#]])
        .run();
}

#[cargo_test]
fn timeout_waiting_for_dependency_publish() {
    // Publish doesn't happen within the timeout window.
    let registry = registry::RegistryBuilder::new()
        .http_api()
        .delayed_index_update(20)
        .build();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["main", "other", "dep"]
        "#,
        )
        .file(
            "main/Cargo.toml",
            r#"
                [package]
                name = "main"
                version = "0.0.1"
                edition = "2015"
                authors = []
                license = "MIT"
                description = "foo"

                [dependencies]
                dep = { version = "0.0.1", path = "../dep" }
            "#,
        )
        .file("main/src/main.rs", "fn main() {}")
        .file(
            "other/Cargo.toml",
            r#"
                [package]
                name = "other"
                version = "0.0.1"
                edition = "2015"
                authors = []
                license = "MIT"
                description = "foo"

                [dependencies]
                dep = { version = "0.0.1", path = "../dep" }
            "#,
        )
        .file("other/src/main.rs", "fn main() {}")
        .file(
            "dep/Cargo.toml",
            r#"
                [package]
                name = "dep"
                version = "0.0.1"
                edition = "2015"
                authors = []
                license = "MIT"
                description = "foo"
            "#,
        )
        .file("dep/src/lib.rs", "")
        .file(
            ".cargo/config.toml",
            r#"
                [publish]
                timeout = 2
            "#,
        )
        .build();

    p.cargo("publish --no-verify -Zpublish-timeout")
        .replace_crates_io(registry.index_url())
        .masquerade_as_nightly_cargo(&["publish-timeout"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] crates.io index
[WARNING] manifest has no documentation, homepage or repository
  |
  = [NOTE] see https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info
[PACKAGING] dep v0.0.1 ([ROOT]/foo/dep)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[WARNING] manifest has no documentation, homepage or repository
  |
  = [NOTE] see https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info
[PACKAGING] main v0.0.1 ([ROOT]/foo/main)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[WARNING] manifest has no documentation, homepage or repository
  |
  = [NOTE] see https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info
[PACKAGING] other v0.0.1 ([ROOT]/foo/other)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[UPLOADING] dep v0.0.1 ([ROOT]/foo/dep)
[UPLOADED] dep v0.0.1 to registry `crates-io`
[NOTE] waiting for dep v0.0.1 to be available at registry `crates-io`.
      2 remaining crates to be published
[WARNING] timed out waiting for dep v0.0.1 to be available in registry `crates-io`
  |
  = [NOTE] the registry may have a backlog that is delaying making the crate available. The crate should be available soon.
[ERROR] unable to publish main v0.0.1 and other v0.0.1 due to a timeout while waiting for published dependencies to be available.

"#]])
        .run();
}

#[cargo_test]
fn package_selection() {
    let registry = registry::RegistryBuilder::new().http_api().build();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["a", "b"]
            "#,
        )
        .file("a/Cargo.toml", &basic_manifest("a", "0.1.0"))
        .file("a/src/lib.rs", "#[test] fn a() {}")
        .file("b/Cargo.toml", &basic_manifest("b", "0.1.0"))
        .file("b/src/lib.rs", "#[test] fn b() {}")
        .build();

    p.cargo("publish --no-verify --dry-run --workspace")
        .replace_crates_io(registry.index_url())
        .with_stderr_data(str![[r#"
[UPDATING] crates.io index
[WARNING] manifest has no description, license, license-file, documentation, homepage or repository
  |
  = [NOTE] see https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info
[PACKAGING] a v0.1.0 ([ROOT]/foo/a)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[WARNING] manifest has no description, license, license-file, documentation, homepage or repository
  |
  = [NOTE] see https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info
[PACKAGING] b v0.1.0 ([ROOT]/foo/b)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[UPLOADING] a v0.1.0 ([ROOT]/foo/a)
[WARNING] aborting upload due to dry run
[UPLOADING] b v0.1.0 ([ROOT]/foo/b)
[WARNING] aborting upload due to dry run

"#]])
        .with_stdout_data(str![[r#""#]])
        .run();

    p.cargo("publish --no-verify --dry-run --package a --package b")
        .replace_crates_io(registry.index_url())
        .with_stderr_data(str![[r#"
[UPDATING] crates.io index
[WARNING] manifest has no description, license, license-file, documentation, homepage or repository
  |
  = [NOTE] see https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info
[PACKAGING] a v0.1.0 ([ROOT]/foo/a)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[WARNING] manifest has no description, license, license-file, documentation, homepage or repository
  |
  = [NOTE] see https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info
[PACKAGING] b v0.1.0 ([ROOT]/foo/b)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[UPLOADING] a v0.1.0 ([ROOT]/foo/a)
[WARNING] aborting upload due to dry run
[UPLOADING] b v0.1.0 ([ROOT]/foo/b)
[WARNING] aborting upload due to dry run

"#]])
        .with_stdout_data(str![[r#""#]])
        .run();

    p.cargo("publish --no-verify --dry-run --workspace --exclude b")
        .replace_crates_io(registry.index_url())
        .with_stderr_data(str![[r#"
[UPDATING] crates.io index
[WARNING] manifest has no description, license, license-file, documentation, homepage or repository
  |
  = [NOTE] see https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info
[PACKAGING] a v0.1.0 ([ROOT]/foo/a)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[UPLOADING] a v0.1.0 ([ROOT]/foo/a)
[WARNING] aborting upload due to dry run

"#]])
        .with_stdout_data(str![[r#""#]])
        .run();
}

#[cargo_test]
fn wait_for_git_publish() {
    // Slow publish to an index with a git index.
    let registry = registry::RegistryBuilder::new()
        .http_api()
        .delayed_index_update(5)
        .build();

    // Publish an earlier version
    Package::new("delay", "0.0.1")
        .file("src/lib.rs", "")
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "delay"
                version = "0.0.2"
                edition = "2015"
                authors = []
                license = "MIT"
                description = "foo"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("publish --no-verify")
        .replace_crates_io(registry.index_url())
        .with_status(0)
        .with_stderr_data(str![[r#"
[UPDATING] crates.io index
[WARNING] manifest has no documentation, homepage or repository
  |
  = [NOTE] see https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info
[PACKAGING] delay v0.0.2 ([ROOT]/foo)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[UPLOADING] delay v0.0.2 ([ROOT]/foo)
[UPLOADED] delay v0.0.2 to registry `crates-io`
[NOTE] waiting for delay v0.0.2 to be available at registry `crates-io`
[HELP] you may press ctrl-c to skip waiting; the crate should be available shortly
[PUBLISHED] delay v0.0.2 at registry `crates-io`

"#]])
        .run();

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
                delay = "0.0.2"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("check").with_status(0).run();
}

#[cargo_test]
fn invalid_token() {
    // Checks publish behavior with an invalid token.
    let registry = RegistryBuilder::new().http_api().http_index().build();

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
                documentation = "foo"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("publish --no-verify")
        .replace_crates_io(registry.index_url())
        .env("CARGO_REGISTRY_TOKEN", "\x16")
        .with_stderr_data(str![[r#"
[UPDATING] crates.io index
[PACKAGING] foo v0.0.1 ([ROOT]/foo)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[UPLOADING] foo v0.0.1 ([ROOT]/foo)
[ERROR] failed to publish foo v0.0.1 to registry at http://127.0.0.1:[..]/

Caused by:
  token contains invalid characters.
  Only printable ISO-8859-1 characters are allowed as it is sent in a HTTPS header.

"#]])
        .with_status(101)
        .run();
}

#[cargo_test]
fn versionless_package() {
    // Use local registry for faster test times since no publish will occur
    let registry = registry::init();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                description = "foo"
            "#,
        )
        .file("src/main.rs", r#"fn main() { println!("hello"); }"#)
        .build();

    p.cargo("publish")
        .replace_crates_io(registry.index_url())
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] `foo` cannot be published.
`package.publish` must be set to `true` or a non-empty list in Cargo.toml to publish.

"#]])
        .run();
}

// A workspace with three projects that depend on one another (level1 -> level2 -> level3).
// level1 is a binary package, to test lockfile generation.
fn workspace_with_local_deps_project() -> Project {
    project()
            .file(
                "Cargo.toml",
                r#"
            [workspace]
            members = ["level1", "level2", "level3"]

            [workspace.dependencies]
            level2 = { path = "level2", version = "0.0.1" }
        "#
            )
            .file(
                "level1/Cargo.toml",
                r#"
            [package]
            name = "level1"
            version = "0.0.1"
            edition = "2015"
            authors = []
            license = "MIT"
            description = "level1"
            repository = "bar"

            [dependencies]
            # Let one dependency also specify features, for the added test coverage when generating package files.
            level2 = { workspace = true, features = ["foo"] }
        "#,
            )
            .file("level1/src/main.rs", "fn main() {}")
            .file(
                "level2/Cargo.toml",
                r#"
            [package]
            name = "level2"
            version = "0.0.1"
            edition = "2015"
            authors = []
            license = "MIT"
            description = "level2"
            repository = "bar"

            [features]
            foo = []

            [dependencies]
            level3 = { path = "../level3", version = "0.0.1" }
        "#
            )
            .file("level2/src/lib.rs", "")
            .file(
                "level3/Cargo.toml",
                r#"
            [package]
            name = "level3"
            version = "0.0.1"
            edition = "2015"
            authors = []
            license = "MIT"
            description = "level3"
            repository = "bar"
        "#,
            )
            .file("level3/src/lib.rs", "")
            .build()
}

#[cargo_test]
fn workspace_with_local_deps() {
    let registry = RegistryBuilder::new().http_api().http_index().build();
    let p = workspace_with_local_deps_project();

    p.cargo("publish")
        .replace_crates_io(registry.index_url())
        .with_stderr_data(str![[r#"
[UPDATING] crates.io index
[PACKAGING] level3 v0.0.1 ([ROOT]/foo/level3)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[PACKAGING] level2 v0.0.1 ([ROOT]/foo/level2)
[UPDATING] crates.io index
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[PACKAGING] level1 v0.0.1 ([ROOT]/foo/level1)
[UPDATING] crates.io index
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] level3 v0.0.1 ([ROOT]/foo/level3)
[COMPILING] level3 v0.0.1 ([ROOT]/foo/target/package/level3-0.0.1)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[VERIFYING] level2 v0.0.1 ([ROOT]/foo/level2)
[UPDATING] crates.io index
[UNPACKING] level3 v0.0.1 (registry `[ROOT]/foo/target/package/tmp-registry`)
[COMPILING] level3 v0.0.1
[COMPILING] level2 v0.0.1 ([ROOT]/foo/target/package/level2-0.0.1)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[VERIFYING] level1 v0.0.1 ([ROOT]/foo/level1)
[UPDATING] crates.io index
[UNPACKING] level2 v0.0.1 (registry `[ROOT]/foo/target/package/tmp-registry`)
[COMPILING] level2 v0.0.1
[COMPILING] level1 v0.0.1 ([ROOT]/foo/target/package/level1-0.0.1)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[UPLOADING] level3 v0.0.1 ([ROOT]/foo/level3)
[UPLOADED] level3 v0.0.1 to registry `crates-io`
[NOTE] waiting for level3 v0.0.1 to be available at registry `crates-io`.
      2 remaining crates to be published
[PUBLISHED] level3 v0.0.1 at registry `crates-io`
[UPLOADING] level2 v0.0.1 ([ROOT]/foo/level2)
[UPLOADED] level2 v0.0.1 to registry `crates-io`
[NOTE] waiting for level2 v0.0.1 to be available at registry `crates-io`.
      1 remaining crate to be published
[PUBLISHED] level2 v0.0.1 at registry `crates-io`
[UPLOADING] level1 v0.0.1 ([ROOT]/foo/level1)
[UPLOADED] level1 v0.0.1 to registry `crates-io`
[NOTE] waiting for level1 v0.0.1 to be available at registry `crates-io`
[HELP] you may press ctrl-c to skip waiting; the crate should be available shortly
[PUBLISHED] level1 v0.0.1 at registry `crates-io`

"#]])
        .run();
}

#[cargo_test]
fn workspace_parallel() {
    let registry = RegistryBuilder::new().http_api().http_index().build();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["a", "b", "c"]
        "#,
        )
        .file(
            "a/Cargo.toml",
            r#"
            [package]
            name = "a"
            version = "0.0.1"
            edition = "2015"
            authors = []
            license = "MIT"
            description = "a"
            repository = "bar"
        "#,
        )
        .file("a/src/lib.rs", "")
        .file(
            "b/Cargo.toml",
            r#"
            [package]
            name = "b"
            version = "0.0.1"
            edition = "2015"
            authors = []
            license = "MIT"
            description = "b"
            repository = "bar"
        "#,
        )
        .file("b/src/lib.rs", "")
        .file(
            "c/Cargo.toml",
            r#"
            [package]
            name = "c"
            version = "0.0.1"
            edition = "2015"
            authors = []
            license = "MIT"
            description = "c"
            repository = "bar"

            [dependencies]
            a = { path = "../a", version = "0.0.1" }
            b = { path = "../b", version = "0.0.1" }
        "#,
        )
        .file("c/src/lib.rs", "")
        .build();

    p.cargo("publish")
        .replace_crates_io(registry.index_url())
        .with_stderr_data(
            str![[r#"
[UPDATING] crates.io index
[PACKAGING] a v0.0.1 ([ROOT]/foo/a)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[PACKAGING] b v0.0.1 ([ROOT]/foo/b)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[PACKAGING] c v0.0.1 ([ROOT]/foo/c)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] a v0.0.1 ([ROOT]/foo/a)
[COMPILING] a v0.0.1 ([ROOT]/foo/target/package/a-0.0.1)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[VERIFYING] b v0.0.1 ([ROOT]/foo/b)
[COMPILING] b v0.0.1 ([ROOT]/foo/target/package/b-0.0.1)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[VERIFYING] c v0.0.1 ([ROOT]/foo/c)
[UPDATING] crates.io index
[UNPACKING] a v0.0.1 (registry `[ROOT]/foo/target/package/tmp-registry`)
[UNPACKING] b v0.0.1 (registry `[ROOT]/foo/target/package/tmp-registry`)
[COMPILING] a v0.0.1
[COMPILING] b v0.0.1
[COMPILING] c v0.0.1 ([ROOT]/foo/target/package/c-0.0.1)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[UPLOADED] b v0.0.1 to registry `crates-io`
[UPLOADED] a v0.0.1 to registry `crates-io`
[NOTE] waiting for a v0.0.1 or b v0.0.1 to be available at registry `crates-io`.
      1 remaining crate to be published
[PUBLISHED] a v0.0.1 and b v0.0.1 at registry `crates-io`
[UPLOADING] c v0.0.1 ([ROOT]/foo/c)
[UPLOADED] c v0.0.1 to registry `crates-io`
[NOTE] waiting for c v0.0.1 to be available at registry `crates-io`
[PUBLISHED] c v0.0.1 at registry `crates-io`
[UPLOADING] a v0.0.1 ([ROOT]/foo/a)
[UPLOADING] b v0.0.1 ([ROOT]/foo/b)
[UPDATING] crates.io index
[HELP] you may press ctrl-c to skip waiting; the crate should be available shortly

"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test]
fn workspace_missing_dependency() {
    let registry = RegistryBuilder::new().http_api().http_index().build();
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
            version = "0.0.1"
            edition = "2015"
            authors = []
            license = "MIT"
            description = "a"
            repository = "bar"
        "#,
        )
        .file("a/src/lib.rs", "")
        .file(
            "b/Cargo.toml",
            r#"
            [package]
            name = "b"
            version = "0.0.1"
            edition = "2015"
            authors = []
            license = "MIT"
            description = "b"
            repository = "bar"

            [dependencies]
            a = { path = "../a", version = "0.0.1" }
        "#,
        )
        .file("b/src/lib.rs", "")
        .build();

    p.cargo("publish -p b")
        .replace_crates_io(registry.index_url())
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] crates.io index
[PACKAGING] b v0.0.1 ([ROOT]/foo/b)
[UPDATING] crates.io index
[ERROR] failed to prepare local package for uploading

Caused by:
  no matching package named `a` found
  location searched: crates.io index
  required by package `b v0.0.1 ([ROOT]/foo/b)`

"#]])
        .run();

    p.cargo("publish -p a")
        .replace_crates_io(registry.index_url())
        .with_stderr_data(str![[r#"
[UPDATING] crates.io index
[PACKAGING] a v0.0.1 ([ROOT]/foo/a)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] a v0.0.1 ([ROOT]/foo/a)
[COMPILING] a v0.0.1 ([ROOT]/foo/target/package/a-0.0.1)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[UPLOADING] a v0.0.1 ([ROOT]/foo/a)
[UPLOADED] a v0.0.1 to registry `crates-io`
[NOTE] waiting for a v0.0.1 to be available at registry `crates-io`
[HELP] you may press ctrl-c to skip waiting; the crate should be available shortly
[PUBLISHED] a v0.0.1 at registry `crates-io`

"#]])
        .run();

    // Publishing the whole workspace now will fail, as `a` is already published.
    p.cargo("publish")
        .replace_crates_io(registry.index_url())
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] crates.io index
[ERROR] crate a@0.0.1 already exists on crates.io index

"#]])
        .run();
}

#[cargo_test]
fn one_unpublishable_package() {
    let registry = RegistryBuilder::new().http_api().http_index().build();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["dep", "main"]
            "#,
        )
        .file(
            "main/Cargo.toml",
            r#"
            [package]
            name = "main"
            version = "0.0.1"
            edition = "2015"
            authors = []
            license = "MIT"
            description = "main"
            repository = "bar"
            publish = false

            [dependencies]
            dep = { path = "../dep", version = "0.1.0" }
        "#,
        )
        .file("main/src/main.rs", "fn main() {}")
        .file(
            "dep/Cargo.toml",
            r#"
            [package]
            name = "dep"
            version = "0.1.0"
            edition = "2015"
            authors = []
            license = "MIT"
            description = "dep"
            repository = "bar"
        "#,
        )
        .file("dep/src/lib.rs", "")
        .build();

    p.cargo("publish")
        .replace_crates_io(registry.index_url())
        .with_stderr_data(str![[r#"
[UPDATING] crates.io index
[PACKAGING] dep v0.1.0 ([ROOT]/foo/dep)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] dep v0.1.0 ([ROOT]/foo/dep)
[COMPILING] dep v0.1.0 ([ROOT]/foo/target/package/dep-0.1.0)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[UPLOADING] dep v0.1.0 ([ROOT]/foo/dep)
[UPLOADED] dep v0.1.0 to registry `crates-io`
[NOTE] waiting for dep v0.1.0 to be available at registry `crates-io`
[HELP] you may press ctrl-c to skip waiting; the crate should be available shortly
[PUBLISHED] dep v0.1.0 at registry `crates-io`

"#]])
        .run();
}

#[cargo_test]
fn virtual_ws_with_multiple_unpublishable_package() {
    let registry = RegistryBuilder::new().http_api().http_index().build();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["dep", "main", "publishable"]
            "#,
        )
        .file(
            "main/Cargo.toml",
            r#"
            [package]
            name = "main"
            version = "0.0.1"
            edition = "2015"
            authors = []
            license = "MIT"
            description = "main"
            repository = "bar"
            publish = false

            [dependencies]
            dep = { path = "../dep", version = "0.1.0" }
        "#,
        )
        .file("main/src/main.rs", "fn main() {}")
        .file(
            "dep/Cargo.toml",
            r#"
            [package]
            name = "dep"
            version = "0.1.0"
            edition = "2015"
            authors = []
            license = "MIT"
            description = "dep"
            repository = "bar"
            publish = false
        "#,
        )
        .file("dep/src/lib.rs", "")
        .file(
            "publishable/Cargo.toml",
            r#"
            [package]
            name = "publishable"
            version = "0.1.0"
            edition = "2015"
            license = "MIT"
            description = "foo"
            repository = "foo"
        "#,
        )
        .file("publishable/src/lib.rs", "")
        .build();

    p.cargo("publish")
        .replace_crates_io(registry.index_url())
        .with_stderr_data(str![[r#"
[UPDATING] crates.io index
[PACKAGING] publishable v0.1.0 ([ROOT]/foo/publishable)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] publishable v0.1.0 ([ROOT]/foo/publishable)
[COMPILING] publishable v0.1.0 ([ROOT]/foo/target/package/publishable-0.1.0)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[UPLOADING] publishable v0.1.0 ([ROOT]/foo/publishable)
[UPLOADED] publishable v0.1.0 to registry `crates-io`
[NOTE] waiting for publishable v0.1.0 to be available at registry `crates-io`
[HELP] you may press ctrl-c to skip waiting; the crate should be available shortly
[PUBLISHED] publishable v0.1.0 at registry `crates-io`

"#]])
        .run();
}

#[cargo_test]
fn workspace_flag_with_unpublishable_packages() {
    let registry = RegistryBuilder::new().http_api().http_index().build();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["publishable", "non-publishable"]

            [package]
            name = "cwd"
            version = "0.0.0"
            edition = "2015"
            license = "MIT"
            description = "foo"
            repository = "foo"
            publish = false
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "publishable/Cargo.toml",
            r#"
            [package]
            name = "publishable"
            version = "0.0.0"
            edition = "2015"
            license = "MIT"
            description = "foo"
            repository = "foo"
            publish = true
        "#,
        )
        .file("publishable/src/lib.rs", "")
        .file(
            "non-publishable/Cargo.toml",
            r#"
            [package]
            name = "non-publishable"
            version = "0.0.0"
            edition = "2015"
            license = "MIT"
            description = "foo"
            repository = "foo"
            publish = false
        "#,
        )
        .file("non-publishable/src/lib.rs", "")
        .build();

    p.cargo("publish --workspace")
        .replace_crates_io(registry.index_url())
        .with_stderr_data(str![[r#"
[UPDATING] crates.io index
[PACKAGING] publishable v0.0.0 ([ROOT]/foo/publishable)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] publishable v0.0.0 ([ROOT]/foo/publishable)
[COMPILING] publishable v0.0.0 ([ROOT]/foo/target/package/publishable-0.0.0)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[UPLOADING] publishable v0.0.0 ([ROOT]/foo/publishable)
[UPLOADED] publishable v0.0.0 to registry `crates-io`
[NOTE] waiting for publishable v0.0.0 to be available at registry `crates-io`
[HELP] you may press ctrl-c to skip waiting; the crate should be available shortly
[PUBLISHED] publishable v0.0.0 at registry `crates-io`

"#]])
        .run();
}

#[cargo_test]
fn unpublishable_package_as_versioned_dev_dep() {
    let registry = RegistryBuilder::new().http_api().http_index().build();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["dep", "main"]
            "#,
        )
        .file(
            "main/Cargo.toml",
            r#"
            [package]
            name = "main"
            version = "0.0.1"
            edition = "2015"
            license = "MIT"
            description = "main"
            repository = "bar"

            [dev-dependencies]
            dep = { path = "../dep", version = "0.1.0" }
        "#,
        )
        .file("main/src/main.rs", "fn main() {}")
        .file(
            "dep/Cargo.toml",
            r#"
            [package]
            name = "dep"
            version = "0.1.0"
            edition = "2015"
            license = "MIT"
            description = "dep"
            repository = "bar"
            publish = false
        "#,
        )
        .file("dep/src/lib.rs", "")
        .build();

    // It is expected to find the versioned dev dep not being published,
    // regardless with `--dry-run` or `--no-verify`.
    p.cargo("publish")
        .replace_crates_io(registry.index_url())
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] crates.io index
[PACKAGING] main v0.0.1 ([ROOT]/foo/main)
[UPDATING] crates.io index
[ERROR] failed to prepare local package for uploading

Caused by:
  no matching package named `dep` found
  location searched: crates.io index
  required by package `main v0.0.1 ([ROOT]/foo/main)`

"#]])
        .run();

    p.cargo("publish --dry-run")
        .replace_crates_io(registry.index_url())
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] crates.io index
[PACKAGING] main v0.0.1 ([ROOT]/foo/main)
[UPDATING] crates.io index
[ERROR] failed to prepare local package for uploading

Caused by:
  no matching package named `dep` found
  location searched: crates.io index
  required by package `main v0.0.1 ([ROOT]/foo/main)`

"#]])
        .run();

    p.cargo("publish --no-verify")
        .replace_crates_io(registry.index_url())
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] crates.io index
[PACKAGING] main v0.0.1 ([ROOT]/foo/main)
[UPDATING] crates.io index
[ERROR] failed to prepare local package for uploading

Caused by:
  no matching package named `dep` found
  location searched: crates.io index
  required by package `main v0.0.1 ([ROOT]/foo/main)`

"#]])
        .run();
}

#[cargo_test]
fn all_unpublishable_packages() {
    let registry = RegistryBuilder::new().http_api().http_index().build();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["non-publishable1", "non-publishable2"]
            "#,
        )
        .file(
            "non-publishable1/Cargo.toml",
            r#"
            [package]
            name = "non-publishable1"
            version = "0.0.0"
            edition = "2015"
            license = "MIT"
            description = "foo"
            repository = "foo"
            publish = false
        "#,
        )
        .file("non-publishable1/src/lib.rs", "")
        .file(
            "non-publishable2/Cargo.toml",
            r#"
            [package]
            name = "non-publishable2"
            version = "0.0.0"
            edition = "2015"
            license = "MIT"
            description = "foo"
            repository = "foo"
            publish = false
        "#,
        )
        .file("non-publishable2/src/lib.rs", "")
        .build();

    p.cargo("publish --workspace")
        .replace_crates_io(registry.index_url())
        .with_stderr_data(str![[r#"
[WARNING] nothing to publish, but found 2 unpublishable packages
  |
  = [HELP] to publish packages, set `package.publish` to `true` or a non-empty list

"#]])
        .run();
}

#[cargo_test]
fn checksum_changed() {
    let registry = RegistryBuilder::new().http_api().http_index().build();

    Package::new("dep", "1.0.0").publish();
    Package::new("transitive", "1.0.0")
        .dep("dep", "1.0.0")
        .publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [workspace]
                members = ["dep"]

                [package]
                name = "foo"
                version = "0.0.1"
                edition = "2015"
                authors = []
                license = "MIT"
                description = "foo"
                documentation = "foo"

                [dependencies]
                dep = { path = "./dep", version = "1.0.0" }
                transitive = "1.0.0"
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "dep/Cargo.toml",
            r#"
                [package]
                name = "dep"
                version = "1.0.0"
                edition = "2015"
            "#,
        )
        .file("dep/src/lib.rs", "")
        .build();

    p.cargo("check").run();

    p.cargo("publish --dry-run --workspace")
        .replace_crates_io(registry.index_url())
        .with_stderr_data(str![[r#"
[UPDATING] crates.io index
[WARNING] crate dep@1.0.0 already exists on crates.io index
[WARNING] manifest has no description, license, license-file, documentation, homepage or repository
  |
  = [NOTE] see https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info
[PACKAGING] dep v1.0.0 ([ROOT]/foo/dep)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[PACKAGING] foo v0.0.1 ([ROOT]/foo)
[UPDATING] crates.io index
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] dep v1.0.0 ([ROOT]/foo/dep)
[COMPILING] dep v1.0.0 ([ROOT]/foo/target/package/dep-1.0.0)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[VERIFYING] foo v0.0.1 ([ROOT]/foo)
[UNPACKING] dep v1.0.0 (registry `[ROOT]/foo/target/package/tmp-registry`)
[COMPILING] dep v1.0.0
[COMPILING] transitive v1.0.0
[COMPILING] foo v0.0.1 ([ROOT]/foo/target/package/foo-0.0.1)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s
[UPLOADING] dep v1.0.0 ([ROOT]/foo/dep)
[WARNING] aborting upload due to dry run
[UPLOADING] foo v0.0.1 ([ROOT]/foo)
[WARNING] aborting upload due to dry run

"#]])
        .run();
}

#[cargo_test]
fn workspace_publish_rate_limit_error() {
    let registry = registry::RegistryBuilder::new()
        .http_api()
        .http_index()
        .add_responder("/api/v1/crates/new", |_req, _| {
            // For simplicity, let's just return rate limit error for all requests
            // This simulates hitting rate limit during workspace publish
            Response {
                code: 429,
                headers: vec!["Retry-After: 3600".to_string()],
                body: format!(
                    "You have published too many new crates in a short period of time. Please try again after Fri, 18 Jul 2025 20:00:34 GMT or email help@crates.io to have your limit increased."
                ).into_bytes(),
            }
        })
        .build();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [workspace]
            members = ["package_a", "package_b", "package_c"]
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "package_a/Cargo.toml",
            r#"
            [package]
            name = "package_a"
            version = "0.1.0"
            edition = "2015"
            license = "MIT"
            description = "package a"
            repository = "https://github.com/test/package_a"
            "#,
        )
        .file("package_a/src/lib.rs", "")
        .file(
            "package_b/Cargo.toml",
            r#"
            [package]
            name = "package_b"
            version = "0.1.0"
            edition = "2015"
            license = "MIT"
            description = "package b"
            repository = "https://github.com/test/package_b"
            "#,
        )
        .file("package_b/src/lib.rs", "")
        .file(
            "package_c/Cargo.toml",
            r#"
            [package]
            name = "package_c"
            version = "0.1.0"
            edition = "2015"
            license = "MIT"
            description = "package c"
            repository = "https://github.com/test/package_c"

            [dependencies]
            package_a = { version = "0.1.0", path = "../package_a" }
            "#,
        )
        .file("package_c/src/lib.rs", "")
        .build();

    // This demonstrates the improved error message after the fix
    // The user now knows which package failed and what packages remain to be published
    p.cargo("publish --workspace --no-verify")
        .replace_crates_io(registry.index_url())
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] crates.io index
[PACKAGING] package_a v0.1.0 ([ROOT]/foo/package_a)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[PACKAGING] package_b v0.1.0 ([ROOT]/foo/package_b)
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[PACKAGING] package_c v0.1.0 ([ROOT]/foo/package_c)
[UPDATING] crates.io index
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[UPLOADING] package_a v0.1.0 ([ROOT]/foo/package_a)
[ERROR] failed to publish package_a v0.1.0 to registry at http://127.0.0.1:[..]/

[NOTE] the following crates have not been published yet:
  package_b v0.1.0 ([ROOT]/foo/package_b)
  package_c v0.1.0 ([ROOT]/foo/package_c)

Caused by:
  failed to get a 200 OK response, got 429
  headers:
  	HTTP/1.1 429
  	Content-Length: 172
  	Connection: close
  	Retry-After: 3600
  	
  body:
  You have published too many new crates in a short period of time. Please try again after Fri, 18 Jul 2025 20:00:34 GMT or email help@crates.io to have your limit increased.

"#]])
        .run();
}
