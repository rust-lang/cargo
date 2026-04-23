//! Tests for SHA256 git repository support (`-Zgit=sha256`).

use cargo_test_support::basic_manifest;
use cargo_test_support::git;
use cargo_test_support::paths;
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
  failed to clone into: [ROOT]/home/.cargo/git/db/dep1-[HASH]-sha256

Caused by:
  SHA256 git repositories require `-Zgit=sha256` to be enabled

"#]])
        .run();
}

#[cargo_test]
fn sha256_gated_with_cached_db() {
    if cargo_uses_gitoxide() {
        eprintln!(
            "gitoxide hasn't yet supported sha256; ignore __CARGO_USE_GITOXIDE_INSTEAD_OF_GIT2"
        );
        return;
    }

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
        .run();

    // Remove lockfile
    // so object format hint must derive from the local git db
    std::fs::remove_file(p.root().join("Cargo.lock")).unwrap();

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
  failed to fetch into: [ROOT]/home/.cargo/git/db/dep1-[HASH]-sha256

Caused by:
  SHA256 git repositories require `-Zgit=sha256` to be enabled

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
        .run();

    // Remove local git db
    // so object format hint must derive from the lockfile's locked revision
    let git_dir = paths::cargo_home().join("git");
    std::fs::remove_dir_all(&git_dir).unwrap();

    p.cargo("check")
        .with_status(101)
        .with_stderr_data(str![[r#"
[UPDATING] git repository `[ROOTURL]/dep1`
[ERROR] failed to get `dep1` as a dependency of package `foo v0.0.0 ([ROOT]/foo)`

Caused by:
  failed to load source for dependency `dep1`

Caused by:
  unable to update [ROOTURL]/dep1#[..]

Caused by:
  failed to clone into: [ROOT]/home/.cargo/git/db/dep1-[HASH]-sha256

Caused by:
  SHA256 git repositories require `-Zgit=sha256` to be enabled

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
  failed to clone into: [ROOT]/home/.cargo/git/db/dep1-[..]-sha256

Caused by:
  gitoxide does not yet support SHA256 repositories

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

    // gitoxide doesn't support SHA256 yet — Cargo bails early
    // with a clear message before attempting the fetch.
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
  failed to clone into: [ROOT]/home/.cargo/git/db/dep1-[HASH]-sha256

Caused by:
  gitoxide does not yet support SHA256 repositories

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
        .with_stderr_data(str![[r#"
[UPDATING] git repository `[ROOTURL]/dep1`
[LOCKING] 1 package to latest compatible version
[CHECKING] dep1 v1.0.0 ([ROOTURL]/dep1#[..])
[CHECKING] foo v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

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

    // Generate a lockfile
    p.cargo("check -Zgit=sha256")
        .masquerade_as_nightly_cargo(&["git=sha256"])
        .run();

    // Verify lockfile contains a 64-char SHA256 revision
    let lock = p.read_lockfile();
    let rev_line = lock
        .lines()
        .find(|l| l.contains("dep1") && l.contains('#'))
        .expect("lockfile must with a rev");
    let rev = rev_line.rsplit('#').next().unwrap().trim_matches('"');
    assert_eq!(rev.len(), 64, "expect SHA256 revision, got {rev:?}");

    // Verify cache db ends with `-sha256`
    let db_paths: Vec<_> = glob::glob(paths::cargo_home().join("git/db/dep1-*").to_str().unwrap())
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(
        db_paths.len(),
        1,
        "expected exactly one db dir: {db_paths:?}"
    );
    let db_dir_name = db_paths[0].file_name().unwrap().to_str().unwrap();
    assert!(
        db_dir_name.ends_with("-sha256"),
        "db dir should end with -sha256, got {db_dir_name:?}"
    );

    // Second build should not re-fetch
    p.cargo("check -Zgit=sha256")
        .masquerade_as_nightly_cargo(&["git=sha256"])
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

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
        .with_stderr_data(str![[r#"
[UPDATING] git repository `[ROOTURL]/dep1`
[LOCKING] 1 package to latest compatible version
[CHECKING] dep1 v1.0.0 ([ROOTURL]/dep1?rev=[..]#[..])
[CHECKING] foo v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}

#[cargo_test]
fn sha256_update_dep() {
    let (git_dep, repo) = git::new_sha256_repo("dep1", |p| {
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
        .run();

    // Modify dep and commit
    git_dep.change_file("src/lib.rs", "// updated");
    git::add(&repo);
    git::commit(&repo);

    // Update and rebuild
    p.cargo("update -Zgit=sha256")
        .masquerade_as_nightly_cargo(&["git=sha256"])
        .with_stderr_data(str![[r#"
[UPDATING] git repository `[ROOTURL]/dep1`
[LOCKING] 1 package to latest compatible version
[UPDATING] dep1 v1.0.0 ([ROOTURL]/dep1#[..]) -> #[..]

"#]])
        .run();

    p.cargo("check -Zgit=sha256")
        .masquerade_as_nightly_cargo(&["git=sha256"])
        .with_stderr_data(str![[r#"
[CHECKING] dep1 v1.0.0 ([ROOTURL]/dep1#[..])
[CHECKING] foo v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

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

    // populates cache first
    p.cargo("check -Zgit=sha256")
        .masquerade_as_nightly_cargo(&["git=sha256"])
        .run();

    // offline works
    p.cargo("check -Zgit=sha256 --frozen")
        .masquerade_as_nightly_cargo(&["git=sha256"])
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

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
        .with_stderr_data(
            str![[r#"
[UPDATING] git repository `[ROOTURL]/dep1`
[UPDATING] git repository `[ROOTURL]/dep256`
[LOCKING] 2 packages to latest compatible versions
[CHECKING] dep256 v1.0.0 ([ROOTURL]/dep256#[..])
[CHECKING] dep1 v1.0.0 ([ROOTURL]/dep1#[..])
[CHECKING] foo v0.0.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]]
            .unordered(),
        )
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
        .with_stderr_data(str![[r#"
[UPDATING] git repository `[ROOTURL]/dep1`
[RUNNING] `git fetch --no-tags --verbose --force --update-head-ok [..][ROOTURL]/dep1[..] [..]+HEAD:refs/remotes/origin/HEAD[..]`
...
[CHECKING] dep1 v1.0.0 ([ROOTURL]/dep1#[..])
[RUNNING] `rustc --crate-name dep1 [..] [ROOT]/home/.cargo/git/checkouts/dep1-[HASH]-sha256/[..]/src/lib.rs [..]`
[CHECKING] foo v0.0.0 ([ROOT]/foo)
[RUNNING] `rustc --crate-name foo [..]`
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
}
