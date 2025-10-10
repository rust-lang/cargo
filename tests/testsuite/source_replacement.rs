//! Tests for `[source]` table (source replacement).

use std::fs;

use crate::prelude::*;
use crate::utils::cargo_process;
use cargo_test_support::registry::{Package, RegistryBuilder, TestRegistry};
use cargo_test_support::{paths, project, str, t};

fn setup_replacement(config: &str) -> TestRegistry {
    let crates_io = RegistryBuilder::new()
        .no_configure_registry()
        .http_api()
        .build();

    let root = paths::root();
    t!(fs::create_dir(&root.join(".cargo")));
    t!(fs::write(root.join(".cargo/config.toml"), config,));
    crates_io
}

#[cargo_test]
fn crates_io_token_not_sent_to_replacement() {
    // verifies that the crates.io token is not sent to a replacement registry during publish.
    let crates_io = setup_replacement(
        r#"
        [source.crates-io]
        replace-with = 'alternative'
    "#,
    );
    let _alternative = RegistryBuilder::new()
        .alternative()
        .http_api()
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
        .file("src/lib.rs", "")
        .build();

    p.cargo("publish --no-verify --registry crates-io")
        .replace_crates_io(crates_io.index_url())
        .with_stderr_data(str![[r#"
[UPDATING] crates.io index
...
"#]])
        .run();
}

#[cargo_test]
fn token_sent_to_correct_registry() {
    // verifies that the crates.io token is not sent to a replacement registry during yank.
    let crates_io = setup_replacement(
        r#"
        [source.crates-io]
        replace-with = 'alternative'
    "#,
    );
    let _alternative = RegistryBuilder::new().alternative().http_api().build();

    cargo_process("yank foo@0.0.1 --registry crates-io")
        .replace_crates_io(crates_io.index_url())
        .with_stderr_data(str![[r#"
[UPDATING] crates.io index
[YANK] foo@0.0.1

"#]])
        .run();

    cargo_process("yank foo@0.0.1 --registry alternative")
        .replace_crates_io(crates_io.index_url())
        .with_stderr_data(str![[r#"
[UPDATING] `alternative` index
[YANK] foo@0.0.1

"#]])
        .run();
}

#[cargo_test]
fn ambiguous_registry() {
    // verifies that an error is issued when a source-replacement is configured
    // and no --registry argument is given.
    let crates_io = setup_replacement(
        r#"
        [source.crates-io]
        replace-with = 'alternative'
    "#,
    );
    let _alternative = RegistryBuilder::new()
        .alternative()
        .http_api()
        .no_configure_token()
        .build();

    cargo_process("yank foo@0.0.1")
        .replace_crates_io(crates_io.index_url())
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] crates-io is replaced with remote registry alternative;
include `--registry alternative` or `--registry crates-io`

"#]])
        .run();
}

#[cargo_test]
fn yank_with_default_crates_io() {
    // verifies that no error is given when registry.default is used.
    let crates_io = setup_replacement(
        r#"
        [source.crates-io]
        replace-with = 'alternative'

        [registry]
        default = 'crates-io'
    "#,
    );
    let _alternative = RegistryBuilder::new().alternative().http_api().build();

    cargo_process("yank foo@0.0.1")
        .replace_crates_io(crates_io.index_url())
        .with_stderr_data(str![[r#"
[UPDATING] crates.io index
[YANK] foo@0.0.1

"#]])
        .run();
}

#[cargo_test]
fn yank_with_default_alternative() {
    // verifies that no error is given when registry.default is an alt registry.
    let crates_io = setup_replacement(
        r#"
        [source.crates-io]
        replace-with = 'alternative'

        [registry]
        default = 'alternative'
    "#,
    );
    let _alternative = RegistryBuilder::new().alternative().http_api().build();

    cargo_process("yank foo@0.0.1")
        .replace_crates_io(crates_io.index_url())
        .with_stderr_data(str![[r#"
[UPDATING] `alternative` index
[YANK] foo@0.0.1

"#]])
        .run();
}

#[cargo_test]
fn publish_with_replacement() {
    // verifies that the crates.io token is not sent to a replacement registry during publish.
    let crates_io = setup_replacement(
        r#"
        [source.crates-io]
        replace-with = 'alternative'
    "#,
    );
    let _alternative = RegistryBuilder::new()
        .alternative()
        .http_api()
        .no_configure_token()
        .build();

    // Publish bar only to alternative. This tests that the publish verification build
    // does uses the source replacement.
    Package::new("bar", "1.0.0").alternative(true).publish();

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
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    // Verifies that the crates.io index is used to find the publishing endpoint
    // and that the crate is sent to crates.io. The source replacement is only used
    // for the verification step.
    p.cargo("publish --registry crates-io")
        .replace_crates_io(crates_io.index_url())
        .with_stderr_data(str![[r#"
[UPDATING] crates.io index
[WARNING] manifest has no documentation, homepage or repository
  |
  = [NOTE] see https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata for more info
[PACKAGING] foo v0.0.1 ([ROOT]/foo)
[UPDATING] `alternative` index
[PACKAGED] 4 files, [FILE_SIZE]B ([FILE_SIZE]B compressed)
[VERIFYING] foo v0.0.1 ([ROOT]/foo)
[DOWNLOADING] crates ...
[DOWNLOADED] bar v1.0.0 (registry `alternative`)
[COMPILING] bar v1.0.0
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
fn undefined_default() {
    // verifies that no error is given when registry.default is used.
    let crates_io = setup_replacement(
        r#"
        [registry]
        default = 'undefined'
    "#,
    );

    cargo_process("yank foo@0.0.1")
        .replace_crates_io(crates_io.index_url())
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] registry index was not found in any configuration: `undefined`

"#]])
        .run();
}

#[cargo_test]
fn source_replacement_with_registry_url() {
    let alternative = RegistryBuilder::new().alternative().http_api().build();
    Package::new("bar", "0.0.1").alternative(true).publish();

    let crates_io = setup_replacement(&format!(
        r#"
        [source.crates-io]
        replace-with = 'using-registry-url'

        [source.using-registry-url]
        registry = '{}'
        "#,
        alternative.index_url()
    ));

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
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check")
        .replace_crates_io(crates_io.index_url())
        .with_stderr_data(str![[r#"
[UPDATING] `using-registry-url` index
[LOCKING] 1 package to latest compatible version
[DOWNLOADING] crates ...
[DOWNLOADED] bar v0.0.1 (registry `using-registry-url`)
[CHECKING] bar v0.0.1
[CHECKING] foo v0.0.1 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn source_replacement_with_no_package_in_directory() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2021"

                [dependencies]
                bar = { version = "^0.8.9" }
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    let root = paths::root();
    t!(fs::create_dir(&root.join("vendor")));

    let crates_io = setup_replacement(&format!(
        r#"
            [source.crates-io]
            replace-with = "vendored-sources"

            [source.vendored-sources]
            directory = "vendor"
        "#
    ));

    p.cargo("build")
        .replace_crates_io(crates_io.index_url())
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] no matching package named `bar` found
location searched: directory source `[ROOT]/vendor` (which is replacing registry `crates-io`)
required by package `foo v0.1.0 ([ROOT]/foo)`

"#]])
        .run();
}
