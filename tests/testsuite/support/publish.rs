use std::fs::{self, File};
use std::io::prelude::*;
use std::path::PathBuf;

use crate::support::git::{repo, Repository};
use crate::support::paths;

use url::Url;

pub fn setup() -> Repository {
    let config = paths::root().join(".cargo/config");
    t!(fs::create_dir_all(config.parent().unwrap()));
    t!(t!(File::create(&config)).write_all(
        format!(
            r#"
        [registry]
        token = "api-token"

        [registries.alternative]
        index = "{registry}"
    "#,
            registry = registry().to_string()
        ).as_bytes()
    ));

    let credentials = paths::root().join("home/.cargo/credentials");
    t!(fs::create_dir_all(credentials.parent().unwrap()));
    t!(t!(File::create(&credentials)).write_all(
        br#"
        [registries.alternative]
        token = "api-token"
    "#
    ));

    t!(fs::create_dir_all(&upload_path().join("api/v1/crates")));

    repo(&registry_path())
        .file(
            "config.json",
            &format!(
                r#"{{
            "dl": "{0}",
            "api": "{0}"
        }}"#,
                upload()
            ),
        ).build()
}

pub fn registry_path() -> PathBuf {
    paths::root().join("registry")
}
pub fn registry() -> Url {
    Url::from_file_path(&*registry_path()).ok().unwrap()
}
pub fn upload_path() -> PathBuf {
    paths::root().join("upload")
}
fn upload() -> Url {
    Url::from_file_path(&*upload_path()).ok().unwrap()
}
