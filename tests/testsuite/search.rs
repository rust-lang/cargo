use std::collections::HashSet;
use std::fs::{self, File};
use std::io::prelude::*;
use std::path::Path;

use cargo_test_support::cargo_process;
use cargo_test_support::git::repo;
use cargo_test_support::paths;
use cargo_test_support::registry::{api_path, registry_path, registry_url};
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
        }],
        "meta": {
            "total": 1
        }
    }"#;

    // Older versions of curl don't peel off query parameters when looking for
    // filenames, so just make both files.
    //
    // On windows, though, `?` is an invalid character, but we always build curl
    // from source there anyway!
    File::create(&dest)
        .unwrap()
        .write_all(content.as_bytes())
        .unwrap();
    if !cfg!(windows) {
        File::create(&dest.with_file_name("crates?q=postgres&per_page=10"))
            .unwrap()
            .write_all(content.as_bytes())
            .unwrap();
    }
}

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

    File::create(&config)
        .unwrap()
        .write_all(
            format!(
                r#"
[source.crates-io]
registry = 'https://wut'
replace-with = 'dummy-registry'

[source.dummy-registry]
registry = '{reg}'
"#,
                reg = registry_url(),
            )
            .as_bytes(),
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
    let cfg = Config::new(Shell::new(), paths::root(), paths::home().join(".cargo"));
    let lock = cfg.acquire_package_cache_lock().unwrap();
    let mut regsrc = RegistrySource::remote(sid, &HashSet::new(), &cfg);
    regsrc.update().unwrap();
    drop(lock);

    cargo_process("search postgres")
        .with_stdout_contains("hoare = \"0.1.1\"    # Design by contract style assertions for Rust")
        .with_stderr("") // without "Updating ... index"
        .run();
}

#[cargo_test]
fn replace_default() {
    setup();
    set_cargo_config();

    cargo_process("search postgres")
        .with_stdout_contains("hoare = \"0.1.1\"    # Design by contract style assertions for Rust")
        .with_stderr_contains("[..]Updating [..] index")
        .run();
}

#[cargo_test]
fn simple() {
    setup();

    cargo_process("search postgres --index")
        .arg(registry_url().to_string())
        .with_stdout_contains("hoare = \"0.1.1\"    # Design by contract style assertions for Rust")
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
        .with_stdout_contains("hoare = \"0.1.1\"    # Design by contract style assertions for Rust")
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
        .with_stdout_contains("hoare = \"0.1.1\"    # Design by contract style assertions for Rust")
        .run();
}

#[cargo_test]
fn multiple_query_params() {
    setup();

    cargo_process("search postgres sql --index")
        .arg(registry_url().to_string())
        .with_stdout_contains("hoare = \"0.1.1\"    # Design by contract style assertions for Rust")
        .run();
}

#[cargo_test]
fn help() {
    cargo_process("search -h").run();
    cargo_process("help search").run();
    // Ensure that help output goes to stdout, not stderr.
    cargo_process("search --help").with_stderr("").run();
    cargo_process("search --help")
        .with_stdout_contains("[..] --frozen [..]")
        .run();
}
