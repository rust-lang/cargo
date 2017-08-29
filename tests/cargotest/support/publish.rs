use std::path::PathBuf;
use std::io::prelude::*;
use std::fs::{self, File};

use support::paths;
use support::git::repo;

use url::Url;

pub fn setup() {
    let config = paths::root().join(".cargo/config");
    t!(fs::create_dir_all(config.parent().unwrap()));
    t!(t!(File::create(&config)).write_all(br#"
        [registry]
            token = "api-token"
    "#));
    t!(fs::create_dir_all(&upload_path().join("api/v1/crates")));

    repo(&registry_path())
        .file("config.json", &format!(r#"{{
            "dl": "{0}",
            "api": "{0}"
        }}"#, upload()))
        .build();
}

fn registry_path() -> PathBuf { paths::root().join("registry") }
pub fn registry() -> Url { Url::from_file_path(&*registry_path()).ok().unwrap() }
pub fn upload_path() -> PathBuf { paths::root().join("upload") }
fn upload() -> Url { Url::from_file_path(&*upload_path()).ok().unwrap() }