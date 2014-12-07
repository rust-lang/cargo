use std::io::{mod, fs, File};

use url::Url;

use cargo::util::{process, ProcessBuilder};
use support::UPDATING;
use support::{ResultTest, execs, cargo_dir};
use support::paths;
use support::git::repo;

use hamcrest::assert_that;

fn registry_path() -> Path { paths::root().join("registry") }
fn registry() -> Url { Url::from_file_path(&registry_path()).unwrap() }
fn api_path() -> Path { paths::root().join("api") }
fn api() -> Url { Url::from_file_path(&api_path()).unwrap() }

fn setup() {
    let config = paths::root().join(".cargo/config");
    fs::mkdir_recursive(&config.dir_path(), io::USER_DIR).assert();
    File::create(&config).write_str(format!(r#"
        [registry]
            index = "{reg}"
    "#, reg = registry()).as_slice()).assert();
    fs::mkdir_recursive(&api_path().join("api/v1"), io::USER_DIR).assert();

    repo(&registry_path())
        .file("config.json", format!(r#"{{
            "dl": "{0}",
            "api": "{0}"
        }}"#, api()))
        .build();
}

fn cargo_process(s: &str) -> ProcessBuilder {
    process(cargo_dir().join("cargo")).unwrap().arg(s)
        .cwd(paths::root())
        .env("HOME", Some(paths::home()))
}

test!(simple {
    let crates = api_path().join("api/v1/crates");
    File::create(&crates).write_str(r#"{
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
    }"#).assert();

    assert_that(cargo_process("search").arg("postgres"),
                execs().with_status(0).with_stdout(format!("\
{updating} registry `[..]`
hoare (0.1.1)    Design by contract style assertions for Rust", updating = UPDATING)));
})
