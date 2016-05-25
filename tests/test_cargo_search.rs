use std::fs::{self, File};
use std::io::prelude::*;
use std::path::PathBuf;

use url::Url;

use cargo::util::ProcessBuilder;
use support::execs;
use support::paths;
use support::git::repo;

use hamcrest::assert_that;

fn registry_path() -> PathBuf { paths::root().join("registry") }
fn registry() -> Url { Url::from_file_path(&*registry_path()).ok().unwrap() }
fn api_path() -> PathBuf { paths::root().join("api") }
fn api() -> Url { Url::from_file_path(&*api_path()).ok().unwrap() }

fn setup() {
    let config = paths::root().join(".cargo/config");
    fs::create_dir_all(config.parent().unwrap()).unwrap();
    File::create(&config).unwrap().write_all(format!(r#"
        [registry]
            index = "{reg}"
    "#, reg = registry()).as_bytes()).unwrap();
    fs::create_dir_all(&api_path().join("api/v1")).unwrap();

    repo(&registry_path())
        .file("config.json", &format!(r#"{{
            "dl": "{0}",
            "api": "{0}"
        }}"#, api()))
        .build();
}

fn cargo_process(s: &str) -> ProcessBuilder {
    let mut b = ::cargo_process();
    b.arg(s);
    return b
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

    assert_that(cargo_process("search").arg("postgres"),
                execs().with_status(0)
                       .with_stderr("\
[UPDATING] registry `[..]`")
                       .with_stdout("\
hoare (0.1.1)    Design by contract style assertions for Rust"));
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

    assert_that(cargo_process("search").arg("postgres").arg("sql"),
                execs().with_status(0)
                       .with_stderr("\
[UPDATING] registry `[..]`")
                       .with_stdout("\
hoare (0.1.1)    Design by contract style assertions for Rust"));
}

#[test]
fn help() {
    assert_that(cargo_process("search").arg("-h"),
                execs().with_status(0));
    assert_that(cargo_process("help").arg("search"),
                execs().with_status(0));
}
