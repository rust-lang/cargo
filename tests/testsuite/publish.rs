use std::fs::{self, File};
use std::io::prelude::*;
use std::io::SeekFrom;

use flate2::read::GzDecoder;
use crate::support::git::repo;
use crate::support::paths;
use crate::support::{basic_manifest, project, publish};
use tar::Archive;

#[test]
fn simple() {
    publish::setup();

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
        ).file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("publish --no-verify --index")
        .arg(publish::registry().to_string())
        .with_stderr(&format!(
            "\
[UPDATING] `{reg}` index
[WARNING] manifest has no documentation, [..]
See [..]
[PACKAGING] foo v0.0.1 ([CWD])
[UPLOADING] foo v0.0.1 ([CWD])
",
            reg = publish::registry_path().to_str().unwrap()
        )).run();

    let mut f = File::open(&publish::upload_path().join("api/v1/crates/new")).unwrap();
    // Skip the metadata payload and the size of the tarball
    let mut sz = [0; 4];
    assert_eq!(f.read(&mut sz).unwrap(), 4);
    let sz = (u32::from(sz[0]) << 0)
        | (u32::from(sz[1]) << 8)
        | (u32::from(sz[2]) << 16)
        | (u32::from(sz[3]) << 24);
    f.seek(SeekFrom::Current(i64::from(sz) + 4)).unwrap();

    // Verify the tarball
    let mut rdr = GzDecoder::new(f);
    assert_eq!(
        rdr.header().unwrap().filename().unwrap(),
        b"foo-0.0.1.crate"
    );
    let mut contents = Vec::new();
    rdr.read_to_end(&mut contents).unwrap();
    let mut ar = Archive::new(&contents[..]);
    for file in ar.entries().unwrap() {
        let file = file.unwrap();
        let fname = file.header().path_bytes();
        let fname = &*fname;
        assert!(
            fname == b"foo-0.0.1/Cargo.toml"
                || fname == b"foo-0.0.1/Cargo.toml.orig"
                || fname == b"foo-0.0.1/src/main.rs",
            "unexpected filename: {:?}",
            file.header().path()
        );
    }
}

#[test]
fn old_token_location() {
    publish::setup();

    // publish::setup puts a token in this file.
    fs::remove_file(paths::root().join(".cargo/config")).unwrap();

    let credentials = paths::root().join("home/.cargo/credentials");
    File::create(credentials)
        .unwrap()
        .write_all(br#"token = "api-token""#)
        .unwrap();

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
        ).file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("publish --no-verify --index")
        .arg(publish::registry().to_string())
        .with_stderr(&format!(
            "\
[UPDATING] `{reg}` index
[WARNING] manifest has no documentation, [..]
See [..]
[PACKAGING] foo v0.0.1 ([CWD])
[UPLOADING] foo v0.0.1 ([CWD])
",
            reg = publish::registry_path().to_str().unwrap()
        )).run();

    let mut f = File::open(&publish::upload_path().join("api/v1/crates/new")).unwrap();
    // Skip the metadata payload and the size of the tarball
    let mut sz = [0; 4];
    assert_eq!(f.read(&mut sz).unwrap(), 4);
    let sz = (u32::from(sz[0]) << 0)
        | (u32::from(sz[1]) << 8)
        | (u32::from(sz[2]) << 16)
        | (u32::from(sz[3]) << 24);
    f.seek(SeekFrom::Current(i64::from(sz) + 4)).unwrap();

    // Verify the tarball
    let mut rdr = GzDecoder::new(f);
    assert_eq!(
        rdr.header().unwrap().filename().unwrap(),
        b"foo-0.0.1.crate"
    );
    let mut contents = Vec::new();
    rdr.read_to_end(&mut contents).unwrap();
    let mut ar = Archive::new(&contents[..]);
    for file in ar.entries().unwrap() {
        let file = file.unwrap();
        let fname = file.header().path_bytes();
        let fname = &*fname;
        assert!(
            fname == b"foo-0.0.1/Cargo.toml"
                || fname == b"foo-0.0.1/Cargo.toml.orig"
                || fname == b"foo-0.0.1/src/main.rs",
            "unexpected filename: {:?}",
            file.header().path()
        );
    }
}

// TODO: Deprecated
// remove once it has been decided --host can be removed
#[test]
fn simple_with_host() {
    publish::setup();

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
        ).file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("publish --no-verify --host")
        .arg(publish::registry().to_string())
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
            reg = publish::registry_path().to_str().unwrap()
        )).run();

    let mut f = File::open(&publish::upload_path().join("api/v1/crates/new")).unwrap();
    // Skip the metadata payload and the size of the tarball
    let mut sz = [0; 4];
    assert_eq!(f.read(&mut sz).unwrap(), 4);
    let sz = (u32::from(sz[0]) << 0)
        | (u32::from(sz[1]) << 8)
        | (u32::from(sz[2]) << 16)
        | (u32::from(sz[3]) << 24);
    f.seek(SeekFrom::Current(i64::from(sz) + 4)).unwrap();

    // Verify the tarball
    let mut rdr = GzDecoder::new(f);
    assert_eq!(
        rdr.header().unwrap().filename().unwrap(),
        b"foo-0.0.1.crate"
    );
    let mut contents = Vec::new();
    rdr.read_to_end(&mut contents).unwrap();
    let mut ar = Archive::new(&contents[..]);
    for file in ar.entries().unwrap() {
        let file = file.unwrap();
        let fname = file.header().path_bytes();
        let fname = &*fname;
        assert!(
            fname == b"foo-0.0.1/Cargo.toml"
                || fname == b"foo-0.0.1/Cargo.toml.orig"
                || fname == b"foo-0.0.1/src/main.rs",
            "unexpected filename: {:?}",
            file.header().path()
        );
    }
}

// TODO: Deprecated
// remove once it has been decided --host can be removed
#[test]
fn simple_with_index_and_host() {
    publish::setup();

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
        ).file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("publish --no-verify --index")
        .arg(publish::registry().to_string())
        .arg("--host")
        .arg(publish::registry().to_string())
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
            reg = publish::registry_path().to_str().unwrap()
        )).run();

    let mut f = File::open(&publish::upload_path().join("api/v1/crates/new")).unwrap();
    // Skip the metadata payload and the size of the tarball
    let mut sz = [0; 4];
    assert_eq!(f.read(&mut sz).unwrap(), 4);
    let sz = (u32::from(sz[0]) << 0)
        | (u32::from(sz[1]) << 8)
        | (u32::from(sz[2]) << 16)
        | (u32::from(sz[3]) << 24);
    f.seek(SeekFrom::Current(i64::from(sz) + 4)).unwrap();

    // Verify the tarball
    let mut rdr = GzDecoder::new(f);
    assert_eq!(
        rdr.header().unwrap().filename().unwrap(),
        b"foo-0.0.1.crate"
    );
    let mut contents = Vec::new();
    rdr.read_to_end(&mut contents).unwrap();
    let mut ar = Archive::new(&contents[..]);
    for file in ar.entries().unwrap() {
        let file = file.unwrap();
        let fname = file.header().path_bytes();
        let fname = &*fname;
        assert!(
            fname == b"foo-0.0.1/Cargo.toml"
                || fname == b"foo-0.0.1/Cargo.toml.orig"
                || fname == b"foo-0.0.1/src/main.rs",
            "unexpected filename: {:?}",
            file.header().path()
        );
    }
}

#[test]
fn git_deps() {
    publish::setup();

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
        ).file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("publish -v --no-verify --index")
        .arg(publish::registry().to_string())
        .with_status(101)
        .with_stderr(
            "\
[UPDATING] [..] index
[ERROR] crates cannot be published to crates.io with dependencies sourced from \
a repository\neither publish `foo` as its own crate on crates.io and \
specify a crates.io version as a dependency or pull it into this \
repository and specify it with a path and version\n\
(crate `foo` has repository path `git://path/to/nowhere`)\
",
        ).run();
}

#[test]
fn path_dependency_no_version() {
    publish::setup();

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
        ).file("src/main.rs", "fn main() {}")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file("bar/src/lib.rs", "")
        .build();

    p.cargo("publish --index")
        .arg(publish::registry().to_string())
        .with_status(101)
        .with_stderr(
            "\
[UPDATING] [..] index
[ERROR] all path dependencies must have a version specified when publishing.
dependency `bar` does not specify a version
",
        ).run();
}

#[test]
fn unpublishable_crate() {
    publish::setup();

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
        ).file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("publish --index")
        .arg(publish::registry().to_string())
        .with_status(101)
        .with_stderr(
            "\
[ERROR] some crates cannot be published.
`foo` is marked as unpublishable
",
        ).run();
}

#[test]
fn dont_publish_dirty() {
    publish::setup();
    let p = project().file("bar", "").build();

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
        ).file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("publish --index")
        .arg(publish::registry().to_string())
        .with_status(101)
        .with_stderr(
            "\
[UPDATING] `[..]` index
error: 1 files in the working directory contain changes that were not yet \
committed into git:

bar

to proceed despite this, pass the `--allow-dirty` flag
",
        ).run();
}

#[test]
fn publish_clean() {
    publish::setup();

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
        ).file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("publish --index")
        .arg(publish::registry().to_string())
        .run();
}

#[test]
fn publish_in_sub_repo() {
    publish::setup();

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
        ).file("bar/src/main.rs", "fn main() {}")
        .build();

    p.cargo("publish")
        .cwd(p.root().join("bar"))
        .arg("--index")
        .arg(publish::registry().to_string())
        .run();
}

#[test]
fn publish_when_ignored() {
    publish::setup();

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
        ).file("src/main.rs", "fn main() {}")
        .file(".gitignore", "baz")
        .build();

    p.cargo("publish --index")
        .arg(publish::registry().to_string())
        .run();
}

#[test]
fn ignore_when_crate_ignored() {
    publish::setup();

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
        ).nocommit_file("bar/src/main.rs", "fn main() {}");
    p.cargo("publish")
        .cwd(p.root().join("bar"))
        .arg("--index")
        .arg(publish::registry().to_string())
        .run();
}

#[test]
fn new_crate_rejected() {
    publish::setup();

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
        ).nocommit_file("src/main.rs", "fn main() {}");
    p.cargo("publish --index")
        .arg(publish::registry().to_string())
        .with_status(101)
        .run();
}

#[test]
fn dry_run() {
    publish::setup();

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
        ).file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("publish --dry-run --index")
        .arg(publish::registry().to_string())
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
        ).run();

    // Ensure the API request wasn't actually made
    assert!(!publish::upload_path().join("api/v1/crates/new").exists());
}

#[test]
fn block_publish_feature_not_enabled() {
    publish::setup();

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
        ).file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("publish --registry alternative -Zunstable-options")
        .masquerade_as_nightly_cargo()
        .with_status(101)
        .with_stderr(
            "\
error: failed to parse manifest at `[..]`

Caused by:
  the `publish` manifest key is unstable for anything other than a value of true or false

Caused by:
  feature `alternative-registries` is required

consider adding `cargo-features = [\"alternative-registries\"]` to the manifest
",
        ).run();
}

#[test]
fn registry_not_in_publish_list() {
    publish::setup();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            cargo-features = ["alternative-registries"]

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
        ).file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("publish")
        .masquerade_as_nightly_cargo()
        .arg("--registry")
        .arg("alternative")
        .arg("-Zunstable-options")
        .with_status(101)
        .with_stderr(
            "\
[ERROR] some crates cannot be published.
`foo` is marked as unpublishable
",
        ).run();
}

#[test]
fn publish_empty_list() {
    publish::setup();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            cargo-features = ["alternative-registries"]

            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
            license = "MIT"
            description = "foo"
            publish = []
        "#,
        ).file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("publish --registry alternative -Zunstable-options")
        .masquerade_as_nightly_cargo()
        .with_status(101)
        .with_stderr(
            "\
[ERROR] some crates cannot be published.
`foo` is marked as unpublishable
",
        ).run();
}

#[test]
fn publish_allowed_registry() {
    publish::setup();

    let p = project().build();

    let _ = repo(&paths::root().join("foo"))
        .file(
            "Cargo.toml",
            r#"
            cargo-features = ["alternative-registries"]

            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
            license = "MIT"
            description = "foo"
            documentation = "foo"
            homepage = "foo"
            publish = ["alternative"]
        "#,
        ).file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("publish --registry alternative -Zunstable-options")
        .masquerade_as_nightly_cargo()
        .run();
}

#[test]
fn block_publish_no_registry() {
    publish::setup();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
            cargo-features = ["alternative-registries"]

            [project]
            name = "foo"
            version = "0.0.1"
            authors = []
            license = "MIT"
            description = "foo"
            publish = []
        "#,
        ).file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("publish --registry alternative -Zunstable-options")
        .masquerade_as_nightly_cargo()
        .with_status(101)
        .with_stderr(
            "\
[ERROR] some crates cannot be published.
`foo` is marked as unpublishable
",
        ).run();
}
