//! Tests for the `cargo search` command.

use cargo::util::cache_lock::CacheLockMode;
use cargo_test_support::cargo_process;
use cargo_test_support::paths;
use cargo_test_support::registry::{RegistryBuilder, Response};
use std::collections::HashSet;

const SEARCH_API_RESPONSE: &[u8] = br#"
{
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

const SEARCH_RESULTS: &str = "\
hoare = \"0.1.1\"        # Design by contract style assertions for Rust
postgres = \"0.17.3\"    # A native, synchronous PostgreSQL client
";

#[must_use]
fn setup() -> RegistryBuilder {
    RegistryBuilder::new()
        .http_api()
        .add_responder("/api/v1/crates", |_, _| Response {
            code: 200,
            headers: vec![],
            body: SEARCH_API_RESPONSE.to_vec(),
        })
}

#[cargo_test]
fn not_update() {
    let registry = setup().build();

    use cargo::core::{Shell, SourceId};
    use cargo::sources::source::Source;
    use cargo::sources::RegistrySource;
    use cargo::util::Config;

    let sid = SourceId::for_registry(registry.index_url()).unwrap();
    let cfg = Config::new(
        Shell::from_write(Box::new(Vec::new())),
        paths::root(),
        paths::home().join(".cargo"),
    );
    let lock = cfg
        .acquire_package_cache_lock(CacheLockMode::DownloadExclusive)
        .unwrap();
    let mut regsrc = RegistrySource::remote(sid, &HashSet::new(), &cfg).unwrap();
    regsrc.invalidate_cache();
    regsrc.block_until_ready().unwrap();
    drop(lock);

    cargo_process("search postgres")
        .replace_crates_io(registry.index_url())
        .with_stdout_contains(SEARCH_RESULTS)
        .with_stderr("") // without "Updating ... index"
        .run();
}

#[cargo_test]
fn replace_default() {
    let registry = setup().build();

    cargo_process("search postgres")
        .replace_crates_io(registry.index_url())
        .with_stdout_contains(SEARCH_RESULTS)
        .with_stderr_contains("[..]Updating [..] index")
        .run();
}

#[cargo_test]
fn simple() {
    let registry = setup().build();

    cargo_process("search postgres --index")
        .arg(registry.index_url().as_str())
        .with_stdout_contains(SEARCH_RESULTS)
        .run();
}

#[cargo_test]
fn multiple_query_params() {
    let registry = setup().build();

    cargo_process("search postgres sql --index")
        .arg(registry.index_url().as_str())
        .with_stdout_contains(SEARCH_RESULTS)
        .run();
}

#[cargo_test]
fn ignore_quiet() {
    let registry = setup().build();

    cargo_process("search -q postgres")
        .replace_crates_io(registry.index_url())
        .with_stdout_contains(SEARCH_RESULTS)
        .run();
}

#[cargo_test]
fn colored_results() {
    let registry = setup().build();

    cargo_process("search --color=never postgres")
        .replace_crates_io(registry.index_url())
        .with_stdout_does_not_contain("[..]\x1b[[..]")
        .run();

    cargo_process("search --color=always postgres")
        .replace_crates_io(registry.index_url())
        .with_stdout_contains("[..]\x1b[[..]")
        .run();
}

#[cargo_test]
fn auth_required_failure() {
    let server = setup().auth_required().no_configure_token().build();

    cargo_process("search postgres")
        .replace_crates_io(server.index_url())
        .with_status(101)
        .with_stderr_contains("[ERROR] no token found, please run `cargo login`")
        .run();
}

#[cargo_test]
fn auth_required() {
    let server = setup().auth_required().build();

    cargo_process("search postgres")
        .replace_crates_io(server.index_url())
        .with_stdout_contains(SEARCH_RESULTS)
        .run();
}
