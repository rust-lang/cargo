//! Tests for SHA256 git repository support (`-Zgit=sha256`).

use cargo_test_support::basic_manifest;
use cargo_test_support::git;
use cargo_test_support::project;
use cargo_test_support::str;

use crate::prelude::*;

#[cargo_test]
fn sha256_gated_libgit2() {
    let (git_dep, _repo) = git::new_sha256_repo("dep1", |p| {
        p.file("Cargo.toml", &basic_manifest("dep1", "1.0.0"))
            .file("src/lib.rs", "")
    });

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"
                    edition = "2021"

                    [dependencies]
                    dep1 = {{ git = '{}' }}
                "#,
                git_dep.url()
            ),
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] git repository `[ROOTURL]/dep1`
[ERROR] failed to get `dep1` as a dependency of package `foo v0.0.0 ([ROOT]/foo)`

Caused by:
  failed to load source for dependency `dep1`

Caused by:
  unable to update [ROOTURL]/dep1

Caused by:
  failed to clone into: [ROOT]/home/.cargo/git/db/dep1-[HASH]

Caused by:
  unexpected data at the end of the pack; class=Indexer (15)

"#]])
        .run();
}

#[cargo_test]
fn sha256_gated_with_cached_db() {
    let (git_dep, _repo) = git::new_sha256_repo("dep1", |p| {
        p.file("Cargo.toml", &basic_manifest("dep1", "1.0.0"))
            .file("src/lib.rs", "")
    });

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"
                    edition = "2021"

                    [dependencies]
                    dep1 = {{ git = '{}' }}
                "#,
                git_dep.url()
            ),
        )
        .file("src/lib.rs", "")
        .build();

    // Populate the SHA256 db cache
    p.cargo("check -Zgit=sha256")
        .masquerade_as_nightly_cargo(&["git=sha256"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] git repository `[ROOTURL]/dep1`
[ERROR] failed to get `dep1` as a dependency of package `foo v0.0.0 ([ROOT]/foo)`

Caused by:
  failed to load source for dependency `dep1`

Caused by:
  unable to update [ROOTURL]/dep1

Caused by:
  failed to clone into: [ROOT]/home/.cargo/git/db/dep1-[HASH]

Caused by:
  unexpected data at the end of the pack; class=Indexer (15)

"#]])
        .run();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] git repository `[ROOTURL]/dep1`
[ERROR] failed to get `dep1` as a dependency of package `foo v0.0.0 ([ROOT]/foo)`

Caused by:
  failed to load source for dependency `dep1`

Caused by:
  unable to update [ROOTURL]/dep1

Caused by:
  failed to fetch into: [ROOT]/home/.cargo/git/db/dep1-[HASH]

Caused by:
  unexpected data at the end of the pack; class=Indexer (15)

"#]])
        .run();
}

#[cargo_test]
fn sha256_gated_with_lockfile() {
    let (git_dep, _repo) = git::new_sha256_repo("dep1", |p| {
        p.file("Cargo.toml", &basic_manifest("dep1", "1.0.0"))
            .file("src/lib.rs", "")
    });

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"
                    edition = "2021"

                    [dependencies]
                    dep1 = {{ git = '{}' }}
                "#,
                git_dep.url()
            ),
        )
        .file("src/lib.rs", "")
        .build();

    // Generate a lockfile with a SHA256 rev
    p.cargo("check -Zgit=sha256")
        .masquerade_as_nightly_cargo(&["git=sha256"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] git repository `[ROOTURL]/dep1`
[ERROR] failed to get `dep1` as a dependency of package `foo v0.0.0 ([ROOT]/foo)`

Caused by:
  failed to load source for dependency `dep1`

Caused by:
  unable to update [ROOTURL]/dep1

Caused by:
  failed to clone into: [ROOT]/home/.cargo/git/db/dep1-[HASH]

Caused by:
  unexpected data at the end of the pack; class=Indexer (15)

"#]])
        .run();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] git repository `[ROOTURL]/dep1`
[ERROR] failed to get `dep1` as a dependency of package `foo v0.0.0 ([ROOT]/foo)`

Caused by:
  failed to load source for dependency `dep1`

Caused by:
  unable to update [ROOTURL]/dep1

Caused by:
  failed to fetch into: [ROOT]/home/.cargo/git/db/dep1-[HASH]

Caused by:
  unexpected data at the end of the pack; class=Indexer (15)

"#]])
        .run();
}

#[cargo_test]
fn sha256_gated_gitoxide_with_sha256_flag() {
    let (git_dep, _repo) = git::new_sha256_repo("dep1", |p| {
        p.file("Cargo.toml", &basic_manifest("dep1", "1.0.0"))
            .file("src/lib.rs", "")
    });

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"
                    edition = "2021"

                    [dependencies]
                    dep1 = {{ git = '{}' }}
                "#,
                git_dep.url()
            ),
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Zgit=sha256 -Zgitoxide=fetch")
        .masquerade_as_nightly_cargo(&["git=sha256", "gitoxide=fetch"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] git repository `[ROOTURL]/dep1`
[ERROR] failed to get `dep1` as a dependency of package `foo v0.0.0 ([ROOT]/foo)`

Caused by:
  failed to load source for dependency `dep1`

Caused by:
  unable to update [ROOTURL]/dep1

Caused by:
  failed to clone into: [ROOT]/home/.cargo/git/db/dep1-[HASH]

Caused by:
  failed to fill whole buffer

"#]])
        .run();
}

#[cargo_test]
fn sha256_gated_gitoxide_without_sha256_flag() {
    let (git_dep, _repo) = git::new_sha256_repo("dep1", |p| {
        p.file("Cargo.toml", &basic_manifest("dep1", "1.0.0"))
            .file("src/lib.rs", "")
    });

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"
                    edition = "2021"

                    [dependencies]
                    dep1 = {{ git = '{}' }}
                "#,
                git_dep.url()
            ),
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Zgitoxide=fetch")
        .masquerade_as_nightly_cargo(&["gitoxide=fetch"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] git repository `[ROOTURL]/dep1`
[ERROR] failed to get `dep1` as a dependency of package `foo v0.0.0 ([ROOT]/foo)`

Caused by:
  failed to load source for dependency `dep1`

Caused by:
  unable to update [ROOTURL]/dep1

Caused by:
  failed to clone into: [ROOT]/home/.cargo/git/db/dep1-[HASH]

Caused by:
  failed to fill whole buffer

"#]])
        .run();
}

#[cargo_test]
fn sha256_basic() {
    let (git_dep, _repo) = git::new_sha256_repo("dep1", |p| {
        p.file("Cargo.toml", &basic_manifest("dep1", "1.0.0"))
            .file("src/lib.rs", "")
    });

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"
                    edition = "2021"

                    [dependencies]
                    dep1 = {{ git = '{}' }}
                "#,
                git_dep.url()
            ),
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Zgit=sha256")
        .masquerade_as_nightly_cargo(&["git=sha256"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] git repository `[ROOTURL]/dep1`
[ERROR] failed to get `dep1` as a dependency of package `foo v0.0.0 ([ROOT]/foo)`

Caused by:
  failed to load source for dependency `dep1`

Caused by:
  unable to update [ROOTURL]/dep1

Caused by:
  failed to clone into: [ROOT]/home/.cargo/git/db/dep1-[HASH]

Caused by:
  unexpected data at the end of the pack; class=Indexer (15)

"#]])
        .run();
}

#[cargo_test]
fn sha256_dep_with_rev() {
    let (git_dep, repo) = git::new_sha256_repo("dep1", |p| {
        p.file("Cargo.toml", &basic_manifest("dep1", "1.0.0"))
            .file("src/lib.rs", "")
    });

    let head = repo.revparse_single("HEAD").unwrap().id().to_string();
    assert_eq!(head.len(), 64, "SHA256 must be 64-chars hex");

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"
                    edition = "2021"

                    [dependencies]
                    dep1 = {{ git = '{}', rev = '{head}' }}
                "#,
                git_dep.url(),
            ),
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Zgit=sha256")
        .masquerade_as_nightly_cargo(&["git=sha256"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] git repository `[ROOTURL]/dep1`
[ERROR] failed to get `dep1` as a dependency of package `foo v0.0.0 ([ROOT]/foo)`

Caused by:
  failed to load source for dependency `dep1`

Caused by:
  unable to update [ROOTURL]/dep1?rev=[..]

Caused by:
  failed to clone into: [ROOT]/home/.cargo/git/db/dep1-[HASH]

Caused by:
  unexpected data at the end of the pack; class=Indexer (15)

"#]])
        .run();
}

#[cargo_test]
fn sha256_and_sha1_deps_coexist() {
    let (sha256_dep, _repo256) = git::new_sha256_repo("dep256", |p| {
        p.file("Cargo.toml", &basic_manifest("dep256", "1.0.0"))
            .file("src/lib.rs", "")
    });

    let sha1_dep = git::new("dep1", |p| {
        p.file("Cargo.toml", &basic_manifest("dep1", "1.0.0"))
            .file("src/lib.rs", "")
    });

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"
                    edition = "2021"

                    [dependencies]
                    dep256 = {{ git = '{}' }}
                    dep1 = {{ git = '{}' }}
                "#,
                sha256_dep.url(),
                sha1_dep.url()
            ),
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Zgit=sha256")
        .masquerade_as_nightly_cargo(&["git=sha256"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] git repository `[ROOTURL]/dep1`
[UPDATING] git repository `[ROOTURL]/dep256`
[ERROR] failed to get `dep256` as a dependency of package `foo v0.0.0 ([ROOT]/foo)`

Caused by:
  failed to load source for dependency `dep256`

Caused by:
  unable to update [ROOTURL]/dep256

Caused by:
  failed to clone into: [ROOT]/home/.cargo/git/db/dep256-[HASH]

Caused by:
  unexpected data at the end of the pack; class=Indexer (15)

"#]])
        .run();
}

#[cargo_test]
fn sha256_lockfile_and_cache_dir() {
    let (git_dep, _repo) = git::new_sha256_repo("dep1", |p| {
        p.file("Cargo.toml", &basic_manifest("dep1", "1.0.0"))
            .file("src/lib.rs", "")
    });

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"
                    edition = "2021"

                    [dependencies]
                    dep1 = {{ git = '{}' }}
                "#,
                git_dep.url()
            ),
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Zgit=sha256")
        .masquerade_as_nightly_cargo(&["git=sha256"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] git repository `[ROOTURL]/dep1`
[ERROR] failed to get `dep1` as a dependency of package `foo v0.0.0 ([ROOT]/foo)`

Caused by:
  failed to load source for dependency `dep1`

Caused by:
  unable to update [ROOTURL]/dep1

Caused by:
  failed to clone into: [ROOT]/home/.cargo/git/db/dep1-[HASH]

Caused by:
  unexpected data at the end of the pack; class=Indexer (15)

"#]])
        .run();
}

#[cargo_test]
fn sha256_update_dep() {
    let (git_dep, _repo) = git::new_sha256_repo("dep1", |p| {
        p.file("Cargo.toml", &basic_manifest("dep1", "1.0.0"))
            .file("src/lib.rs", "")
    });

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"
                    edition = "2021"

                    [dependencies]
                    dep1 = {{ git = '{}' }}
                "#,
                git_dep.url()
            ),
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Zgit=sha256")
        .masquerade_as_nightly_cargo(&["git=sha256"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] git repository `[ROOTURL]/dep1`
[ERROR] failed to get `dep1` as a dependency of package `foo v0.0.0 ([ROOT]/foo)`

Caused by:
  failed to load source for dependency `dep1`

Caused by:
  unable to update [ROOTURL]/dep1

Caused by:
  failed to clone into: [ROOT]/home/.cargo/git/db/dep1-[HASH]

Caused by:
  unexpected data at the end of the pack; class=Indexer (15)

"#]])
        .run();
}

#[cargo_test]
fn sha256_offline_with_cached_db() {
    let (git_dep, _repo) = git::new_sha256_repo("dep1", |p| {
        p.file("Cargo.toml", &basic_manifest("dep1", "1.0.0"))
            .file("src/lib.rs", "")
    });

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"
                    edition = "2021"

                    [dependencies]
                    dep1 = {{ git = '{}' }}
                "#,
                git_dep.url()
            ),
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -Zgit=sha256")
        .masquerade_as_nightly_cargo(&["git=sha256"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] git repository `[ROOTURL]/dep1`
[ERROR] failed to get `dep1` as a dependency of package `foo v0.0.0 ([ROOT]/foo)`

Caused by:
  failed to load source for dependency `dep1`

Caused by:
  unable to update [ROOTURL]/dep1

Caused by:
  failed to clone into: [ROOT]/home/.cargo/git/db/dep1-[HASH]

Caused by:
  unexpected data at the end of the pack; class=Indexer (15)

"#]])
        .run();
}

#[cargo_test]
fn sha256_fetch_with_cli() {
    let (git_dep, _repo) = git::new_sha256_repo("dep1", |p| {
        p.file("Cargo.toml", &basic_manifest("dep1", "1.0.0"))
            .file("src/lib.rs", "")
    });

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"
                    edition = "2021"

                    [dependencies]
                    dep1 = {{ git = '{}' }}
                "#,
                git_dep.url()
            ),
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check -v -Zgit=sha256")
        .env("CARGO_NET_GIT_FETCH_WITH_CLI", "true")
        .masquerade_as_nightly_cargo(&["git=sha256"])
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] git repository `[ROOTURL]/dep1`
[RUNNING] `git fetch --no-tags --verbose --force --update-head-ok '[ROOTURL]/dep1' '+HEAD:refs/remotes/origin/HEAD'`
fatal: mismatched algorithms: client sha1; server sha256
[WARNING] spurious network error (3 tries remaining): process didn't exit successfully: `git fetch --no-tags --verbose --force --update-head-ok '[ROOTURL]/dep1' '+HEAD:refs/remotes/origin/HEAD'` ([EXIT_STATUS]: 128)
fatal: mismatched algorithms: client sha1; server sha256
[WARNING] spurious network error (2 tries remaining): process didn't exit successfully: `git fetch --no-tags --verbose --force --update-head-ok '[ROOTURL]/dep1' '+HEAD:refs/remotes/origin/HEAD'` ([EXIT_STATUS]: 128)
fatal: mismatched algorithms: client sha1; server sha256
[WARNING] spurious network error (1 try remaining): process didn't exit successfully: `git fetch --no-tags --verbose --force --update-head-ok '[ROOTURL]/dep1' '+HEAD:refs/remotes/origin/HEAD'` ([EXIT_STATUS]: 128)
fatal: mismatched algorithms: client sha1; server sha256
[ERROR] failed to get `dep1` as a dependency of package `foo v0.0.0 ([ROOT]/foo)`

Caused by:
  failed to load source for dependency `dep1`

Caused by:
  unable to update [ROOTURL]/dep1

Caused by:
  failed to clone into: [ROOT]/home/.cargo/git/db/dep1-[HASH]

Caused by:
  process didn't exit successfully: `git fetch --no-tags --verbose --force --update-head-ok '[ROOTURL]/dep1' '+HEAD:refs/remotes/origin/HEAD'` ([EXIT_STATUS]: 128)

"#]])
        .run();
}
