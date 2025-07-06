//! Tests for last-use tracking and auto-gc.
//!
//! Cargo supports an environment variable called `__CARGO_TEST_LAST_USE_NOW`
//! to have cargo pretend that the current time is the given time (in seconds
//! since the unix epoch). This is used throughout these tests to simulate
//! what happens when time passes. The [`days_ago_unix`] and
//! [`months_ago_unix`] functions help with setting this value.

use std::env;
use std::fmt::Write;
use std::path::Path;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::OnceLock;
use std::time::{Duration, SystemTime};

use crate::prelude::*;
use crate::utils::cargo_process;
use cargo::GlobalContext;
use cargo::core::global_cache_tracker::{self, DeferredGlobalLastUse, GlobalCacheTracker};
use cargo::util::cache_lock::CacheLockMode;
use cargo_test_support::compare::assert_e2e;
use cargo_test_support::paths;
use cargo_test_support::registry::{Package, RegistryBuilder};
use cargo_test_support::{
    Execs, Project, basic_manifest, execs, git, process, project, retry, sleep_ms, str,
    thread_wait_timeout,
};
use itertools::Itertools;

use super::config::GlobalContextBuilder;

/// Helper to create a simple `foo` project which depends on a registry
/// dependency called `bar`.
fn basic_foo_bar_project() -> Project {
    Package::new("bar", "1.0.0").publish();
    project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

                [dependencies]
                bar = "1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .build()
}

/// Helper to get the names of files in a directory as strings.
fn get_names(glob: &str) -> Vec<String> {
    let mut names: Vec<_> = glob::glob(paths::home().join(glob).to_str().unwrap())
        .unwrap()
        .map(|p| p.unwrap().file_name().unwrap().to_str().unwrap().to_owned())
        .collect();
    names.sort();
    names
}

fn get_registry_names(which: &str) -> Vec<String> {
    get_names(&format!(".cargo/registry/{which}/*/*"))
}

fn get_index_names() -> Vec<String> {
    get_names(&format!(".cargo/registry/index/*"))
}

fn get_git_db_names() -> Vec<String> {
    get_names(&format!(".cargo/git/db/*"))
}

fn get_git_checkout_names(db_name: &str) -> Vec<String> {
    get_names(&format!(".cargo/git/checkouts/{db_name}/*"))
}

fn days_ago(n: u64) -> SystemTime {
    now() - Duration::from_secs(60 * 60 * 24 * n)
}

fn now() -> SystemTime {
    // This captures the time once to avoid potential time boundaries or
    // inconsistencies affecting a test. For example, on a fast system
    // `days_ago(1)` called twice in a row will return the same answer.
    // However, on a slower system, or if the clock happens to flip over from
    // one second to the next, then it would return different answers. This
    // ensures that it always returns the same answer.
    static START: OnceLock<SystemTime> = OnceLock::new();
    *START.get_or_init(|| SystemTime::now())
}

/// Helper for simulating running cargo in the past. Use with the
/// `__CARGO_TEST_LAST_USE_NOW` environment variable.
fn days_ago_unix(n: u64) -> String {
    days_ago(n)
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        .to_string()
}

/// Helper for simulating running cargo in the past. Use with the
/// `__CARGO_TEST_LAST_USE_NOW` environment variable.
fn months_ago_unix(n: u64) -> String {
    days_ago_unix(n * 30)
}

/// Populates last-use database and the cache files.
///
/// This makes it easier to more accurately specify exact sizes. Creating
/// specific sizes with `Package` is too difficult.
fn populate_cache(
    gctx: &GlobalContext,
    test_crates: &[(&str, u64, u64, u64)],
) -> (PathBuf, PathBuf) {
    let cache_dir = paths::home().join(".cargo/registry/cache/example.com-a6c4a5adcb232b9a");
    let src_dir = paths::home().join(".cargo/registry/src/example.com-a6c4a5adcb232b9a");

    GlobalCacheTracker::db_path(&gctx)
        .into_path_unlocked()
        .rm_rf();

    let _lock = gctx
        .acquire_package_cache_lock(CacheLockMode::MutateExclusive)
        .unwrap();
    let mut tracker = GlobalCacheTracker::new(&gctx).unwrap();
    let mut deferred = DeferredGlobalLastUse::new();

    cache_dir.rm_rf();
    cache_dir.mkdir_p();
    src_dir.rm_rf();
    src_dir.mkdir_p();
    paths::home()
        .join(".cargo/registry/index/example.com-a6c4a5adcb232b9a")
        .mkdir_p();
    let mut create = |name: &str, age, crate_size: u64, src_size: u64| {
        let crate_filename = format!("{name}.crate").into();
        deferred.mark_registry_crate_used_stamp(
            global_cache_tracker::RegistryCrate {
                encoded_registry_name: "example.com-a6c4a5adcb232b9a".into(),
                crate_filename,
                size: crate_size,
            },
            Some(&days_ago(age)),
        );
        deferred.mark_registry_src_used_stamp(
            global_cache_tracker::RegistrySrc {
                encoded_registry_name: "example.com-a6c4a5adcb232b9a".into(),
                package_dir: name.into(),
                size: Some(src_size),
            },
            Some(&days_ago(age)),
        );
        std::fs::write(
            cache_dir.join(crate_filename),
            "x".repeat(crate_size as usize),
        )
        .unwrap();
        let path = src_dir.join(name);
        path.mkdir_p();
        std::fs::write(path.join("data"), "x".repeat(src_size as usize)).unwrap()
    };

    for (name, age, crate_size, src_size) in test_crates {
        create(name, *age, *crate_size, *src_size);
    }
    deferred.save(&mut tracker).unwrap();

    (cache_dir, src_dir)
}

/// Returns an `Execs` that will run the rustup `cargo` proxy from the global
/// system's cargo home directory.
fn rustup_cargo() -> Execs {
    // Modify the PATH to ensure that `cargo` and `rustc` comes from
    // CARGO_HOME. This is necessary because cargo adds the "deps" directory
    // into PATH on Windows, which points to the wrong cargo.
    let real_cargo_home_bin = Path::new(&std::env::var_os("CARGO_HOME").unwrap()).join("bin");
    let mut paths = vec![real_cargo_home_bin];
    paths.extend(env::split_paths(&env::var_os("PATH").unwrap_or_default()));
    let path = env::join_paths(paths).unwrap();
    let mut e = execs().with_process_builder(process("cargo"));
    e.env("PATH", path);
    e
}

#[cargo_test]
fn clean_gc_gated() {
    cargo_process("clean gc")
        .with_status(101)
        .with_stderr_data(str![[r#"
[ERROR] the `cargo clean gc` command is unstable, and only available on the nightly channel of Cargo, but this is the `stable` channel
See https://doc.rust-lang.org/book/appendix-07-nightly-rust.html for more information about Rust release channels.
See https://github.com/rust-lang/cargo/issues/12633 for more information about the `cargo clean gc` command.

"#]]
        )
        .run();
}

#[cargo_test]
fn implies_source() {
    // Checks that when a src, crate, or checkout is marked as used, the
    // corresponding index or git db also gets marked as used.
    let gctx = GlobalContextBuilder::new().build();
    let _lock = gctx
        .acquire_package_cache_lock(CacheLockMode::MutateExclusive)
        .unwrap();
    let mut deferred = DeferredGlobalLastUse::new();
    let mut tracker = GlobalCacheTracker::new(&gctx).unwrap();

    deferred.mark_registry_crate_used(global_cache_tracker::RegistryCrate {
        encoded_registry_name: "example.com-a6c4a5adcb232b9a".into(),
        crate_filename: "regex-1.8.4.crate".into(),
        size: 123,
    });
    deferred.mark_registry_src_used(global_cache_tracker::RegistrySrc {
        encoded_registry_name: "index.crates.io-6f17d22bba15001f".into(),
        package_dir: "rand-0.8.5".into(),
        size: None,
    });
    deferred.mark_git_checkout_used(global_cache_tracker::GitCheckout {
        encoded_git_name: "cargo-e7ff1db891893a9e".into(),
        short_name: "f0a4ee0".into(),
        size: None,
    });
    deferred.save(&mut tracker).unwrap();

    let mut indexes = tracker.registry_index_all().unwrap();
    assert_eq!(indexes.len(), 2);
    indexes.sort_by(|a, b| a.0.encoded_registry_name.cmp(&b.0.encoded_registry_name));
    assert_eq!(
        indexes[0].0.encoded_registry_name,
        "example.com-a6c4a5adcb232b9a"
    );
    assert_eq!(
        indexes[1].0.encoded_registry_name,
        "index.crates.io-6f17d22bba15001f"
    );

    let dbs = tracker.git_db_all().unwrap();
    assert_eq!(dbs.len(), 1);
    assert_eq!(dbs[0].0.encoded_git_name, "cargo-e7ff1db891893a9e");
}

#[cargo_test]
fn auto_gc_defaults() {
    // Checks that the auto-gc deletes old entries, and leaves new ones intact.
    Package::new("old", "1.0.0").publish();
    Package::new("new", "1.0.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

                [dependencies]
                old = "1.0"
                new = "1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .build();
    // Populate the last-use data.
    p.cargo("check")
        .env("__CARGO_TEST_LAST_USE_NOW", months_ago_unix(4))
        .run();
    assert_eq!(get_registry_names("src"), ["new-1.0.0", "old-1.0.0"]);
    assert_eq!(
        get_registry_names("cache"),
        ["new-1.0.0.crate", "old-1.0.0.crate"]
    );

    // Run again with just one package. Make sure the old src gets deleted,
    // but .crate does not.
    p.change_file(
        "Cargo.toml",
        r#"
            [package]
            name = "foo"
            version = "0.1.0"
            edition = "2015"

            [dependencies]
            new = "1.0"
        "#,
    );
    p.cargo("check")
        .env("__CARGO_TEST_LAST_USE_NOW", months_ago_unix(2))
        .run();
    assert_eq!(get_registry_names("src"), ["new-1.0.0"]);
    assert_eq!(
        get_registry_names("cache"),
        ["new-1.0.0.crate", "old-1.0.0.crate"]
    );

    // Run again after the .crate should have aged out.
    p.cargo("check").run();
    assert_eq!(get_registry_names("src"), ["new-1.0.0"]);
    assert_eq!(get_registry_names("cache"), ["new-1.0.0.crate"]);
}

#[cargo_test]
fn auto_gc_config_gated() {
    // gc.auto config options should be ignored without -Zgc
    Package::new("old", "1.0.0").publish();
    Package::new("new", "1.0.0").publish();
    let p = project()
        .file(
            ".cargo/config.toml",
            r#"
                [gc.auto]
                frequency = "always"
                max-src-age = "1 day"
                max-crate-age = "3 days"
                max-index-age = "3 days"
                max-git-co-age = "1 day"
                max-git-db-age = "3 days"
            "#,
        )
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

                [dependencies]
                old = "1.0"
                new = "1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .build();
    // Populate the last-use data.
    p.cargo("check")
        .env("__CARGO_TEST_LAST_USE_NOW", days_ago_unix(4))
        .run();
    assert_eq!(get_registry_names("src"), ["new-1.0.0", "old-1.0.0"]);
    assert_eq!(
        get_registry_names("cache"),
        ["new-1.0.0.crate", "old-1.0.0.crate"]
    );

    // Run again with just one package. Without -Zgc, it should use the
    // defaults and ignore the config. Nothing should get deleted since the
    // defaults are much greater than 4 days.
    p.change_file(
        "Cargo.toml",
        r#"
            [package]
            name = "foo"
            version = "0.1.0"
            edition = "2015"

            [dependencies]
            new = "1.0"
        "#,
    );

    p.cargo("check").run();
    assert_eq!(get_registry_names("src"), ["new-1.0.0", "old-1.0.0"]);
    assert_eq!(
        get_registry_names("cache"),
        ["new-1.0.0.crate", "old-1.0.0.crate"]
    );
}

#[cargo_test]
fn auto_gc_config() {
    // Can configure auto gc settings.
    Package::new("old", "1.0.0").publish();
    Package::new("new", "1.0.0").publish();
    let p = project()
        .file(
            ".cargo/config.toml",
            r#"
                [cache]
                auto-clean-frequency = "always"
                [cache.global-clean]
                max-src-age = "1 day"
                max-crate-age = "3 days"
                max-index-age = "3 days"
                max-git-co-age = "1 day"
                max-git-db-age = "3 days"
            "#,
        )
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

                [dependencies]
                old = "1.0"
                new = "1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .build();
    // Populate the last-use data.
    p.cargo("check")
        .env("__CARGO_TEST_LAST_USE_NOW", days_ago_unix(4))
        .run();
    assert_eq!(get_registry_names("src"), ["new-1.0.0", "old-1.0.0"]);
    assert_eq!(
        get_registry_names("cache"),
        ["new-1.0.0.crate", "old-1.0.0.crate"]
    );

    // Run again with just one package. Make sure the old src gets deleted,
    // but .crate does not.
    p.change_file(
        "Cargo.toml",
        r#"
            [package]
            name = "foo"
            version = "0.1.0"
            edition = "2015"

            [dependencies]
            new = "1.0"
        "#,
    );
    p.cargo("check -Zgc")
        .masquerade_as_nightly_cargo(&["gc"])
        .env("__CARGO_TEST_LAST_USE_NOW", days_ago_unix(2))
        .run();
    assert_eq!(get_registry_names("src"), ["new-1.0.0"]);
    assert_eq!(
        get_registry_names("cache"),
        ["new-1.0.0.crate", "old-1.0.0.crate"]
    );

    // Run again after the .crate should have aged out.
    p.cargo("check -Zgc")
        .masquerade_as_nightly_cargo(&["gc"])
        .run();
    assert_eq!(get_registry_names("src"), ["new-1.0.0"]);
    assert_eq!(get_registry_names("cache"), ["new-1.0.0.crate"]);
}

#[cargo_test]
fn frequency() {
    // cache.auto-clean-frequency settings
    let p = basic_foo_bar_project();
    p.change_file(
        ".cargo/config.toml",
        r#"
            [cache]
            auto-clean-frequency = "never"
        "#,
    );
    // Populate data in the past.
    p.cargo("check")
        .env("__CARGO_TEST_LAST_USE_NOW", months_ago_unix(4))
        .run();
    assert_eq!(get_index_names().len(), 1);
    assert_eq!(get_registry_names("src"), ["bar-1.0.0"]);
    assert_eq!(get_registry_names("cache"), ["bar-1.0.0.crate"]);

    p.change_file("Cargo.toml", &basic_manifest("foo", "0.2.0"));

    // Try after the default expiration time, with "never" it shouldn't gc.
    p.cargo("check").run();
    assert_eq!(get_index_names().len(), 1);
    assert_eq!(get_registry_names("src"), ["bar-1.0.0"]);
    assert_eq!(get_registry_names("cache"), ["bar-1.0.0.crate"]);

    // Try again with a setting that allows it to run.
    p.cargo("check")
        .env("CARGO_CACHE_AUTO_CLEAN_FREQUENCY", "1 day")
        .run();
    assert_eq!(get_index_names().len(), 0);
    assert_eq!(get_registry_names("src").len(), 0);
    assert_eq!(get_registry_names("cache").len(), 0);
}

#[cargo_test]
fn auto_gc_index() {
    // Deletes the index if it hasn't been used in a while.
    let p = basic_foo_bar_project();
    p.cargo("check")
        .env("__CARGO_TEST_LAST_USE_NOW", months_ago_unix(4))
        .run();
    assert_eq!(get_index_names().len(), 1);

    // Make sure it stays within the time frame.
    p.change_file(
        "Cargo.toml",
        r#"
            [package]
            name = "foo"
            version = "0.1.0"
            edition = "2015"
        "#,
    );
    p.cargo("check")
        .env("__CARGO_TEST_LAST_USE_NOW", months_ago_unix(2))
        .run();
    assert_eq!(get_index_names().len(), 1);

    // After it expires, it should be deleted.
    p.cargo("check").run();
    assert_eq!(get_index_names().len(), 0);
}

#[cargo_test]
fn auto_gc_git() {
    // auto-gc should delete git checkouts and dbs.

    // Returns the short git name of a checkout.
    let short_id = |repo: &git2::Repository| -> String {
        let head = repo.revparse_single("HEAD").unwrap();
        let short_id = head.short_id().unwrap();
        short_id.as_str().unwrap().to_owned()
    };

    // Set up a git dependency and fetch it and populate the database,
    // 6 months in the past.
    let (git_project, git_repo) = git::new_repo("bar", |p| {
        p.file("Cargo.toml", &basic_manifest("bar", "1.0.0"))
            .file("src/lib.rs", "")
    });
    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

                [dependencies]
                bar = {{ git = '{}' }}
            "#,
                git_project.url()
            ),
        )
        .file("src/lib.rs", "")
        .build();
    p.cargo("check")
        .env("__CARGO_TEST_LAST_USE_NOW", months_ago_unix(6))
        .run();
    let db_names = get_git_db_names();
    assert_eq!(db_names.len(), 1);
    let first_short_oid = short_id(&git_repo);
    assert_eq!(
        get_git_checkout_names(&db_names[0]),
        [first_short_oid.clone()]
    );

    // Use a new git checkout, should keep both.
    git_project.change_file("src/lib.rs", "// modified");
    git::add(&git_repo);
    git::commit(&git_repo);
    p.cargo("update")
        .env("__CARGO_TEST_LAST_USE_NOW", months_ago_unix(6))
        .run();
    assert_eq!(get_git_db_names().len(), 1);
    let second_short_oid = short_id(&git_repo);
    let mut both = vec![first_short_oid, second_short_oid.clone()];
    both.sort();
    assert_eq!(get_git_checkout_names(&db_names[0]), both);

    // In the future, using the second checkout should delete the first.
    p.cargo("check")
        .env("__CARGO_TEST_LAST_USE_NOW", months_ago_unix(4))
        .run();
    assert_eq!(get_git_db_names().len(), 1);
    assert_eq!(
        get_git_checkout_names(&db_names[0]),
        [second_short_oid.clone()]
    );

    // After three months, the db should get deleted.
    p.change_file("Cargo.toml", &basic_manifest("foo", "0.2.0"));
    p.cargo("check").run();
    assert_eq!(get_git_db_names().len(), 0);
    assert_eq!(get_git_checkout_names(&db_names[0]).len(), 0);
}

#[cargo_test]
fn auto_gc_various_commands() {
    // Checks that auto gc works with a variety of commands.
    //
    // Auto-gc is only run on a subset of commands. Generally it is run on
    // commands that are already doing a lot of work, or heavily involve the
    // use of the registry.
    Package::new("bar", "1.0.0").publish();
    let cmds = ["check", "fetch"];
    for cmd in cmds {
        eprintln!("checking command {cmd}");
        let p = project()
            .file(
                "Cargo.toml",
                r#"
                    [package]
                    name = "foo"
                    version = "0.1.0"
                    edition = "2015"

                    [dependencies]
                    bar = "1.0"
                "#,
            )
            .file("src/lib.rs", "")
            .build();
        // Populate the last-use data.
        p.cargo(cmd)
            .env("__CARGO_TEST_LAST_USE_NOW", months_ago_unix(4))
            .run();
        let gctx = GlobalContextBuilder::new().build();
        let lock = gctx
            .acquire_package_cache_lock(CacheLockMode::MutateExclusive)
            .unwrap();
        let tracker = GlobalCacheTracker::new(&gctx).unwrap();
        let indexes = tracker.registry_index_all().unwrap();
        assert_eq!(indexes.len(), 1);
        let crates = tracker.registry_crate_all().unwrap();
        assert_eq!(crates.len(), 1);
        let srcs = tracker.registry_src_all().unwrap();
        assert_eq!(srcs.len(), 1);
        drop(lock);

        // After everything is aged out, it should all be deleted.
        p.change_file("Cargo.toml", &basic_manifest("foo", "0.2.0"));
        p.cargo(cmd).run();
        let lock = gctx
            .acquire_package_cache_lock(CacheLockMode::MutateExclusive)
            .unwrap();
        let indexes = tracker.registry_index_all().unwrap();
        assert_eq!(indexes.len(), 0);
        let crates = tracker.registry_crate_all().unwrap();
        assert_eq!(crates.len(), 0);
        let srcs = tracker.registry_src_all().unwrap();
        assert_eq!(srcs.len(), 0);
        drop(tracker);
        drop(lock);
        paths::home().join(".cargo/registry").rm_rf();
        GlobalCacheTracker::db_path(&gctx)
            .into_path_unlocked()
            .rm_rf();
    }
}

#[cargo_test]
fn updates_last_use_various_commands() {
    // Checks that last-use tracking is updated by various commands.
    //
    // Not *all* commands update the index tracking, even though they
    // technically involve reading the index. There isn't a convenient place
    // to ensure it gets saved while avoiding saving too often in other
    // commands. For the most part, this should be fine, since these commands
    // usually aren't run without running one of the commands that does save
    // the tracking. Some of the commands are:
    //
    // - login, owner, yank, search
    // - report future-incompatibilities
    // - package --no-verify
    // - fetch --locked
    Package::new("bar", "1.0.0").publish();
    let cmds = [
        // name, expected_crates (0=doesn't download)
        ("check", 1),
        ("fetch", 1),
        ("tree", 1),
        ("generate-lockfile", 0),
        ("update", 0),
        ("metadata", 1),
        ("vendor --respect-source-config", 1),
    ];
    for (cmd, expected_crates) in cmds {
        eprintln!("checking command {cmd}");
        let p = project()
            .file(
                "Cargo.toml",
                r#"
                    [package]
                    name = "foo"
                    version = "0.1.0"
                    edition = "2015"

                    [dependencies]
                    bar = "1.0"
                "#,
            )
            .file("src/lib.rs", "")
            .build();
        // Populate the last-use data.
        p.cargo(cmd).run();
        let gctx = GlobalContextBuilder::new().build();
        let lock = gctx
            .acquire_package_cache_lock(CacheLockMode::MutateExclusive)
            .unwrap();
        let tracker = GlobalCacheTracker::new(&gctx).unwrap();
        let indexes = tracker.registry_index_all().unwrap();
        assert_eq!(indexes.len(), 1);
        let crates = tracker.registry_crate_all().unwrap();
        assert_eq!(crates.len(), expected_crates);
        let srcs = tracker.registry_src_all().unwrap();
        assert_eq!(srcs.len(), expected_crates);
        drop(tracker);
        drop(lock);
        paths::home().join(".cargo/registry").rm_rf();
        GlobalCacheTracker::db_path(&gctx)
            .into_path_unlocked()
            .rm_rf();
    }
}

#[cargo_test]
fn both_git_and_http_index_cleans() {
    // Checks that either the git or http index cache gets cleaned.
    let _crates_io = RegistryBuilder::new().build();
    let _alternative = RegistryBuilder::new().alternative().http_index().build();
    Package::new("from_git", "1.0.0").publish();
    Package::new("from_http", "1.0.0")
        .alternative(true)
        .publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

                [dependencies]
                from_git = "1.0"
                from_http = { version = "1.0", registry = "alternative" }
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("update")
        .env("__CARGO_TEST_LAST_USE_NOW", months_ago_unix(4))
        .run();
    let gctx = GlobalContextBuilder::new().build();
    let lock = gctx
        .acquire_package_cache_lock(CacheLockMode::MutateExclusive)
        .unwrap();
    let tracker = GlobalCacheTracker::new(&gctx).unwrap();
    let indexes = tracker.registry_index_all().unwrap();
    assert_eq!(indexes.len(), 2);
    assert_eq!(get_index_names().len(), 2);
    drop(lock);

    // Running in the future without these indexes should delete them.
    p.change_file("Cargo.toml", &basic_manifest("foo", "0.2.0"));
    p.cargo("clean gc -Zgc")
        .masquerade_as_nightly_cargo(&["gc"])
        .run();
    let lock = gctx
        .acquire_package_cache_lock(CacheLockMode::MutateExclusive)
        .unwrap();
    let indexes = tracker.registry_index_all().unwrap();
    assert_eq!(indexes.len(), 0);
    assert_eq!(get_index_names().len(), 0);
    drop(lock);
}

#[cargo_test]
fn clean_gc_dry_run() {
    // Basic `clean --gc --dry-run` test.
    let p = basic_foo_bar_project();
    // Populate the last-use data.
    p.cargo("fetch")
        .env("__CARGO_TEST_LAST_USE_NOW", months_ago_unix(4))
        .run();

    let registry_root = paths::home().join(".cargo/registry");
    let glob_registry = |name| -> PathBuf {
        let mut paths: Vec<_> = glob::glob(registry_root.join(name).join("*").to_str().unwrap())
            .unwrap()
            .map(|p| p.unwrap())
            .collect();
        assert_eq!(paths.len(), 1);
        paths.pop().unwrap()
    };
    let index = glob_registry("index").ls_r();
    let src = glob_registry("src").ls_r();
    let cache = glob_registry("cache").ls_r();
    let mut expected_files = index
        .iter()
        .chain(src.iter())
        .chain(cache.iter())
        .map(|p| p.to_str().unwrap())
        .join("\n");
    expected_files.push_str("\n");
    let expected_files = snapbox::filter::normalize_paths(&expected_files);
    let expected_files = assert_e2e().redactions().redact(&expected_files);

    p.cargo("clean gc --dry-run -v -Zgc")
        .masquerade_as_nightly_cargo(&["gc"])
        .with_stdout_data(expected_files.as_str().unordered())
        .with_stderr_data(str![[r#"
[SUMMARY] [FILE_NUM] files, [FILE_SIZE]B total
[WARNING] no files deleted due to --dry-run

"#]])
        .run();

    // Again, make sure the information is still tracked.
    p.cargo("clean gc --dry-run -v -Zgc")
        .masquerade_as_nightly_cargo(&["gc"])
        .with_stdout_data(expected_files.as_str().unordered())
        .with_stderr_data(str![[r#"
[SUMMARY] [FILE_NUM] files, [FILE_SIZE]B total
[WARNING] no files deleted due to --dry-run

"#]])
        .run();
}

#[cargo_test]
fn clean_default_gc() {
    // `clean gc` without options should also gc
    let p = basic_foo_bar_project();
    // Populate the last-use data.
    p.cargo("fetch")
        .env("__CARGO_TEST_LAST_USE_NOW", months_ago_unix(4))
        .run();
    p.cargo("clean gc -v -Zgc")
        .masquerade_as_nightly_cargo(&["gc"])
        .with_stderr_data(
            str![[r#"
[REMOVING] [ROOT]/home/.cargo/registry/index/-[HASH]
[REMOVING] [ROOT]/home/.cargo/registry/src/-[HASH]
[REMOVING] [ROOT]/home/.cargo/registry/cache/-[HASH]
[REMOVED] [FILE_NUM] files, [FILE_SIZE]B total

"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test]
fn tracks_sizes() {
    // Checks that sizes are properly tracked in the db.
    Package::new("dep1", "1.0.0")
        .file("src/lib.rs", "")
        .publish();
    Package::new("dep2", "1.0.0")
        .file("src/lib.rs", "")
        .file("data", &"abcdefghijklmnopqrstuvwxyz".repeat(1000))
        .publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

                [dependencies]
                dep1 = "1.0"
                dep2 = "1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .build();
    p.cargo("fetch").run();

    // Check that the crate sizes are the same as on disk.
    let gctx = GlobalContextBuilder::new().build();
    let _lock = gctx
        .acquire_package_cache_lock(CacheLockMode::MutateExclusive)
        .unwrap();
    let tracker = GlobalCacheTracker::new(&gctx).unwrap();
    let mut crates = tracker.registry_crate_all().unwrap();
    crates.sort_by(|a, b| a.0.crate_filename.cmp(&b.0.crate_filename));
    let db_sizes: Vec<_> = crates.iter().map(|c| c.0.size).collect();

    let mut actual: Vec<_> = p
        .glob(paths::home().join(".cargo/registry/cache/*/*"))
        .map(|p| p.unwrap())
        .collect();
    actual.sort();
    let actual_sizes: Vec<_> = actual
        .iter()
        .map(|path| std::fs::metadata(path).unwrap().len())
        .collect();
    assert_eq!(db_sizes, actual_sizes);

    // Also check the src sizes are computed.
    let mut srcs = tracker.registry_src_all().unwrap();
    srcs.sort_by(|a, b| a.0.package_dir.cmp(&b.0.package_dir));
    let db_sizes: Vec<_> = srcs.iter().map(|c| c.0.size.unwrap()).collect();
    let mut actual: Vec<_> = p
        .glob(paths::home().join(".cargo/registry/src/*/*"))
        .map(|p| p.unwrap())
        .collect();
    actual.sort();
    // .cargo-ok is not tracked in the size.
    actual.iter().for_each(|p| p.join(".cargo-ok").rm_rf());
    let actual_sizes: Vec<_> = actual
        .iter()
        .map(|path| cargo_util::du(path, &[]).unwrap())
        .collect();
    assert_eq!(db_sizes, actual_sizes);
    assert!(db_sizes[1] > 26000);
}

#[cargo_test]
fn max_size() {
    // Checks --max-crate-size and --max-src-size with various cleaning thresholds.
    let gctx = GlobalContextBuilder::new().build();

    let test_crates = [
        // name, age, crate_size, src_size
        ("a-1.0.0", 5, 1, 1),
        ("b-1.0.0", 6, 2, 2),
        ("c-1.0.0", 3, 3, 3),
        ("d-1.0.0", 2, 4, 4),
        ("e-1.0.0", 2, 5, 5),
        ("f-1.0.0", 9, 6, 6),
        ("g-1.0.0", 1, 1, 1),
    ];

    // Determine the order things get deleted so they can be verified.
    let mut names_by_timestamp: Vec<_> = test_crates
        .iter()
        .map(|(name, age, _, _)| (days_ago_unix(*age), name))
        .collect();
    names_by_timestamp.sort();
    let names_by_timestamp: Vec<_> = names_by_timestamp
        .into_iter()
        .map(|(_, name)| name)
        .collect();

    // This exercises the different boundary conditions.
    for (clean_size, files) in [
        (22, 0),
        (21, 1),
        (16, 1),
        (15, 2),
        (14, 2),
        (13, 3),
        (12, 4),
        (10, 4),
        (9, 5),
        (6, 5),
        (5, 6),
        (1, 6),
        (0, 7),
    ] {
        let (removed, kept) = names_by_timestamp.split_at(files);
        // --max-crate-size
        let (cache_dir, src_dir) = populate_cache(&gctx, &test_crates);
        let mut stderr = String::new();
        for name in removed {
            writeln!(stderr, "[REMOVING] [..]{name}.crate").unwrap();
        }
        let total_display = if removed.is_empty() {
            ""
        } else {
            ", [FILE_SIZE]B total"
        };
        let files_display = if files == 0 {
            "0 files"
        } else if files == 1 {
            "1 file"
        } else {
            "[FILE_NUM] files"
        };
        writeln!(stderr, "[REMOVED] {files_display}{total_display}").unwrap();
        cargo_process(&format!("clean gc -Zgc -v --max-crate-size={clean_size}"))
            .masquerade_as_nightly_cargo(&["gc"])
            .with_stderr_data(stderr.unordered())
            .run();
        for name in kept {
            assert!(cache_dir.join(format!("{name}.crate")).exists());
        }
        for name in removed {
            assert!(!cache_dir.join(format!("{name}.crate")).exists());
        }

        // --max-src-size
        populate_cache(&gctx, &test_crates);
        let mut stderr = String::new();
        for name in removed {
            writeln!(stderr, "[REMOVING] [..]{name}").unwrap();
        }
        let total_display = if removed.is_empty() {
            ""
        } else {
            ", [FILE_SIZE]B total"
        };
        writeln!(stderr, "[REMOVED] {files_display}{total_display}").unwrap();
        cargo_process(&format!("clean gc -Zgc -v --max-src-size={clean_size}"))
            .masquerade_as_nightly_cargo(&["gc"])
            .with_stderr_data(stderr.unordered())
            .run();
        for name in kept {
            assert!(src_dir.join(name).exists());
        }
        for name in removed {
            assert!(!src_dir.join(name).exists());
        }
    }
}

#[cargo_test]
fn max_size_untracked_crate() {
    // When a .crate file exists from an older version of cargo that did not
    // track sizes, `clean --max-crate-size` should populate the db with the
    // sizes.
    let gctx = GlobalContextBuilder::new().build();
    let cache = paths::home().join(".cargo/registry/cache/example.com-a6c4a5adcb232b9a");
    cache.mkdir_p();
    paths::home()
        .join(".cargo/registry/index/example.com-a6c4a5adcb232b9a")
        .mkdir_p();
    // Create the `.crate files.
    let test_crates = [
        // name, size
        ("a-1.0.0.crate", 1234),
        ("b-1.0.0.crate", 42),
        ("c-1.0.0.crate", 0),
    ];
    for (name, size) in test_crates {
        std::fs::write(cache.join(name), "x".repeat(size as usize)).unwrap()
    }
    // This should scan the directory and populate the db with the size information.
    cargo_process("clean gc -Zgc -v --max-crate-size=100000")
        .masquerade_as_nightly_cargo(&["gc"])
        .with_stderr_data(str![[r#"
[REMOVED] 0 files

"#]])
        .run();
    // Check that it stored the size data.
    let _lock = gctx
        .acquire_package_cache_lock(CacheLockMode::MutateExclusive)
        .unwrap();
    let tracker = GlobalCacheTracker::new(&gctx).unwrap();
    let crates = tracker.registry_crate_all().unwrap();
    let mut actual: Vec<_> = crates
        .iter()
        .map(|(rc, _time)| (rc.crate_filename.as_str(), rc.size))
        .collect();
    actual.sort();
    assert_eq!(test_crates, actual.as_slice());
}

/// Helper to prepare the max-size test.
fn max_size_untracked_prepare() -> (GlobalContext, Project) {
    // First, publish and download a dependency.
    let p = basic_foo_bar_project();
    p.cargo("fetch").run();
    // Pretend it was an older version that did not track last-use.
    let gctx = GlobalContextBuilder::new().build();
    GlobalCacheTracker::db_path(&gctx)
        .into_path_unlocked()
        .rm_rf();
    (gctx, p)
}

/// Helper to verify the max-size test.
fn max_size_untracked_verify(gctx: &GlobalContext) {
    let actual: Vec<_> = glob::glob(
        paths::home()
            .join(".cargo/registry/src/*/*")
            .to_str()
            .unwrap(),
    )
    .unwrap()
    .map(|p| p.unwrap())
    .collect();
    assert_eq!(actual.len(), 1);
    let actual_size = cargo_util::du(&actual[0], &[]).unwrap();
    let lock = gctx
        .acquire_package_cache_lock(CacheLockMode::MutateExclusive)
        .unwrap();
    let tracker = GlobalCacheTracker::new(&gctx).unwrap();
    let srcs = tracker.registry_src_all().unwrap();
    assert_eq!(srcs.len(), 1);
    assert_eq!(srcs[0].0.size, Some(actual_size));
    drop(lock);
}

#[cargo_test]
fn max_size_untracked_src_from_use() {
    // When a src directory exists from an older version of cargo that did not
    // track sizes, doing a build should populate the db with an entry with an
    // unknown size. `clean --max-src-size` should then fix the size.
    let (gctx, p) = max_size_untracked_prepare();

    // Run a command that will update the db with an unknown src size.
    p.cargo("tree").run();
    // Check that it is None.
    let lock = gctx
        .acquire_package_cache_lock(CacheLockMode::MutateExclusive)
        .unwrap();
    let tracker = GlobalCacheTracker::new(&gctx).unwrap();
    let srcs = tracker.registry_src_all().unwrap();
    assert_eq!(srcs.len(), 1);
    assert_eq!(srcs[0].0.size, None);
    drop(lock);

    // Fix the size.
    p.cargo("clean gc -v --max-src-size=10000 -Zgc")
        .masquerade_as_nightly_cargo(&["gc"])
        .with_stderr_data(str![[r#"
[REMOVED] 0 files

"#]])
        .run();
    max_size_untracked_verify(&gctx);
}

#[cargo_test]
fn max_size_untracked_src_from_clean() {
    // When a src directory exists from an older version of cargo that did not
    // track sizes, `clean --max-src-size` should populate the db with the
    // sizes.
    let (gctx, p) = max_size_untracked_prepare();

    // Clean should scan the src and update the db.
    p.cargo("clean gc -v --max-src-size=10000 -Zgc")
        .masquerade_as_nightly_cargo(&["gc"])
        .with_stderr_data(str![[r#"
[REMOVED] 0 files

"#]])
        .run();
    max_size_untracked_verify(&gctx);
}

#[cargo_test]
fn max_download_size() {
    // --max-download-size
    //
    // This creates some sample crates of specific sizes, and then tries
    // deleting at various specific size thresholds that exercise different
    // edge conditions.
    let gctx = GlobalContextBuilder::new().build();

    let test_crates = [
        // name, age, crate_size, src_size
        ("d-1.0.0", 4, 4, 5),
        ("c-1.0.0", 3, 3, 3),
        ("a-1.0.0", 1, 2, 5),
        ("b-1.0.0", 1, 1, 7),
    ];

    for (max_size, num_deleted, files_deleted) in [
        (30, 0, 0),
        (29, 1, 1),
        (24, 2, 2),
        (20, 3, 3),
        (1, 7, 7),
        (0, 8, 8),
    ] {
        populate_cache(&gctx, &test_crates);
        // Determine the order things will be deleted.
        let delete_order: Vec<String> = test_crates
            .iter()
            .flat_map(|(name, _, _, _)| [name.to_string(), format!("{name}.crate")])
            .collect();
        let (removed, _kept) = delete_order.split_at(num_deleted);
        let mut stderr = String::new();
        for name in removed {
            writeln!(stderr, "[REMOVING] [..]{name}").unwrap();
        }
        let files_display = if files_deleted == 0 {
            "0 files"
        } else if files_deleted == 1 {
            "1 file"
        } else {
            "[FILE_NUM] files"
        };
        let total_display = if removed.is_empty() {
            ""
        } else {
            ", [FILE_SIZE]B total"
        };
        writeln!(stderr, "[REMOVED] {files_display}{total_display}",).unwrap();
        cargo_process(&format!("clean gc -Zgc -v --max-download-size={max_size}"))
            .masquerade_as_nightly_cargo(&["gc"])
            .with_stderr_data(stderr.unordered())
            .run();
    }
}

#[cargo_test]
fn package_cache_lock_during_build() {
    // Verifies that a shared lock is held during a build. Resolution and
    // downloads should be OK while that is held, but mutation should block.
    //
    // This works by launching a build with a build script that will pause.
    // Then it performs other cargo commands and verifies their behavior.
    Package::new("bar", "1.0.0").publish();
    let p_foo = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

                [dependencies]
                bar = "1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "build.rs",
            r#"
                fn main() {
                    std::fs::write("blocking", "").unwrap();
                    let path = std::path::Path::new("ready");
                    loop {
                        if path.exists() {
                            break;
                        } else {
                            std::thread::sleep(std::time::Duration::from_millis(100))
                        }
                    }
                }
            "#,
        )
        .build();
    let p_foo2 = project()
        .at("foo2")
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo2"
                version = "0.1.0"
                edition = "2015"

                [dependencies]
                bar = "1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    // Start a build that will pause once the build starts.
    let mut foo_child = p_foo
        .cargo("check")
        .build_command()
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    // Wait for it to enter build script.
    retry(100, || p_foo.root().join("blocking").exists().then_some(()));

    // Start a build with a different target directory. It should not block,
    // even though it gets a download lock, and then a shared lock.
    //
    // Also verify that auto-gc gets disabled.
    p_foo2
        .cargo("check")
        .env("CARGO_CACHE_AUTO_CLEAN_FREQUENCY", "always")
        .env("CARGO_LOG", "gc=debug")
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
   [..]s DEBUG gc: unable to acquire mutate lock, auto gc disabled
[CHECKING] bar v1.0.0
[CHECKING] foo2 v0.1.0 ([ROOT]/foo2)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();

    // Ensure that the first build really blocked.
    assert!(matches!(foo_child.try_wait(), Ok(None)));

    // Cleaning while a command is running should block.
    let mut clean_cmd = p_foo2
        .cargo("clean gc --max-download-size=0 -Zgc")
        .masquerade_as_nightly_cargo(&["gc"])
        .build_command();
    clean_cmd.stderr(Stdio::piped());
    let mut clean_child = clean_cmd.spawn().unwrap();

    // Give the clean command a chance to finish (it shouldn't).
    sleep_ms(500);
    // They should both still be running.
    assert!(matches!(foo_child.try_wait(), Ok(None)));
    assert!(matches!(clean_child.try_wait(), Ok(None)));

    // Let the original build finish.
    p_foo.change_file("ready", "");

    // Wait for clean to finish.
    let thread = std::thread::spawn(|| clean_child.wait_with_output().unwrap());
    let output = thread_wait_timeout(100, thread);
    assert!(output.status.success());
    // Validate the output of the clean.
    execs()
        .with_stderr_data(str![[r#"
[BLOCKING] waiting for file lock on package cache mutation
[REMOVED] [FILE_NUM] files, [FILE_SIZE]B total

"#]])
        .run_output(&output);
}

#[cargo_test]
fn read_only_locking_auto_gc() {
    // Tests the behavior for auto-gc on a read-only directory.
    let p = basic_foo_bar_project();
    // Populate cache.
    p.cargo("fetch").run();
    let cargo_home = paths::home().join(".cargo");
    let mut perms = std::fs::metadata(&cargo_home).unwrap().permissions();
    // Test when it can't update auto-gc db.
    perms.set_readonly(true);
    std::fs::set_permissions(&cargo_home, perms.clone()).unwrap();
    p.cargo("check -Zgc")
        .masquerade_as_nightly_cargo(&["gc"])
        .with_stderr_data(str![[r#"
[CHECKING] bar v1.0.0
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    // Try again without the last-use existing (such as if the cache was
    // populated by an older version of cargo).
    perms.set_readonly(false);
    std::fs::set_permissions(&cargo_home, perms.clone()).unwrap();
    let gctx = GlobalContextBuilder::new().build();
    GlobalCacheTracker::db_path(&gctx)
        .into_path_unlocked()
        .rm_rf();
    perms.set_readonly(true);
    std::fs::set_permissions(&cargo_home, perms.clone()).unwrap();
    p.cargo("check -Zgc")
        .masquerade_as_nightly_cargo(&["gc"])
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    perms.set_readonly(false);
    std::fs::set_permissions(&cargo_home, perms).unwrap();
}

#[cargo_test]
fn delete_index_also_deletes_crates() {
    // Checks that when an index is delete that src and cache directories also get deleted.
    let p = basic_foo_bar_project();
    p.cargo("fetch")
        .env("__CARGO_TEST_LAST_USE_NOW", months_ago_unix(4))
        .run();

    assert_eq!(get_registry_names("src"), ["bar-1.0.0"]);
    assert_eq!(get_registry_names("cache"), ["bar-1.0.0.crate"]);

    p.cargo("clean gc")
        .arg("--max-index-age=0 days")
        .arg("-Zgc")
        .masquerade_as_nightly_cargo(&["gc"])
        .with_stderr_data(str![[r#"
[REMOVED] [FILE_NUM] files, [FILE_SIZE]B total

"#]])
        .run();

    assert_eq!(get_registry_names("src").len(), 0);
    assert_eq!(get_registry_names("cache").len(), 0);
}

#[cargo_test]
fn clean_syncs_missing_files() {
    // When files go missing in the cache, clean operations that need to track
    // the size should also remove them from the database.
    Package::new("bar", "1.0.0").publish();
    Package::new("baz", "1.0.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

                [dependencies]
                bar = "1.0"
                baz = "1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .build();
    p.cargo("fetch").run();

    // Verify things are tracked.
    let gctx = GlobalContextBuilder::new().build();
    let lock = gctx
        .acquire_package_cache_lock(CacheLockMode::MutateExclusive)
        .unwrap();
    let tracker = GlobalCacheTracker::new(&gctx).unwrap();
    let crates = tracker.registry_crate_all().unwrap();
    assert_eq!(crates.len(), 2);
    let srcs = tracker.registry_src_all().unwrap();
    assert_eq!(srcs.len(), 2);
    drop(lock);

    // Remove the files.
    for pattern in [
        ".cargo/registry/cache/*/bar-1.0.0.crate",
        ".cargo/registry/src/*/bar-1.0.0",
    ] {
        p.glob(paths::home().join(pattern))
            .map(|p| p.unwrap())
            .next()
            .unwrap()
            .rm_rf();
    }

    // Clean should update the db.
    p.cargo("clean gc -v --max-download-size=1GB -Zgc")
        .masquerade_as_nightly_cargo(&["gc"])
        .with_stderr_data(str![[r#"
[REMOVED] 0 files

"#]])
        .run();

    // Verify
    let crates = tracker.registry_crate_all().unwrap();
    assert_eq!(crates.len(), 1);
    let srcs = tracker.registry_src_all().unwrap();
    assert_eq!(srcs.len(), 1);
}

#[cargo_test]
fn offline_doesnt_auto_gc() {
    // When running offline, auto-gc shouldn't run.
    let p = basic_foo_bar_project();
    p.cargo("fetch")
        .env("__CARGO_TEST_LAST_USE_NOW", months_ago_unix(4))
        .run();
    // Remove the dependency.
    p.change_file("Cargo.toml", &basic_manifest("foo", "0.1.0"));
    // Run offline, make sure it doesn't delete anything
    p.cargo("check --offline")
        .with_stderr_data(str![[r#"
[CHECKING] foo v0.1.0 ([ROOT]/foo)
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    assert_eq!(get_registry_names("src"), ["bar-1.0.0"]);
    assert_eq!(get_registry_names("cache"), ["bar-1.0.0.crate"]);
    // Run online, make sure auto-gc runs.
    p.cargo("check")
        .with_stderr_data(str![[r#"
[FINISHED] `dev` profile [unoptimized + debuginfo] target(s) in [ELAPSED]s

"#]])
        .run();
    assert_eq!(get_registry_names("src"), &[] as &[String]);
    assert_eq!(get_registry_names("cache"), &[] as &[String]);
}

#[cargo_test]
fn can_handle_future_schema() -> anyhow::Result<()> {
    // It should work when a future version of cargo has made schema changes
    // to the database.
    let p = basic_foo_bar_project();
    p.cargo("fetch")
        .env("__CARGO_TEST_LAST_USE_NOW", months_ago_unix(4))
        .run();
    // Modify the schema to pretend this is done by a future version of cargo.
    let gctx = GlobalContextBuilder::new().build();
    let db_path = GlobalCacheTracker::db_path(&gctx).into_path_unlocked();
    let conn = rusqlite::Connection::open(&db_path)?;
    let user_version: u32 =
        conn.query_row("SELECT user_version FROM pragma_user_version", [], |row| {
            row.get(0)
        })?;
    conn.execute("ALTER TABLE global_data ADD COLUMN foo DEFAULT 123", [])?;
    conn.pragma_update(None, "user_version", &(user_version + 1))?;
    drop(conn);
    // Verify it doesn't blow up.
    p.cargo("clean gc --max-download-size=0 -Zgc")
        .masquerade_as_nightly_cargo(&["gc"])
        .with_stderr_data(str![[r#"
[REMOVED] [FILE_NUM] files, [FILE_SIZE]B total

"#]])
        .run();
    Ok(())
}

#[cargo_test]
fn clean_max_git_age() {
    // --max-git-*-age flags
    let (git_a, git_a_repo) = git::new_repo("git_a", |p| {
        p.file("Cargo.toml", &basic_manifest("git_a", "1.0.0"))
            .file("src/lib.rs", "")
    });
    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

                [dependencies]
                git_a = {{ git = '{}' }}
            "#,
                git_a.url()
            ),
        )
        .file("src/lib.rs", "")
        .build();
    // Populate last-use tracking.
    p.cargo("fetch")
        .env("__CARGO_TEST_LAST_USE_NOW", days_ago_unix(4))
        .run();
    // Update git_a to create a separate checkout.
    git_a.change_file("src/lib.rs", "// test");
    git::add(&git_a_repo);
    git::commit(&git_a_repo);
    // Update last-use tracking, where the first git checkout will stay "old".
    p.cargo("update -p git_a")
        .env("__CARGO_TEST_LAST_USE_NOW", days_ago_unix(2))
        .with_stderr_data(str![[r#"
[UPDATING] git repository `[ROOTURL]/git_a`
[LOCKING] 1 package to latest compatible version
[UPDATING] git_a v1.0.0 ([ROOTURL]/git_a#[..]) -> #[..]

"#]])
        .run();

    let db_names = get_git_db_names();
    assert_eq!(db_names.len(), 1);
    let db_name = &db_names[0];
    let co_names = get_git_checkout_names(&db_name);
    assert_eq!(co_names.len(), 2);

    // Delete the first checkout
    p.cargo("clean gc -v -Zgc")
        .arg("--max-git-co-age=3 days")
        .masquerade_as_nightly_cargo(&["gc"])
        .with_stderr_data(str![[r#"
[REMOVING] [ROOT]/home/.cargo/git/checkouts/git_a-[HASH]/[..]
[REMOVED] [FILE_NUM] files, [FILE_SIZE]B total

"#]])
        .run();

    let db_names = get_git_db_names();
    assert_eq!(db_names.len(), 1);
    let co_names = get_git_checkout_names(&db_name);
    assert_eq!(co_names.len(), 1);

    // delete the second checkout
    p.cargo("clean gc -v -Zgc")
        .arg("--max-git-co-age=0 days")
        .masquerade_as_nightly_cargo(&["gc"])
        .with_stderr_data(str![[r#"
[REMOVING] [ROOT]/home/.cargo/git/checkouts/git_a-[HASH]/[..]
[REMOVED] [FILE_NUM] files, [FILE_SIZE]B total

"#]])
        .run();

    let db_names = get_git_db_names();
    assert_eq!(db_names.len(), 1);
    let co_names = get_git_checkout_names(&db_name);
    assert_eq!(co_names.len(), 0);

    // delete the db
    p.cargo("clean gc -v -Zgc")
        .arg("--max-git-db-age=1 days")
        .masquerade_as_nightly_cargo(&["gc"])
        .with_stderr_data(str![[r#"
[REMOVING] [ROOT]/home/.cargo/git/db/git_a-[HASH]
[REMOVING] [ROOT]/home/.cargo/git/checkouts/git_a-[HASH]
[REMOVED] [FILE_NUM] files, [FILE_SIZE]B total

"#]])
        .run();

    let db_names = get_git_db_names();
    assert_eq!(db_names.len(), 0);
    let co_names = get_git_checkout_names(&db_name);
    assert_eq!(co_names.len(), 0);
}

#[cargo_test]
fn clean_max_src_crate_age() {
    // --max-src-age and --max-crate-age flags
    let p = basic_foo_bar_project();
    // Populate last-use tracking.
    p.cargo("fetch")
        .env("__CARGO_TEST_LAST_USE_NOW", days_ago_unix(4))
        .run();
    // Update bar to create a separate copy with a different timestamp.
    Package::new("bar", "1.0.1").publish();
    p.cargo("update -p bar")
        .env("__CARGO_TEST_LAST_USE_NOW", days_ago_unix(2))
        .with_stderr_data(str![[r#"
[UPDATING] `dummy-registry` index
[LOCKING] 1 package to latest compatible version
[UPDATING] bar v1.0.0 -> v1.0.1

"#]])
        .run();
    p.cargo("fetch")
        .env("__CARGO_TEST_LAST_USE_NOW", days_ago_unix(2))
        .with_stderr_data(str![[r#"
[DOWNLOADING] crates ...
[DOWNLOADED] bar v1.0.1 (registry `dummy-registry`)

"#]])
        .run();

    assert_eq!(get_registry_names("src"), ["bar-1.0.0", "bar-1.0.1"]);
    assert_eq!(
        get_registry_names("cache"),
        ["bar-1.0.0.crate", "bar-1.0.1.crate"]
    );

    // Delete the old src.
    p.cargo("clean gc -v -Zgc")
        .arg("--max-src-age=3 days")
        .masquerade_as_nightly_cargo(&["gc"])
        .with_stderr_data(str![[r#"
[REMOVING] [ROOT]/home/.cargo/registry/src/-[HASH]/bar-1.0.0
[REMOVED] [FILE_NUM] files, [FILE_SIZE]B total

"#]])
        .run();

    // delete the second src
    p.cargo("clean gc -v -Zgc")
        .arg("--max-src-age=0 days")
        .masquerade_as_nightly_cargo(&["gc"])
        .with_stderr_data(str![[r#"
[REMOVING] [ROOT]/home/.cargo/registry/src/-[HASH]/bar-1.0.1
[REMOVED] [FILE_NUM] files, [FILE_SIZE]B total

"#]])
        .run();

    // delete the old crate
    p.cargo("clean gc -v -Zgc")
        .arg("--max-crate-age=3 days")
        .masquerade_as_nightly_cargo(&["gc"])
        .with_stderr_data(str![[r#"
[REMOVING] [ROOT]/home/.cargo/registry/cache/-[HASH]/bar-1.0.0.crate
[REMOVED] 1 file, [FILE_SIZE]B total

"#]])
        .run();

    // delete the seecond crate
    p.cargo("clean gc -v -Zgc")
        .arg("--max-crate-age=0 days")
        .masquerade_as_nightly_cargo(&["gc"])
        .with_stderr_data(str![[r#"
[REMOVING] [ROOT]/home/.cargo/registry/cache/-[HASH]/bar-1.0.1.crate
[REMOVED] 1 file, [FILE_SIZE]B total

"#]])
        .run();
}

#[cargo_test]
fn clean_max_git_size() {
    // clean --max-git-size
    //
    // Creates two checkouts. The sets a size threshold to delete one. And
    // then with 0 max size to delete everything.
    let (git_project, git_repo) = git::new_repo("bar", |p| {
        p.file("Cargo.toml", &basic_manifest("bar", "1.0.0"))
            .file("src/lib.rs", "")
    });
    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

                [dependencies]
                bar = {{ git = '{}' }}
            "#,
                git_project.url()
            ),
        )
        .file("src/lib.rs", "")
        .build();
    // Fetch and populate db.
    p.cargo("fetch")
        .env("__CARGO_TEST_LAST_USE_NOW", days_ago_unix(3))
        .run();

    // Figure out the name of the first checkout.
    let git_root = paths::home().join(".cargo/git");
    let db_names = get_git_db_names();
    assert_eq!(db_names.len(), 1);
    let db_name = &db_names[0];
    let co_names = get_git_checkout_names(&db_name);
    assert_eq!(co_names.len(), 1);
    let first_co_name = &co_names[0];

    // Make an update and create a new checkout.
    git_project.change_file("src/lib.rs", "// modified");
    git::add(&git_repo);
    git::commit(&git_repo);
    p.cargo("update")
        // Use a different time so that the first checkout timestamp is less
        // than the second.
        .env("__CARGO_TEST_LAST_USE_NOW", days_ago_unix(2))
        .run();

    // Figure out the threshold to use.
    let mut co_names = get_git_checkout_names(&db_name);
    assert_eq!(co_names.len(), 2);
    co_names.retain(|name| name != first_co_name);
    assert_eq!(co_names.len(), 1);
    let second_co_name = &co_names[0];
    let second_co_path = git_root
        .join("checkouts")
        .join(db_name)
        .join(second_co_name);
    let second_co_size = cargo_util::du(&second_co_path, &["!.git"]).unwrap();

    let db_size = cargo_util::du(&git_root.join("db").join(db_name), &[]).unwrap();

    let threshold = db_size + second_co_size;

    p.cargo(&format!("clean gc --max-git-size={threshold} -Zgc -v"))
        .masquerade_as_nightly_cargo(&["gc"])
        .with_stderr_data(&format!(
            "\
[REMOVING] [ROOT]/home/.cargo/git/checkouts/bar-[HASH]/{first_co_name}
[REMOVED] [..]
"
        ))
        .run();

    // And then try cleaning everything.
    p.cargo("clean gc --max-git-size=0 -Zgc -v")
        .masquerade_as_nightly_cargo(&["gc"])
        .with_stderr_data(
            format!(
                "\
[REMOVING] [ROOT]/home/.cargo/git/checkouts/bar-[HASH]/{second_co_name}
[REMOVING] [ROOT]/home/.cargo/git/db/bar-[HASH]
[REMOVED] [..]
"
            )
            .unordered(),
        )
        .run();
}

// Helper for setting up fake git sizes for git size cleaning.
fn setup_fake_git_sizes(db_name: &str, db_size: usize, co_sizes: &[usize]) {
    let base_git = paths::home().join(".cargo/git");
    let db_path = base_git.join("db").join(db_name);
    db_path.mkdir_p();
    std::fs::write(db_path.join("test"), "x".repeat(db_size)).unwrap();
    let base_co = base_git.join("checkouts").join(db_name);
    for (i, size) in co_sizes.iter().enumerate() {
        let co_name = format!("co{i}");
        let co_path = base_co.join(co_name);
        co_path.mkdir_p();
        std::fs::write(co_path.join("test"), "x".repeat(*size)).unwrap();
    }
}

#[cargo_test]
fn clean_max_git_size_untracked() {
    // If there are git directories that aren't tracked in the database,
    // `--max-git-size` should pick it up.
    //
    // The db_name of "example" depends on the sorting order of the names ("e"
    // should be after "c"), so that the db comes after the checkouts.
    setup_fake_git_sizes("example", 5000, &[1000, 2000]);
    cargo_process(&format!("clean gc -Zgc -v --max-git-size=7000"))
        .masquerade_as_nightly_cargo(&["gc"])
        .with_stderr_data(str![[r#"
[REMOVING] [ROOT]/home/.cargo/git/checkouts/example/co0
[REMOVED] 1 file, [FILE_SIZE]B total

"#]])
        .run();
    cargo_process(&format!("clean gc -Zgc -v --max-git-size=5000"))
        .masquerade_as_nightly_cargo(&["gc"])
        .with_stderr_data(str![[r#"
[REMOVING] [ROOT]/home/.cargo/git/checkouts/example/co1
[REMOVED] 1 file, [FILE_SIZE]B total

"#]])
        .run();
    cargo_process(&format!("clean gc -Zgc -v --max-git-size=0"))
        .masquerade_as_nightly_cargo(&["gc"])
        .with_stderr_data(str![[r#"
[REMOVING] [ROOT]/home/.cargo/git/db/example
[REMOVED] 1 file, [FILE_SIZE]B total

"#]])
        .run();
}

#[cargo_test]
fn clean_max_git_size_deletes_co_from_db() {
    // In the scenario where it thinks it needs to delete the db, it should
    // also delete all the checkouts.
    //
    // The db_name of "abc" depends on the sorting order of the names ("a"
    // should be before "c"), so that the db comes before the checkouts.
    setup_fake_git_sizes("abc", 5000, &[1000, 2000]);
    // This deletes everything because it tries to delete the db, which then
    // deletes all checkouts.
    cargo_process(&format!("clean gc -Zgc -v --max-git-size=3000"))
        .masquerade_as_nightly_cargo(&["gc"])
        .with_stderr_data(str![[r#"
[REMOVING] [ROOT]/home/.cargo/git/db/abc
[REMOVING] [ROOT]/home/.cargo/git/checkouts/abc/co1
[REMOVING] [ROOT]/home/.cargo/git/checkouts/abc/co0
[REMOVED] [FILE_NUM] files, [FILE_SIZE]B total

"#]])
        .run();
}

#[cargo_test]
fn handles_missing_index() {
    // Checks behavior when index is missing.
    let p = basic_foo_bar_project();
    p.cargo("fetch").run();
    paths::home().join(".cargo/registry/index").rm_rf();
    cargo_process("clean gc -v --max-download-size=0 -Zgc")
        .masquerade_as_nightly_cargo(&["gc"])
        .with_stderr_data(
            str![[r#"
[REMOVING] [ROOT]/home/.cargo/registry/cache/-[HASH]
[REMOVING] [ROOT]/home/.cargo/registry/src/-[HASH]
[REMOVED] [FILE_NUM] files, [FILE_SIZE]B total

"#]]
            .unordered(),
        )
        .run();
}

#[cargo_test]
fn handles_missing_git_db() {
    // Checks behavior when git db is missing.
    let git_project = git::new("bar", |p| {
        p.file("Cargo.toml", &basic_manifest("bar", "1.0.0"))
            .file("src/lib.rs", "")
    });
    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

                [dependencies]
                bar = {{ git = '{}' }}
            "#,
                git_project.url()
            ),
        )
        .file("src/lib.rs", "")
        .build();
    p.cargo("fetch").run();
    paths::home().join(".cargo/git/db").rm_rf();
    cargo_process("clean gc -v --max-git-size=0 -Zgc")
        .masquerade_as_nightly_cargo(&["gc"])
        .with_stderr_data(str![[r#"
[REMOVING] [ROOT]/home/.cargo/git/checkouts/bar-[HASH]
[REMOVED] [FILE_NUM] files, [FILE_SIZE]B total

"#]])
        .run();
}

#[cargo_test]
fn clean_gc_quiet_is_quiet() {
    // Checks that --quiet works with `cargo clean gc`, since there was a
    // subtle issue with how the flag is defined as a global flag.
    let p = basic_foo_bar_project();
    p.cargo("fetch")
        .env("__CARGO_TEST_LAST_USE_NOW", months_ago_unix(4))
        .run();
    p.cargo("clean gc --quiet -Zgc --dry-run")
        .masquerade_as_nightly_cargo(&["gc"])
        .with_stdout_data("")
        .with_stderr_data("")
        .run();
    // Verify exact same command without -q would actually display something.
    p.cargo("clean gc -Zgc --dry-run")
        .masquerade_as_nightly_cargo(&["gc"])
        .with_stdout_data("")
        .with_stderr_data(str![[r#"
[SUMMARY] [FILE_NUM] files, [FILE_SIZE]B total
[WARNING] no files deleted due to --dry-run

"#]])
        .run();
}

#[cargo_test(requires_rustup_stable)]
fn compatible_with_older_cargo() {
    // Ensures that db stays backwards compatible across versions.

    // T-4 months: Current version, build the database.
    Package::new("old", "1.0.0").publish();
    Package::new("middle", "1.0.0").publish();
    Package::new("new", "1.0.0").publish();
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.1.0"
                edition = "2015"

                [dependencies]
                old = "1.0"
                middle = "1.0"
                new = "1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .build();
    // Populate the last-use data.
    p.cargo("check")
        .env("__CARGO_TEST_LAST_USE_NOW", months_ago_unix(4))
        .run();
    assert_eq!(
        get_registry_names("src"),
        ["middle-1.0.0", "new-1.0.0", "old-1.0.0"]
    );
    assert_eq!(
        get_registry_names("cache"),
        ["middle-1.0.0.crate", "new-1.0.0.crate", "old-1.0.0.crate"]
    );

    // T-2 months: Stable version, make sure it reads and deletes old src.
    p.change_file(
        "Cargo.toml",
        r#"
            [package]
            name = "foo"
            version = "0.1.0"
            edition = "2015"

            [dependencies]
            new = "1.0"
            middle = "1.0"
        "#,
    );
    rustup_cargo()
        .args(&["+stable", "check"])
        .cwd(p.root())
        .env("__CARGO_TEST_LAST_USE_NOW", months_ago_unix(2))
        .run();
    assert_eq!(get_registry_names("src"), ["middle-1.0.0", "new-1.0.0"]);
    assert_eq!(
        get_registry_names("cache"),
        ["middle-1.0.0.crate", "new-1.0.0.crate", "old-1.0.0.crate"]
    );

    // T-0 months: Current version, make sure it can read data from stable,
    // deletes old crate and middle src.
    p.change_file(
        "Cargo.toml",
        r#"
            [package]
            name = "foo"
            version = "0.1.0"
            edition = "2015"

            [dependencies]
            new = "1.0"
        "#,
    );
    p.cargo("check").run();
    assert_eq!(get_registry_names("src"), ["new-1.0.0"]);
    assert_eq!(
        get_registry_names("cache"),
        ["middle-1.0.0.crate", "new-1.0.0.crate"]
    );
}

#[cargo_test(requires_rustup_stable)]
fn forward_compatible() {
    // Checks that db created in an older version can be read in a newer version.
    Package::new("bar", "1.0.0").publish();
    let git_project = git::new("from_git", |p| {
        p.file("Cargo.toml", &basic_manifest("from_git", "1.0.0"))
            .file("src/lib.rs", "")
    });

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"

                    [dependencies]
                    bar = "1.0.0"
                    from_git = {{ git = '{}' }}
                "#,
                git_project.url()
            ),
        )
        .file("src/lib.rs", "")
        .build();

    rustup_cargo()
        .args(&["+stable", "check"])
        .cwd(p.root())
        .run();

    let config = GlobalContextBuilder::new().build();
    let lock = config
        .acquire_package_cache_lock(CacheLockMode::MutateExclusive)
        .unwrap();
    let tracker = GlobalCacheTracker::new(&config).unwrap();
    // Don't want to check the actual index name here, since although the
    // names are semi-stable, they might change over long periods of time.
    let indexes = tracker.registry_index_all().unwrap();
    assert_eq!(indexes.len(), 1);
    let crates = tracker.registry_crate_all().unwrap();
    let names: Vec<_> = crates
        .iter()
        .map(|(krate, _timestamp)| krate.crate_filename)
        .collect();
    assert_eq!(names, &["bar-1.0.0.crate"]);
    let srcs = tracker.registry_src_all().unwrap();
    let names: Vec<_> = srcs
        .iter()
        .map(|(src, _timestamp)| src.package_dir)
        .collect();
    assert_eq!(names, &["bar-1.0.0"]);
    let dbs: Vec<_> = tracker.git_db_all().unwrap();
    assert_eq!(dbs.len(), 1);
    let cos: Vec<_> = tracker.git_checkout_all().unwrap();
    assert_eq!(cos.len(), 1);
    drop(lock);
}

#[cargo_test]
fn resilient_to_unexpected_files() {
    // Tests that it doesn't choke on unexpected files.
    Package::new("bar", "1.0.0").publish();
    let git_project = git::new("from_git", |p| {
        p.file("Cargo.toml", &basic_manifest("from_git", "1.0.0"))
            .file("src/lib.rs", "")
    });

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [package]
                    name = "foo"

                    [dependencies]
                    bar = "1.0.0"
                    from_git = {{ git = '{}' }}
                "#,
                git_project.url()
            ),
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("fetch -Zgc")
        .masquerade_as_nightly_cargo(&["gc"])
        .env("__CARGO_TEST_LAST_USE_NOW", months_ago_unix(4))
        .run();

    let root = paths::home().join(".cargo");
    std::fs::write(root.join("registry/index/foo"), "").unwrap();
    std::fs::write(root.join("registry/cache/foo"), "").unwrap();
    std::fs::write(root.join("registry/src/foo"), "").unwrap();
    std::fs::write(root.join("git/db/foo"), "").unwrap();
    std::fs::write(root.join("git/checkouts/foo"), "").unwrap();

    p.cargo("clean gc -Zgc")
        .masquerade_as_nightly_cargo(&["gc"])
        .with_stderr_data(str![[r#"
[REMOVED] [FILE_NUM] files, [FILE_SIZE]B total

"#]])
        .run();
}
