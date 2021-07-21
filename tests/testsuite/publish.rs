//! Tests for the `cargo publish` command.

use cargo_test_support::git::{self, repo};
use cargo_test_support::paths;
use cargo_test_support::registry::{self, registry_path, registry_url, Package};
use cargo_test_support::{basic_manifest, no_such_file_err_msg, project, publish};
use std::fs;

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
          "vers": "0.0.1"
          }
        "#,
        "foo-0.0.1.crate",
        &["Cargo.lock", "Cargo.toml", "Cargo.toml.orig", "src/main.rs"],
    );
}

fn validate_upload_foo_clean() {
    publish::validate_upload(
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
fn simple() {
    registry::init();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
                authors = []
                license = "MIT"
                description = "foo"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("publish --no-verify --token sekrit")
        .with_stderr(
            "\
[UPDATING] `dummy-registry` index
[WARNING] manifest has no documentation, [..]
See [..]
[PACKAGING] foo v0.0.1 ([CWD])
[UPLOADING] foo v0.0.1 ([CWD])
",
        )
        .run();

    validate_upload_foo();
}

#[cargo_test]
fn old_token_location() {
    // Check that the `token` key works at the root instead of under a
    // `[registry]` table.
    registry::init();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
                authors = []
                license = "MIT"
                description = "foo"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    let credentials = paths::home().join(".cargo/credentials");
    fs::remove_file(&credentials).unwrap();

    // Verify can't publish without a token.
    p.cargo("publish --no-verify")
        .with_status(101)
        .with_stderr_contains(
            "[ERROR] no upload token found, \
            please run `cargo login` or pass `--token`",
        )
        .run();

    fs::write(&credentials, r#"token = "api-token""#).unwrap();

    p.cargo("publish --no-verify")
        .with_stderr(
            "\
[UPDATING] `dummy-registry` index
[WARNING] using `registry.token` config value with source replacement is deprecated
This may become a hard error in the future[..]
Use the --token command-line flag to remove this warning.
[WARNING] manifest has no documentation, [..]
See [..]
[PACKAGING] foo v0.0.1 ([CWD])
[UPLOADING] foo v0.0.1 ([CWD])
",
        )
        .run();

    validate_upload_foo();
}

// TODO: Deprecated
// remove once it has been decided --host can be removed
#[cargo_test]
fn simple_with_host() {
    registry::init();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
                authors = []
                license = "MIT"
                description = "foo"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("publish --no-verify --token sekrit --host")
        .arg(registry_url().to_string())
        .with_stderr(&format!(
            "\
[WARNING] The flag '--host' is no longer valid.

Previous versions of Cargo accepted this flag, but it is being
deprecated. The flag is being renamed to 'index', as the flag
wants the location of the index. Please use '--index' instead.

This will soon become a hard error, so it's either recommended
to update to a fixed version or contact the upstream maintainer
about this warning.
[UPDATING] `{reg}` index
[WARNING] manifest has no documentation, [..]
See [..]
[PACKAGING] foo v0.0.1 ([CWD])
[UPLOADING] foo v0.0.1 ([CWD])
",
            reg = registry_path().to_str().unwrap()
        ))
        .run();

    validate_upload_foo();
}

// TODO: Deprecated
// remove once it has been decided --host can be removed
#[cargo_test]
fn simple_with_index_and_host() {
    registry::init();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
                authors = []
                license = "MIT"
                description = "foo"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("publish --no-verify --token sekrit --index")
        .arg(registry_url().to_string())
        .arg("--host")
        .arg(registry_url().to_string())
        .with_stderr(&format!(
            "\
[WARNING] The flag '--host' is no longer valid.

Previous versions of Cargo accepted this flag, but it is being
deprecated. The flag is being renamed to 'index', as the flag
wants the location of the index. Please use '--index' instead.

This will soon become a hard error, so it's either recommended
to update to a fixed version or contact the upstream maintainer
about this warning.
[UPDATING] `{reg}` index
[WARNING] manifest has no documentation, [..]
See [..]
[PACKAGING] foo v0.0.1 ([CWD])
[UPLOADING] foo v0.0.1 ([CWD])
",
            reg = registry_path().to_str().unwrap()
        ))
        .run();

    validate_upload_foo();
}

#[cargo_test]
fn git_deps() {
    registry::init();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
                authors = []
                license = "MIT"
                description = "foo"

                [dependencies.foo]
                git = "git://path/to/nowhere"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("publish -v --no-verify --token sekrit")
        .with_status(101)
        .with_stderr(
            "\
[UPDATING] [..] index
[ERROR] all dependencies must have a version specified when publishing.
dependency `foo` does not specify a version
Note: The published dependency will use the version from crates.io,
the `git` specification will be removed from the dependency declaration.
",
        )
        .run();
}

#[cargo_test]
fn path_dependency_no_version() {
    registry::init();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
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

    p.cargo("publish --token sekrit")
        .with_status(101)
        .with_stderr(
            "\
[UPDATING] [..] index
[ERROR] all dependencies must have a version specified when publishing.
dependency `bar` does not specify a version
Note: The published dependency will use the version from crates.io,
the `path` specification will be removed from the dependency declaration.
",
        )
        .run();
}

#[cargo_test]
fn unpublishable_crate() {
    registry::init();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
                authors = []
                license = "MIT"
                description = "foo"
                publish = false
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("publish --index")
        .arg(registry_url().to_string())
        .with_status(101)
        .with_stderr(
            "\
[ERROR] `foo` cannot be published.
The registry `crates-io` is not listed in the `publish` value in Cargo.toml.
",
        )
        .run();
}

#[cargo_test]
fn dont_publish_dirty() {
    registry::init();
    let p = project().file("bar", "").build();

    let _ = git::repo(&paths::root().join("foo"))
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
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

    p.cargo("publish --token sekrit")
        .with_status(101)
        .with_stderr(
            "\
[UPDATING] `[..]` index
error: 1 files in the working directory contain changes that were not yet \
committed into git:

bar

to proceed despite this and include the uncommitted changes, pass the `--allow-dirty` flag
",
        )
        .run();
}

#[cargo_test]
fn publish_clean() {
    registry::init();

    let p = project().build();

    let _ = repo(&paths::root().join("foo"))
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
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

    p.cargo("publish --token sekrit").run();

    validate_upload_foo_clean();
}

#[cargo_test]
fn publish_in_sub_repo() {
    registry::init();

    let p = project().no_manifest().file("baz", "").build();

    let _ = repo(&paths::root().join("foo"))
        .file(
            "bar/Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
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

    p.cargo("publish --token sekrit").cwd("bar").run();

    validate_upload_foo_clean();
}

#[cargo_test]
fn publish_when_ignored() {
    registry::init();

    let p = project().file("baz", "").build();

    let _ = repo(&paths::root().join("foo"))
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
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

    p.cargo("publish --token sekrit").run();

    publish::validate_upload(
        CLEAN_FOO_JSON,
        "foo-0.0.1.crate",
        &[
            "Cargo.lock",
            "Cargo.toml",
            "Cargo.toml.orig",
            "src/main.rs",
            ".gitignore",
            ".cargo_vcs_info.json",
        ],
    );
}

#[cargo_test]
fn ignore_when_crate_ignored() {
    registry::init();

    let p = project().no_manifest().file("bar/baz", "").build();

    let _ = repo(&paths::root().join("foo"))
        .file(".gitignore", "bar")
        .nocommit_file(
            "bar/Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
                authors = []
                license = "MIT"
                description = "foo"
                documentation = "foo"
                homepage = "foo"
                repository = "foo"
            "#,
        )
        .nocommit_file("bar/src/main.rs", "fn main() {}");
    p.cargo("publish --token sekrit").cwd("bar").run();

    publish::validate_upload(
        CLEAN_FOO_JSON,
        "foo-0.0.1.crate",
        &[
            "Cargo.lock",
            "Cargo.toml",
            "Cargo.toml.orig",
            "src/main.rs",
            "baz",
        ],
    );
}

#[cargo_test]
fn new_crate_rejected() {
    registry::init();

    let p = project().file("baz", "").build();

    let _ = repo(&paths::root().join("foo"))
        .nocommit_file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
                authors = []
                license = "MIT"
                description = "foo"
                documentation = "foo"
                homepage = "foo"
                repository = "foo"
            "#,
        )
        .nocommit_file("src/main.rs", "fn main() {}");
    p.cargo("publish --token sekrit")
        .with_status(101)
        .with_stderr_contains(
            "[ERROR] 3 files in the working directory contain \
             changes that were not yet committed into git:",
        )
        .run();
}

#[cargo_test]
fn dry_run() {
    registry::init();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
                authors = []
                license = "MIT"
                description = "foo"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("publish --dry-run --index")
        .arg(registry_url().to_string())
        .with_stderr(
            "\
[UPDATING] `[..]` index
[WARNING] manifest has no documentation, [..]
See [..]
[PACKAGING] foo v0.0.1 ([CWD])
[VERIFYING] foo v0.0.1 ([CWD])
[COMPILING] foo v0.0.1 [..]
[FINISHED] dev [unoptimized + debuginfo] target(s) in [..]
[UPLOADING] foo v0.0.1 ([CWD])
[WARNING] aborting upload due to dry run
",
        )
        .run();

    // Ensure the API request wasn't actually made
    assert!(registry::api_path().join("api/v1/crates").exists());
    assert!(!registry::api_path().join("api/v1/crates/new").exists());
}

#[cargo_test]
fn registry_not_in_publish_list() {
    registry::init();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
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
        .with_stderr(
            "\
[ERROR] `foo` cannot be published.
The registry `alternative` is not listed in the `publish` value in Cargo.toml.
",
        )
        .run();
}

#[cargo_test]
fn publish_empty_list() {
    registry::init();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
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
        .with_stderr(
            "\
[ERROR] `foo` cannot be published.
The registry `alternative` is not listed in the `publish` value in Cargo.toml.
",
        )
        .run();
}

#[cargo_test]
fn publish_allowed_registry() {
    registry::alt_init();

    let p = project().build();

    let _ = repo(&paths::root().join("foo"))
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
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

    p.cargo("publish --registry alternative").run();

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
    registry::alt_init();

    let p = project().build();

    let _ = repo(&paths::root().join("foo"))
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
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

    p.cargo("publish").run();

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
fn publish_fail_with_no_registry_specified() {
    registry::init();

    let p = project().build();

    let _ = repo(&paths::root().join("foo"))
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
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
        .with_stderr(
            "\
[ERROR] `foo` cannot be published.
The registry `crates-io` is not listed in the `publish` value in Cargo.toml.
",
        )
        .run();
}

#[cargo_test]
fn block_publish_no_registry() {
    registry::init();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
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
        .with_stderr(
            "\
[ERROR] `foo` cannot be published.
The registry `alternative` is not listed in the `publish` value in Cargo.toml.
",
        )
        .run();
}

#[cargo_test]
fn publish_with_crates_io_explicit() {
    // Explicitly setting `crates-io` in the publish list.
    registry::init();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
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
        .with_stderr(
            "\
[ERROR] `foo` cannot be published.
The registry `alternative` is not listed in the `publish` value in Cargo.toml.
",
        )
        .run();

    p.cargo("publish").run();
}

#[cargo_test]
fn publish_with_select_features() {
    registry::init();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
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

    p.cargo("publish --features required --token sekrit")
        .with_stderr_contains("[UPLOADING] foo v0.0.1 ([CWD])")
        .run();
}

#[cargo_test]
fn publish_with_all_features() {
    registry::init();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
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

    p.cargo("publish --all-features --token sekrit")
        .with_stderr_contains("[UPLOADING] foo v0.0.1 ([CWD])")
        .run();
}

#[cargo_test]
fn publish_with_no_default_features() {
    registry::init();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
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

    p.cargo("publish --no-default-features --token sekrit")
        .with_stderr_contains("error: This crate requires `required` feature!")
        .with_status(101)
        .run();
}

#[cargo_test]
fn publish_with_patch() {
    Package::new("bar", "1.0.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
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
    p.cargo("publish --token sekrit")
        .with_stderr_contains("[..]newfunc[..]")
        .with_status(101)
        .run();

    // Remove the usage of new functionality and try again.
    p.change_file("src/main.rs", "extern crate bar; pub fn main() {}");

    p.cargo("publish --token sekrit").run();

    // Note, use of `registry` in the deps here is an artifact that this
    // publishes to a fake, local registry that is pretending to be crates.io.
    // Normal publishes would set it to null.
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
              "registry": "https://github.com/rust-lang/crates.io-index",
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
          "vers": "0.0.1"
          }
        "#,
        "foo-0.0.1.crate",
        &["Cargo.lock", "Cargo.toml", "Cargo.toml.orig", "src/main.rs"],
    );
}

#[cargo_test]
fn publish_checks_for_token_before_verify() {
    registry::init();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
                authors = []
                license = "MIT"
                description = "foo"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    let credentials = paths::home().join(".cargo/credentials");
    fs::remove_file(&credentials).unwrap();

    // Assert upload token error before the package is verified
    p.cargo("publish")
        .with_status(101)
        .with_stderr_contains(
            "[ERROR] no upload token found, \
            please run `cargo login` or pass `--token`",
        )
        .with_stderr_does_not_contain("[VERIFYING] foo v0.0.1 ([CWD])")
        .run();

    // Assert package verified successfully on dry run
    p.cargo("publish --dry-run")
        .with_status(0)
        .with_stderr_contains("[VERIFYING] foo v0.0.1 ([CWD])")
        .run();
}

#[cargo_test]
fn publish_with_bad_source() {
    let p = project()
        .file(
            ".cargo/config",
            r#"
            [source.crates-io]
            replace-with = 'local-registry'

            [source.local-registry]
            local-registry = 'registry'
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("publish --token sekrit")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] registry `[..]/foo/registry` does not support API commands.
Check for a source-replacement in .cargo/config.
",
        )
        .run();

    p.change_file(
        ".cargo/config",
        r#"
        [source.crates-io]
        replace-with = "vendored-sources"

        [source.vendored-sources]
        directory = "vendor"
        "#,
    );

    p.cargo("publish --token sekrit")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] dir [..]/foo/vendor does not support API commands.
Check for a source-replacement in .cargo/config.
",
        )
        .run();
}

#[cargo_test]
fn publish_git_with_version() {
    // A dependency with both `git` and `version`.
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

    p.cargo("run").with_stdout("2").run();
    p.cargo("publish --no-verify --token sekrit").run();

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
              "registry": "https://github.com/rust-lang/crates.io-index",
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
          "vers": "0.1.0"
          }
        "#,
        "foo-0.1.0.crate",
        &["Cargo.lock", "Cargo.toml", "Cargo.toml.orig", "src/main.rs"],
        &[
            (
                "Cargo.toml",
                // Check that only `version` is included in Cargo.toml.
                &format!(
                    "{}\n\
                     [package]\n\
                     edition = \"2018\"\n\
                     name = \"foo\"\n\
                     version = \"0.1.0\"\n\
                     authors = []\n\
                     description = \"foo\"\n\
                     license = \"MIT\"\n\
                     [dependencies.dep1]\n\
                     version = \"1.0\"\n\
                    ",
                    cargo::core::package::MANIFEST_PREAMBLE
                ),
            ),
            (
                "Cargo.lock",
                // The important check here is that it is 1.0.1 in the registry.
                "# This file is automatically @generated by Cargo.\n\
                 # It is not intended for manual editing.\n\
                 version = 3\n\
                 \n\
                 [[package]]\n\
                 name = \"dep1\"\n\
                 version = \"1.0.1\"\n\
                 source = \"registry+https://github.com/rust-lang/crates.io-index\"\n\
                 checksum = \"[..]\"\n\
                 \n\
                 [[package]]\n\
                 name = \"foo\"\n\
                 version = \"0.1.0\"\n\
                 dependencies = [\n\
                 \x20\"dep1\",\n\
                 ]\n\
                 ",
            ),
        ],
    );
}

#[cargo_test]
fn publish_dev_dep_no_version() {
    registry::init();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"
            authors = []
            license = "MIT"
            description = "foo"
            documentation = "foo"
            homepage = "foo"
            repository = "foo"

            [dev-dependencies]
            bar = { path = "bar" }
            "#,
        )
        .file("src/lib.rs", "")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file("bar/src/lib.rs", "")
        .build();

    p.cargo("publish --no-verify --token sekrit")
        .with_stderr(
            "\
[UPDATING] [..]
[PACKAGING] foo v0.1.0 [..]
[UPLOADING] foo v0.1.0 [..]
",
        )
        .run();

    publish::validate_upload_with_contents(
        r#"
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
          "vers": "0.1.0"
        }
        "#,
        "foo-0.1.0.crate",
        &["Cargo.toml", "Cargo.toml.orig", "src/lib.rs"],
        &[(
            "Cargo.toml",
            &format!(
                r#"{}
[package]
name = "foo"
version = "0.1.0"
authors = []
description = "foo"
homepage = "foo"
documentation = "foo"
license = "MIT"
repository = "foo"

[dev-dependencies]
"#,
                cargo::core::package::MANIFEST_PREAMBLE
            ),
        )],
    );
}

#[cargo_test]
fn credentials_ambiguous_filename() {
    registry::init();

    let credentials_toml = paths::home().join(".cargo/credentials.toml");
    fs::write(credentials_toml, r#"token = "api-token""#).unwrap();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
                authors = []
                license = "MIT"
                description = "foo"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("publish --no-verify --token sekrit")
        .with_stderr_contains(
            "\
[WARNING] Both `[..]/credentials` and `[..]/credentials.toml` exist. Using `[..]/credentials`
",
        )
        .run();

    validate_upload_foo();
}

#[cargo_test]
fn index_requires_token() {
    // --index will not load registry.token to avoid possibly leaking
    // crates.io token to another server.
    registry::init();
    let credentials = paths::home().join(".cargo/credentials");
    fs::remove_file(&credentials).unwrap();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.0.1"
            authors = []
            license = "MIT"
            description = "foo"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("publish --no-verify --index")
        .arg(registry_url().to_string())
        .with_status(101)
        .with_stderr(
            "\
[UPDATING] [..]
[ERROR] command-line argument --index requires --token to be specified
",
        )
        .run();
}

#[cargo_test]
fn registry_token_with_source_replacement() {
    // publish with source replacement without --token
    registry::init();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
                authors = []
                license = "MIT"
                description = "foo"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("publish --no-verify")
        .with_stderr(
            "\
[UPDATING] [..]
[WARNING] using `registry.token` config value with source replacement is deprecated
This may become a hard error in the future[..]
Use the --token command-line flag to remove this warning.
[WARNING] manifest has no documentation, [..]
See [..]
[PACKAGING] foo v0.0.1 ([CWD])
[UPLOADING] foo v0.0.1 ([CWD])
",
        )
        .run();
}

#[cargo_test]
fn publish_with_missing_readme() {
    registry::init();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                authors = []
                license = "MIT"
                description = "foo"
                homepage = "https://example.com/"
                readme = "foo.md"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("publish --no-verify --token sekrit")
        .with_status(101)
        .with_stderr(&format!(
            "\
[UPDATING] [..]
[PACKAGING] foo v0.1.0 [..]
[UPLOADING] foo v0.1.0 [..]
[ERROR] failed to read `readme` file for package `foo v0.1.0 ([ROOT]/foo)`

Caused by:
  failed to read `[ROOT]/foo/foo.md`

Caused by:
  {}
",
            no_such_file_err_msg()
        ))
        .run();
}

#[cargo_test]
fn api_error_json() {
    // Registry returns an API error.
    let t = registry::RegistryBuilder::new().build_api_server(&|_headers| {
        (403, &r#"{"errors": [{"detail": "you must be logged in"}]}"#)
    });

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
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
        .with_stderr(
            "\
[UPDATING] [..]
[PACKAGING] foo v0.0.1 [..]
[UPLOADING] foo v0.0.1 [..]
[ERROR] failed to publish to registry at http://127.0.0.1:[..]/

Caused by:
  the remote server responded with an error (status 403 Forbidden): you must be logged in
",
        )
        .run();

    t.join().unwrap();
}

#[cargo_test]
fn api_error_200() {
    // Registry returns an API error with a 200 status code.
    let t = registry::RegistryBuilder::new().build_api_server(&|_headers| {
        (
            200,
            &r#"{"errors": [{"detail": "max upload size is 123"}]}"#,
        )
    });

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
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
        .with_stderr(
            "\
[UPDATING] [..]
[PACKAGING] foo v0.0.1 [..]
[UPLOADING] foo v0.0.1 [..]
[ERROR] failed to publish to registry at http://127.0.0.1:[..]/

Caused by:
  the remote server responded with an error: max upload size is 123
",
        )
        .run();

    t.join().unwrap();
}

#[cargo_test]
fn api_error_code() {
    // Registry returns an error code without a JSON message.
    let t = registry::RegistryBuilder::new().build_api_server(&|_headers| (400, &"go away"));

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
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
        .with_stderr(
            "\
[UPDATING] [..]
[PACKAGING] foo v0.0.1 [..]
[UPLOADING] foo v0.0.1 [..]
[ERROR] failed to publish to registry at http://127.0.0.1:[..]/

Caused by:
  failed to get a 200 OK response, got 400
  headers:
  <tab>HTTP/1.1 400
  <tab>Content-Length: 7
  <tab>
  body:
  go away
",
        )
        .run();

    t.join().unwrap();
}

#[cargo_test]
fn api_curl_error() {
    // Registry has a network error.
    let t = registry::RegistryBuilder::new().build_api_server(&|_headers| panic!("broke!"));

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
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
        .with_stderr(
            "\
[UPDATING] [..]
[PACKAGING] foo v0.0.1 [..]
[UPLOADING] foo v0.0.1 [..]
[ERROR] failed to publish to registry at http://127.0.0.1:[..]/

Caused by:
  [52] [..]
",
        )
        .run();

    let e = t.join().unwrap_err();
    assert_eq!(*e.downcast::<&str>().unwrap(), "broke!");
}

#[cargo_test]
fn api_other_error() {
    // Registry returns an invalid response.
    let t = registry::RegistryBuilder::new().build_api_server(&|_headers| (200, b"\xff"));

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
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
        .with_stderr(
            "\
[UPDATING] [..]
[PACKAGING] foo v0.0.1 [..]
[UPLOADING] foo v0.0.1 [..]
[ERROR] failed to publish to registry at http://127.0.0.1:[..]/

Caused by:
  invalid response from server

Caused by:
  response body was not valid utf-8
",
        )
        .run();

    t.join().unwrap();
}
