//! Tests for the `cargo search` command.

use cargo::core::GitReference;
use cargo_test_support::cargo_process;
use cargo_test_support::git::repo;
use cargo_test_support::paths;
use cargo_test_support::registry::{api_path, registry_path, registry_url};
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use url::Url;

fn api() -> Url {
    Url::from_file_path(&*api_path()).ok().unwrap()
}

fn write_crates(dest: &Path) {
    let content = r#"{
        "crates": [{
            "created_at": "2014-11-16T20:17:35Z",
            "description": "Design by contract style assertions for Rust",
            "documentation": null,
            "downloads": 2,
            "homepage": null,
            "id": "hoare",
            "keywords": [],
            "license": null,
            "links": {
                "owners": "/api/v1/crates/hoare/owners",
                "reverse_dependencies": "/api/v1/crates/hoare/reverse_dependencies",
                "version_downloads": "/api/v1/crates/hoare/downloads",
                "versions": "/api/v1/crates/hoare/versions"
            },
            "max_version": "0.1.1",
            "name": "hoare",
            "repository": "https://github.com/nick29581/libhoare",
            "updated_at": "2014-11-20T21:49:21Z",
            "versions": null
        },
        {
            "id": "postgres",
            "name": "postgres",
            "updated_at": "2020-05-01T23:17:54.335921+00:00",
            "versions": null,
            "keywords": null,
            "categories": null,
            "badges": [
                {
                    "badge_type": "circle-ci",
                    "attributes": {
                        "repository": "sfackler/rust-postgres",
                        "branch": null
                    }
                }
            ],
            "created_at": "2014-11-24T02:34:44.756689+00:00",
            "downloads": 535491,
            "recent_downloads": 88321,
            "max_version": "0.17.3",
            "newest_version": "0.17.3",
            "description": "A native, synchronous PostgreSQL client",
            "homepage": null,
            "documentation": null,
            "repository": "https://github.com/sfackler/rust-postgres",
            "links": {
                "version_downloads": "/api/v1/crates/postgres/downloads",
                "versions": "/api/v1/crates/postgres/versions",
                "owners": "/api/v1/crates/postgres/owners",
                "owner_team": "/api/v1/crates/postgres/owner_team",
                "owner_user": "/api/v1/crates/postgres/owner_user",
                "reverse_dependencies": "/api/v1/crates/postgres/reverse_dependencies"
            },
            "exact_match": true
        }
        ],
        "meta": {
            "total": 2
        }
    }"#;

    // Older versions of curl don't peel off query parameters when looking for
    // filenames, so just make both files.
    //
    // On windows, though, `?` is an invalid character, but we always build curl
    // from source there anyway!
    fs::write(&dest, content).unwrap();
    if !cfg!(windows) {
        fs::write(
            &dest.with_file_name("crates?q=postgres&per_page=10"),
            content,
        )
        .unwrap();
    }
}

const SEARCH_RESULTS: &str = "\
hoare = \"0.1.1\"        # Design by contract style assertions for Rust
postgres = \"0.17.3\"    # A native, synchronous PostgreSQL client
";

fn setup() {
    let cargo_home = paths::root().join(".cargo");
    fs::create_dir_all(cargo_home).unwrap();
    fs::create_dir_all(&api_path().join("api/v1")).unwrap();

    // Init a new registry
    let _ = repo(&registry_path())
        .file(
            "config.json",
            &format!(r#"{{"dl":"{0}","api":"{0}"}}"#, api()),
        )
        .build();

    let base = api_path().join("api/v1/crates");
    write_crates(&base);
}

fn set_cargo_config() {
    let config = paths::root().join(".cargo/config");

    fs::write(
        &config,
        format!(
            r#"
            [source.crates-io]
            registry = 'https://wut'
            replace-with = 'dummy-registry'

            [source.dummy-registry]
            registry = '{reg}'
            "#,
            reg = registry_url(),
        ),
    )
    .unwrap();
}

#[cargo_test]
fn not_update() {
    setup();
    set_cargo_config();

    use cargo::core::{Shell, Source, SourceId};
    use cargo::sources::RegistrySource;
    use cargo::util::Config;

    let sid = SourceId::for_registry(&registry_url()).unwrap();
    let cfg = Config::new(
        Shell::from_write(Box::new(Vec::new())),
        paths::root(),
        paths::home().join(".cargo"),
    );
    let lock = cfg.acquire_package_cache_lock().unwrap();
    let mut regsrc =
        RegistrySource::remote(sid, &HashSet::new(), &cfg, GitReference::DefaultBranch);
    regsrc.update().unwrap();
    drop(lock);

    cargo_process("search postgres")
        .with_stdout_contains(SEARCH_RESULTS)
        .with_stderr("") // without "Updating ... index"
        .run();
}

#[cargo_test]
fn replace_default() {
    setup();
    set_cargo_config();

    cargo_process("search postgres")
        .with_stdout_contains(SEARCH_RESULTS)
        .with_stderr_contains("[..]Updating [..] index")
        .run();
}

#[cargo_test]
fn simple() {
    setup();

    cargo_process("search postgres --index")
        .arg(registry_url().to_string())
        .with_stdout_contains(SEARCH_RESULTS)
        .run();
}

// TODO: Deprecated
// remove once it has been decided '--host' can be safely removed
#[cargo_test]
fn simple_with_host() {
    setup();

    cargo_process("search postgres --host")
        .arg(registry_url().to_string())
        .with_stderr(
            "\
[WARNING] The flag '--host' is no longer valid.

Previous versions of Cargo accepted this flag, but it is being
deprecated. The flag is being renamed to 'index', as the flag
wants the location of the index. Please use '--index' instead.

This will soon become a hard error, so it's either recommended
to update to a fixed version or contact the upstream maintainer
about this warning.
[UPDATING] `[CWD]/registry` index
",
        )
        .with_stdout_contains(SEARCH_RESULTS)
        .run();
}

// TODO: Deprecated
// remove once it has been decided '--host' can be safely removed
#[cargo_test]
fn simple_with_index_and_host() {
    setup();

    cargo_process("search postgres --index")
        .arg(registry_url().to_string())
        .arg("--host")
        .arg(registry_url().to_string())
        .with_stderr(
            "\
[WARNING] The flag '--host' is no longer valid.

Previous versions of Cargo accepted this flag, but it is being
deprecated. The flag is being renamed to 'index', as the flag
wants the location of the index. Please use '--index' instead.

This will soon become a hard error, so it's either recommended
to update to a fixed version or contact the upstream maintainer
about this warning.
[UPDATING] `[CWD]/registry` index
",
        )
        .with_stdout_contains(SEARCH_RESULTS)
        .run();
}

#[cargo_test]
fn multiple_query_params() {
    setup();

    cargo_process("search postgres sql --index")
        .arg(registry_url().to_string())
        .with_stdout_contains(SEARCH_RESULTS)
        .run();
}
