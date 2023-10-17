use crate::git_gc::find_index;
use cargo_test_support::registry::Package;
use cargo_test_support::{basic_manifest, git, paths, project};

enum RepoMode {
    Shallow,
    Complete,
}

#[cargo_test]
fn gitoxide_clones_shallow_two_revs_same_deps() {
    perform_two_revs_same_deps(true)
}

fn perform_two_revs_same_deps(shallow: bool) {
    let bar = git::new("meta-dep", |project| {
        project
            .file("Cargo.toml", &basic_manifest("bar", "0.0.0"))
            .file("src/lib.rs", "pub fn bar() -> i32 { 1 }")
    });

    let repo = git2::Repository::open(&bar.root()).unwrap();
    let rev1 = repo.revparse_single("HEAD").unwrap().id();

    // Commit the changes and make sure we trigger a recompile
    bar.change_file("src/lib.rs", "pub fn bar() -> i32 { 2 }");
    git::add(&repo);
    let rev2 = git::commit(&repo);

    let foo = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"
                    version = "0.0.0"
                    authors = []

                    [dependencies.bar]
                    git = '{}'
                    rev = "{}"

                    [dependencies.baz]
                    path = "../baz"
                "#,
                bar.url(),
                rev1
            ),
        )
        .file(
            "src/main.rs",
            r#"
                extern crate bar;
                extern crate baz;

                fn main() {
                    assert_eq!(bar::bar(), 1);
                    assert_eq!(baz::baz(), 2);
                }
            "#,
        )
        .build();

    let _baz = project()
        .at("baz")
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "baz"
                    version = "0.0.0"
                    authors = []

                    [dependencies.bar]
                    git = '{}'
                    rev = "{}"
                "#,
                bar.url(),
                rev2
            ),
        )
        .file(
            "src/lib.rs",
            r#"
                extern crate bar;
                pub fn baz() -> i32 { bar::bar() }
            "#,
        )
        .build();

    let args = if shallow {
        "build -v -Zgitoxide=fetch,shallow-deps"
    } else {
        "build -v"
    };
    foo.cargo(args)
        .masquerade_as_nightly_cargo(&["unstable features must be available for -Z gitoxide"])
        .run();
    assert!(foo.bin("foo").is_file());
    foo.process(&foo.bin("foo")).run();
}

#[cargo_test]
fn two_revs_same_deps() {
    perform_two_revs_same_deps(false)
}

#[cargo_test]
fn gitoxide_clones_registry_with_shallow_protocol_and_follow_up_with_git2_fetch(
) -> anyhow::Result<()> {
    Package::new("bar", "1.0.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [dependencies]
                bar = "1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .build();
    p.cargo("fetch")
        .arg("-Zgitoxide=fetch,shallow-index")
        .masquerade_as_nightly_cargo(&["unstable features must be available for -Z gitoxide"])
        .run();

    let shallow_repo = gix::open_opts(find_index(), gix::open::Options::isolated())?;
    assert_eq!(
        shallow_repo
            .rev_parse_single("origin/HEAD")?
            .ancestors()
            .all()?
            .count(),
        1,
        "shallow clones always start at depth of 1 to minimize download size"
    );
    assert!(shallow_repo.is_shallow());

    Package::new("bar", "1.1.0").publish();
    p.cargo("update")
        .env("__CARGO_USE_GITOXIDE_INSTEAD_OF_GIT2", "0")
        .run();

    let repo = gix::open_opts(
        find_remote_index(RepoMode::Complete),
        gix::open::Options::isolated(),
    )?;
    assert_eq!(
        repo.rev_parse_single("origin/HEAD")?
            .ancestors()
            .all()?
            .count(),
        3,
        "an entirely new repo was cloned which is never shallow"
    );
    assert!(!repo.is_shallow());
    Ok(())
}

#[cargo_test]
fn gitoxide_clones_git_dependency_with_shallow_protocol_and_git2_is_used_for_followup_fetches(
) -> anyhow::Result<()> {
    // Example where an old lockfile with an explicit branch="master" in Cargo.toml.
    Package::new("bar", "1.0.0").publish();
    let (bar, bar_repo) = git::new_repo("bar", |p| {
        p.file("Cargo.toml", &basic_manifest("bar", "1.0.0"))
            .file("src/lib.rs", "")
    });

    bar.change_file("src/lib.rs", "// change");
    git::add(&bar_repo);
    git::commit(&bar_repo);

    {
        let mut walk = bar_repo.revwalk()?;
        walk.push_head()?;
        assert_eq!(
            walk.count(),
            2,
            "original repo has initial commit and change commit"
        );
    }

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"
                    version = "0.1.0"

                    [dependencies]
                    bar = {{ version = "1.0", git = "{}", branch = "master" }}
                "#,
                bar.url()
            ),
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("update")
        .arg("-Zgitoxide=fetch,shallow-deps")
        .masquerade_as_nightly_cargo(&["unstable features must be available for -Z gitoxide"])
        .run();

    let db_clone = gix::open_opts(
        find_bar_db(RepoMode::Shallow),
        gix::open::Options::isolated(),
    )?;
    assert!(db_clone.is_shallow());
    assert_eq!(
        db_clone
            .rev_parse_single("origin/master")?
            .ancestors()
            .all()?
            .count(),
        1,
        "db clones are shallow and have a shortened history"
    );

    let dep_checkout = gix::open_opts(
        find_lexicographically_first_bar_checkout(),
        gix::open::Options::isolated(),
    )?;
    assert!(dep_checkout.is_shallow());
    assert_eq!(
        dep_checkout.head_id()?.ancestors().all()?.count(),
        1,
        "db checkouts are hard-linked clones with the shallow file copied separately."
    );

    bar.change_file("src/lib.rs", "// another change");
    git::add(&bar_repo);
    git::commit(&bar_repo);
    {
        let mut walk = bar_repo.revwalk()?;
        walk.push_head()?;
        assert_eq!(
            walk.count(),
            3,
            "original repo has initial commit and change commit, and another change"
        );
    }

    p.cargo("update")
        .env("__CARGO_USE_GITOXIDE_INSTEAD_OF_GIT2", "0")
        .run();

    let db_clone = gix::open_opts(
        find_bar_db(RepoMode::Complete),
        gix::open::Options::isolated(),
    )?;
    assert_eq!(
        db_clone
            .rev_parse_single("origin/master")?
            .ancestors()
            .all()?
            .count(),
        3,
        "the db clone was re-initialized and has all commits"
    );
    assert!(
        !db_clone.is_shallow(),
        "shallow-ness was removed as git2 does not support it"
    );
    assert_eq!(
        dep_checkout.head_id()?.ancestors().all()?.count(),
        1,
        "the original dep checkout didn't change - there is a new one for each update we get locally"
    );

    let max_history_depth = glob::glob(
        paths::home()
            .join(".cargo/git/checkouts/bar-*/*/.git")
            .to_str()
            .unwrap(),
    )?
        .map(|path| -> anyhow::Result<usize> {
            let dep_checkout = gix::open_opts(path?, gix::open::Options::isolated())?;
            let depth = dep_checkout.head_id()?.ancestors().all()?.count();
            assert_eq!(dep_checkout.is_shallow(), depth == 1, "the first checkout is done with gitoxide and shallow, the second one is git2 non-shallow");
            Ok(depth)
        })
        .map(Result::unwrap)
        .max()
        .expect("two checkout repos");

    assert_eq!(
        max_history_depth, 3,
        "the new checkout sees all commits of the non-shallow DB repository"
    );

    Ok(())
}

#[cargo_test]
fn gitoxide_shallow_clone_followed_by_non_shallow_update() -> anyhow::Result<()> {
    Package::new("bar", "1.0.0").publish();
    let (bar, bar_repo) = git::new_repo("bar", |p| {
        p.file("Cargo.toml", &basic_manifest("bar", "1.0.0"))
            .file("src/lib.rs", "")
    });

    bar.change_file("src/lib.rs", "// change");
    git::add(&bar_repo);
    git::commit(&bar_repo);

    {
        let mut walk = bar_repo.revwalk()?;
        walk.push_head()?;
        assert_eq!(
            walk.count(),
            2,
            "original repo has initial commit and change commit"
        );
    }

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"
                    version = "0.1.0"

                    [dependencies]
                    bar = {{ version = "1.0", git = "{}", branch = "master" }}
                "#,
                bar.url()
            ),
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("update")
        .arg("-Zgitoxide=fetch,shallow-deps")
        .masquerade_as_nightly_cargo(&["unstable features must be available for -Z gitoxide"])
        .run();

    let shallow_db_clone = gix::open_opts(
        find_bar_db(RepoMode::Shallow),
        gix::open::Options::isolated(),
    )?;
    assert!(shallow_db_clone.is_shallow());
    assert_eq!(
        shallow_db_clone
            .rev_parse_single("origin/master")?
            .ancestors()
            .all()?
            .count(),
        1,
        "db clones are shallow and have a shortened history"
    );

    let dep_checkout = gix::open_opts(
        find_lexicographically_first_bar_checkout(),
        gix::open::Options::isolated(),
    )?;
    assert!(dep_checkout.is_shallow());
    assert_eq!(
        dep_checkout.head_id()?.ancestors().all()?.count(),
        1,
        "db checkouts are hard-linked clones with the shallow file copied separately."
    );

    bar.change_file("src/lib.rs", "// another change");
    git::add(&bar_repo);
    git::commit(&bar_repo);
    {
        let mut walk = bar_repo.revwalk()?;
        walk.push_head()?;
        assert_eq!(
            walk.count(),
            3,
            "original repo has initial commit and change commit, and another change"
        );
    }

    p.cargo("update")
        .arg("-Zgitoxide=fetch") // shallow-deps is omitted intentionally
        .masquerade_as_nightly_cargo(&["unstable features must be available for -Z gitoxide"])
        .run();

    let db_clone = gix::open_opts(
        find_bar_db(RepoMode::Complete),
        gix::open::Options::isolated(),
    )?;
    assert_eq!(
        db_clone
            .rev_parse_single("origin/master")?
            .ancestors()
            .all()?
            .count(),
        3,
        "we created an entirely new non-shallow clone"
    );
    assert!(!db_clone.is_shallow());
    assert_eq!(
        dep_checkout.head_id()?.ancestors().all()?.count(),
        1,
        "the original dep checkout didn't change - there is a new one for each update we get locally"
    );

    let max_history_depth = glob::glob(
        paths::home()
            .join(".cargo/git/checkouts/bar-*/*/.git")
            .to_str()
            .unwrap(),
    )?
    .map(|path| -> anyhow::Result<usize> {
        let path = path?;
        let dep_checkout = gix::open_opts(&path, gix::open::Options::isolated())?;
        assert_eq!(
            dep_checkout.is_shallow(),
            path.to_string_lossy().contains("-shallow"),
            "checkouts of shallow db repos are shallow as well"
        );
        let depth = dep_checkout.head_id()?.ancestors().all()?.count();
        Ok(depth)
    })
    .map(Result::unwrap)
    .max()
    .expect("two checkout repos");

    assert_eq!(
        max_history_depth, 3,
        "we see the previous shallow checkout as well as new new unshallow one"
    );

    Ok(())
}

#[cargo_test]
fn gitoxide_clones_registry_with_shallow_protocol_and_follow_up_fetch_maintains_shallowness(
) -> anyhow::Result<()> {
    Package::new("bar", "1.0.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [dependencies]
                bar = "1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .build();
    p.cargo("fetch")
        .arg("-Zgitoxide=fetch,shallow-index")
        .masquerade_as_nightly_cargo(&["unstable features must be available for -Z gitoxide"])
        .run();

    let repo = gix::open_opts(find_index(), gix::open::Options::isolated())?;
    assert_eq!(
        repo.rev_parse_single("origin/HEAD")?
            .ancestors()
            .all()?
            .count(),
        1,
        "shallow clones always start at depth of 1 to minimize download size"
    );
    assert!(repo.is_shallow());

    Package::new("bar", "1.1.0").publish();
    p.cargo("update")
        .arg("-Zgitoxide=fetch,shallow-index") // NOTE: the flag needs to be consistent or else a different index is created
        .masquerade_as_nightly_cargo(&["unstable features must be available for -Z gitoxide"])
        .run();

    assert_eq!(
        repo.rev_parse_single("origin/HEAD")?
            .ancestors()
            .all()?
            .count(),
        1,
        "subsequent shallow fetches wont' fetch what's inbetween, only the single commit that we need while leveraging existing commits"
    );
    assert!(repo.is_shallow());

    Package::new("bar", "1.2.0").publish();
    Package::new("bar", "1.3.0").publish();
    p.cargo("update")
        .arg("-Zgitoxide=fetch,shallow-index")
        .masquerade_as_nightly_cargo(&["unstable features must be available for -Z gitoxide"])
        .run();

    assert_eq!(
        repo.rev_parse_single("origin/HEAD")?
            .ancestors()
            .all()?
            .count(),
        1,
        "shallow boundaries are moved with each fetch to maintain only a single commit of history"
    );
    assert!(repo.is_shallow());

    Ok(())
}

/// If there is shallow *and* non-shallow clones, non-shallow will naturally be returned due to sort order.
#[cargo_test]
fn gitoxide_clones_registry_without_shallow_protocol_and_follow_up_fetch_uses_shallowness(
) -> anyhow::Result<()> {
    Package::new("bar", "1.0.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [dependencies]
                bar = "1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .build();
    p.cargo("fetch")
        .arg("-Zgitoxide=fetch")
        .masquerade_as_nightly_cargo(&["unstable features must be available for -Z gitoxide"])
        .run();

    let repo = gix::open_opts(find_index(), gix::open::Options::isolated())?;
    assert_eq!(
        repo.rev_parse_single("origin/HEAD")?
            .ancestors()
            .all()?
            .count(),
        2,
        "initial commit and the first crate"
    );
    assert!(!repo.is_shallow());

    Package::new("bar", "1.1.0").publish();
    p.cargo("update")
        .arg("-Zgitoxide=fetch,shallow-index")
        .masquerade_as_nightly_cargo(&["unstable features must be available for -Z gitoxide"])
        .run();

    let shallow_repo = gix::open_opts(
        find_remote_index(RepoMode::Shallow),
        gix::open::Options::isolated(),
    )?;
    assert_eq!(
        shallow_repo.rev_parse_single("origin/HEAD")?
            .ancestors()
            .all()?
            .count(),
        1,
        "the follow up clones an entirely new index which is now shallow and which is in its own location"
    );
    assert!(shallow_repo.is_shallow());

    Package::new("bar", "1.2.0").publish();
    Package::new("bar", "1.3.0").publish();
    p.cargo("update")
        .arg("-Zgitoxide=fetch,shallow-index")
        .masquerade_as_nightly_cargo(&["unstable features must be available for -Z gitoxide"])
        .run();

    assert_eq!(
        shallow_repo
            .rev_parse_single("origin/HEAD")?
            .ancestors()
            .all()?
            .count(),
        1,
        "subsequent shallow fetches wont' fetch what's inbetween, only the single commit that we need while leveraging existing commits"
    );
    assert!(shallow_repo.is_shallow());

    p.cargo("update")
        .arg("-Zgitoxide=fetch")
        .masquerade_as_nightly_cargo(&["unstable features must be available for -Z gitoxide"])
        .run();

    assert_eq!(
        repo.rev_parse_single("origin/HEAD")?
            .ancestors()
            .all()?
            .count(),
        5,
        "we can separately fetch the non-shallow index as well and it sees all commits"
    );

    Ok(())
}

#[cargo_test]
fn gitoxide_git_dependencies_switch_from_branch_to_rev() -> anyhow::Result<()> {
    // db exists from previous build, then dependency changes to refer to revision that isn't
    // available in the shallow clone.

    let (bar, bar_repo) = git::new_repo("bar", |p| {
        p.file("Cargo.toml", &basic_manifest("bar", "1.0.0"))
            .file("src/lib.rs", "")
    });

    // this commit would not be available in a shallow clone.
    let first_commit_pre_change = bar_repo.head().unwrap().target().unwrap();

    bar.change_file("src/lib.rs", "// change");
    git::add(&bar_repo);
    git::commit(&bar_repo);
    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"
                    version = "0.1.0"

                    [dependencies]
                    bar = {{ git = "{}", branch = "master" }}
                "#,
                bar.url(),
            ),
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check")
        .arg("-Zgitoxide=fetch,shallow-deps")
        .masquerade_as_nightly_cargo(&["unstable features must be available for -Z gitoxide"])
        .run();

    let db_clone = gix::open_opts(
        find_bar_db(RepoMode::Shallow),
        gix::open::Options::isolated(),
    )?;
    assert!(db_clone.is_shallow());

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"
                    version = "0.1.0"

                    [dependencies]
                    bar = {{ git = "{}", rev = "{}" }}
                "#,
                bar.url(),
                first_commit_pre_change
            ),
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check")
        .arg("-Zgitoxide=fetch,shallow-deps")
        .masquerade_as_nightly_cargo(&["unstable features must be available for -Z gitoxide"])
        .run();

    assert!(
        db_clone.is_shallow(),
        "we maintain shallowness and never unshallow"
    );

    Ok(())
}

#[cargo_test]
fn shallow_deps_work_with_revisions_and_branches_mixed_on_same_dependency() -> anyhow::Result<()> {
    let (bar, bar_repo) = git::new_repo("bar", |p| {
        p.file("Cargo.toml", &basic_manifest("bar", "1.0.0"))
            .file("src/lib.rs", "")
    });

    // this commit would not be available in a shallow clone.
    let first_commit_pre_change = bar_repo.head().unwrap().target().unwrap();

    bar.change_file("src/lib.rs", "// change");
    git::add(&bar_repo);
    git::commit(&bar_repo);

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"
                    version = "0.1.0"

                    [dependencies]
                    bar-renamed = {{ package = "bar", git = "{}", rev = "{}" }}
                    bar = {{ git = "{}", branch = "master" }}
                "#,
                bar.url(),
                first_commit_pre_change,
                bar.url(),
            ),
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("check")
        .arg("-Zgitoxide=fetch,shallow-deps")
        .masquerade_as_nightly_cargo(&["unstable features must be available for -Z gitoxide"])
        .run();

    let db_paths = glob::glob(paths::home().join(".cargo/git/db/bar-*").to_str().unwrap())?
        .map(Result::unwrap)
        .collect::<Vec<_>>();
    assert_eq!(
        db_paths.len(),
        1,
        "only one db checkout source is used per dependency"
    );
    let db_clone = gix::open_opts(&db_paths[0], gix::open::Options::isolated())?;
    assert!(
        db_clone.is_shallow(),
        "the repo is shallow while having all data it needs"
    );

    Ok(())
}

#[cargo_test]
fn gitoxide_clones_registry_with_shallow_protocol_and_aborts_and_updates_again(
) -> anyhow::Result<()> {
    Package::new("bar", "1.0.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"

                [dependencies]
                bar = "1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .build();
    p.cargo("fetch")
        .arg("-Zgitoxide=fetch,shallow-index")
        .masquerade_as_nightly_cargo(&["unstable features must be available for -Z gitoxide"])
        .run();

    let repo = gix::open_opts(find_index(), gix::open::Options::isolated())?;
    assert_eq!(
        repo.rev_parse_single("origin/HEAD")?
            .ancestors()
            .all()?
            .count(),
        1,
        "shallow clones always start at depth of 1 to minimize download size"
    );
    assert!(repo.is_shallow());
    let shallow_lock = repo.shallow_file().with_extension("lock");
    // adding a lock file and deleting the original simulates a left-over clone that was aborted, leaving a lock file
    // in place without ever having moved it to the right location.
    std::fs::write(&shallow_lock, &[])?;
    std::fs::remove_file(repo.shallow_file())?;

    Package::new("bar", "1.1.0").publish();
    p.cargo("update")
        .arg("-Zgitoxide=fetch,shallow-index")
        .masquerade_as_nightly_cargo(&["unstable features must be available for -Z gitoxide"])
        .run();

    assert!(!shallow_lock.is_file(), "the repository was re-initialized");
    assert!(repo.is_shallow());
    assert_eq!(
        repo.rev_parse_single("origin/HEAD")?
            .ancestors()
            .all()?
            .count(),
        1,
        "it's a fresh shallow clone - otherwise it would have 2 commits if the previous shallow clone would still be present"
    );

    Ok(())
}

fn find_lexicographically_first_bar_checkout() -> std::path::PathBuf {
    glob::glob(
        paths::home()
            .join(".cargo/git/checkouts/bar-*/*/.git")
            .to_str()
            .unwrap(),
    )
    .unwrap()
    .next()
    .unwrap()
    .unwrap()
    .to_owned()
}

fn find_remote_index(mode: RepoMode) -> std::path::PathBuf {
    glob::glob(
        paths::home()
            .join(".cargo/registry/index/*")
            .to_str()
            .unwrap(),
    )
    .unwrap()
    .map(Result::unwrap)
    .filter(|p| p.to_string_lossy().ends_with("-shallow") == matches!(mode, RepoMode::Shallow))
    .next()
    .unwrap()
}

/// Find a checkout directory for bar, `shallow` or not.
fn find_bar_db(mode: RepoMode) -> std::path::PathBuf {
    glob::glob(paths::home().join(".cargo/git/db/bar-*").to_str().unwrap())
        .unwrap()
        .map(Result::unwrap)
        .filter(|p| p.to_string_lossy().ends_with("-shallow") == matches!(mode, RepoMode::Shallow))
        .next()
        .unwrap()
        .to_owned()
}
