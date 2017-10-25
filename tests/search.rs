extern crate cargo;
extern crate cargotest;
extern crate hamcrest;
extern crate url;

use std::fs::{self, File};
use std::io::prelude::*;
use std::path::PathBuf;

use cargo::util::ProcessBuilder;
use cargotest::support::execs;
use cargotest::support::git::repo;
use cargotest::support::paths;
use hamcrest::assert_that;
use url::Url;

fn registry_path() -> PathBuf { paths::root().join("registry") }
fn registry() -> Url { Url::from_file_path(&*registry_path()).ok().unwrap() }
fn api_path() -> PathBuf { paths::root().join("api") }
fn api() -> Url { Url::from_file_path(&*api_path()).ok().unwrap() }

fn setup() {
    let config = paths::root().join(".cargo/config");
    fs::create_dir_all(config.parent().unwrap()).unwrap();
    fs::create_dir_all(&api_path().join("api/v1")).unwrap();

    let _ = repo(&registry_path())
        .file("config.json", &format!(r#"{{
            "dl": "{0}",
            "api": "{0}"
        }}"#, api()))
        .build();
}

fn cargo_process(s: &str) -> ProcessBuilder {
    let mut b = cargotest::cargo_process();
    b.arg(s);
    b
}

#[test]
fn simple() {
    setup();

    let contents = r#"{
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
    let base = api_path().join("api/v1/crates");

    // Older versions of curl don't peel off query parameters when looking for
    // filenames, so just make both files.
    //
    // On windows, though, `?` is an invalid character, but we always build curl
    // from source there anyway!
    File::create(&base).unwrap().write_all(contents.as_bytes()).unwrap();
    if !cfg!(windows) {
        File::create(&base.with_file_name("crates?q=postgres&per_page=10")).unwrap()
             .write_all(contents.as_bytes()).unwrap();
    }

    assert_that(cargo_process("search").arg("postgres")
                    .arg("--index").arg(registry().to_string()),
                execs().with_status(0)
                       .with_stdout_contains("\
hoare = \"0.1.1\"    # Design by contract style assertions for Rust"));
}

// TODO: Depricated
// remove once it has been decided '--host' can be safely removed
#[test]
fn simple_with_host() {
    setup();

    let contents = r#"{
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
    let base = api_path().join("api/v1/crates");

    // Older versions of curl don't peel off query parameters when looking for
    // filenames, so just make both files.
    //
    // On windows, though, `?` is an invalid character, but we always build curl
    // from source there anyway!
    File::create(&base).unwrap().write_all(contents.as_bytes()).unwrap();
    if !cfg!(windows) {
        File::create(&base.with_file_name("crates?q=postgres&per_page=10")).unwrap()
             .write_all(contents.as_bytes()).unwrap();
    }

    assert_that(cargo_process("search").arg("postgres")
                    .arg("--host").arg(registry().to_string()),
                execs().with_status(0)
                       .with_stderr(&format!("\
[WARNING] The flag '--host' is no longer valid.

Previous versions of Cargo accepted this flag, but it is being
depricated. The flag is being renamed to 'index', as the flag
wants the location of the index in which to search. Please
use '--index' instead.

This will soon become a hard error, so it's either recommended
to update to a fixed version or contact the upstream maintainer
about this warning.
[UPDATING] registry `{reg}`
",
    reg = registry()))
                       .with_stdout_contains("\
hoare = \"0.1.1\"    # Design by contract style assertions for Rust"));
}

// TODO: Depricated
// remove once it has been decided '--host' can be safely removed
#[test]
fn simple_with_index_and_host() {
    setup();

    let contents = r#"{
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
    let base = api_path().join("api/v1/crates");

    // Older versions of curl don't peel off query parameters when looking for
    // filenames, so just make both files.
    //
    // On windows, though, `?` is an invalid character, but we always build curl
    // from source there anyway!
    File::create(&base).unwrap().write_all(contents.as_bytes()).unwrap();
    if !cfg!(windows) {
        File::create(&base.with_file_name("crates?q=postgres&per_page=10")).unwrap()
             .write_all(contents.as_bytes()).unwrap();
    }

    assert_that(cargo_process("search").arg("postgres")
                    .arg("--index").arg(registry().to_string())
                    .arg("--host").arg(registry().to_string()),
                execs().with_status(0)
                        .with_stderr(&format!("\
[WARNING] The flag '--host' is no longer valid.

Previous versions of Cargo accepted this flag, but it is being
depricated. The flag is being renamed to 'index', as the flag
wants the location of the index in which to search. Please
use '--index' instead.

This will soon become a hard error, so it's either recommended
to update to a fixed version or contact the upstream maintainer
about this warning.
[UPDATING] registry `{reg}`
",
    reg = registry()))
                       .with_stdout_contains("\
hoare = \"0.1.1\"    # Design by contract style assertions for Rust"));
}

#[test]
fn multiple_query_params() {
    setup();

    let contents = r#"{
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
    let base = api_path().join("api/v1/crates");

    // Older versions of curl don't peel off query parameters when looking for
    // filenames, so just make both files.
    //
    // On windows, though, `?` is an invalid character, but we always build curl
    // from source there anyway!
    File::create(&base).unwrap().write_all(contents.as_bytes()).unwrap();
    if !cfg!(windows) {
        File::create(&base.with_file_name("crates?q=postgres+sql&per_page=10")).unwrap()
             .write_all(contents.as_bytes()).unwrap();
    }

    assert_that(cargo_process("search").arg("postgres").arg("sql")
                    .arg("--index").arg(registry().to_string()),
                execs().with_status(0)
                       .with_stdout_contains("\
hoare = \"0.1.1\"    # Design by contract style assertions for Rust"));
}

#[test]
fn help() {
    assert_that(cargo_process("search").arg("-h"),
                execs().with_status(0));
    assert_that(cargo_process("help").arg("search"),
                execs().with_status(0));
}
