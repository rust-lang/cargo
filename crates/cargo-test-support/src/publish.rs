//! Helpers for testing `cargo package` / `cargo publish`
//!
//! # Example
//!
//! ```no_run
//! # use cargo_test_support::registry::RegistryBuilder;
//! # use cargo_test_support::publish::validate_upload;
//! # use cargo_test_support::project;
//! validate_upload(
//!     r#"
//!     {
//!       "authors": [],
//!       "badges": {},
//!       "categories": [],
//!       "deps": [],
//!       "description": "foo",
//!       "documentation": null,
//!       "features": {},
//!       "homepage": null,
//!       "keywords": [],
//!       "license": "MIT",
//!       "license_file": null,
//!       "links": null,
//!       "name": "foo",
//!       "readme": null,
//!       "readme_file": null,
//!       "repository": null,
//!       "rust_version": null,
//!       "vers": "0.0.1"
//!       }
//!     "#,
//!     "foo-0.0.1.crate",
//!     &["Cargo.lock", "Cargo.toml", "Cargo.toml.orig", "src/main.rs"],
//! );
//! ```

use crate::compare::InMemoryDir;
use crate::registry::{self, alt_api_path};
use flate2::read::GzDecoder;
use snapbox::prelude::*;
use std::collections::HashSet;
use std::fs::File;
use std::io::{self, SeekFrom, prelude::*};
use std::path::Path;
use tar::Archive;

fn read_le_u32<R>(mut reader: R) -> io::Result<u32>
where
    R: Read,
{
    let mut buf = [0; 4];
    reader.read_exact(&mut buf)?;
    Ok(u32::from_le_bytes(buf))
}

/// Check the `cargo publish` API call
#[track_caller]
pub fn validate_upload(expected_json: &str, expected_crate_name: &str, expected_files: &[&str]) {
    let new_path = registry::api_path().join("api/v1/crates/new");
    _validate_upload(
        &new_path,
        expected_json,
        expected_crate_name,
        expected_files,
        (),
    );
}

/// Check the `cargo publish` API call, with file contents
#[track_caller]
pub fn validate_upload_with_contents(
    expected_json: &str,
    expected_crate_name: &str,
    expected_files: &[&str],
    expected_contents: impl Into<InMemoryDir>,
) {
    let new_path = registry::api_path().join("api/v1/crates/new");
    _validate_upload(
        &new_path,
        expected_json,
        expected_crate_name,
        expected_files,
        expected_contents,
    );
}

/// Check the `cargo publish` API call to the alternative test registry
#[track_caller]
pub fn validate_alt_upload(
    expected_json: &str,
    expected_crate_name: &str,
    expected_files: &[&str],
) {
    let new_path = alt_api_path().join("api/v1/crates/new");
    _validate_upload(
        &new_path,
        expected_json,
        expected_crate_name,
        expected_files,
        (),
    );
}

#[track_caller]
fn _validate_upload(
    new_path: &Path,
    expected_json: &str,
    expected_crate_name: &str,
    expected_files: &[&str],
    expected_contents: impl Into<InMemoryDir>,
) {
    let (actual_json, krate_bytes) = read_new_post(new_path);

    snapbox::assert_data_eq!(actual_json, expected_json.is_json());

    // Verify the tarball.
    validate_crate_contents(
        &krate_bytes[..],
        expected_crate_name,
        expected_files,
        expected_contents,
    );
}

#[track_caller]
fn read_new_post(new_path: &Path) -> (Vec<u8>, Vec<u8>) {
    let mut f = File::open(new_path).unwrap();

    // 32-bit little-endian integer of length of JSON data.
    let json_sz = read_le_u32(&mut f).expect("read json length");
    let mut json_bytes = vec![0; json_sz as usize];
    f.read_exact(&mut json_bytes).expect("read JSON data");

    // 32-bit little-endian integer of length of crate file.
    let crate_sz = read_le_u32(&mut f).expect("read crate length");
    let mut krate_bytes = vec![0; crate_sz as usize];
    f.read_exact(&mut krate_bytes).expect("read crate data");

    // Check at end.
    let current = f.seek(SeekFrom::Current(0)).unwrap();
    assert_eq!(f.seek(SeekFrom::End(0)).unwrap(), current);

    (json_bytes, krate_bytes)
}

/// Checks the contents of a `.crate` file.
///
/// - `expected_crate_name` should be something like `foo-0.0.1.crate`.
/// - `expected_files` should be a complete list of files in the crate
///   (relative to `expected_crate_name`).
/// - `expected_contents` should be a list of `(file_name, contents)` tuples
///   to validate the contents of the given file. Only the listed files will
///   be checked (others will be ignored).
#[track_caller]
pub fn validate_crate_contents(
    reader: impl Read,
    expected_crate_name: &str,
    expected_files: &[&str],
    expected_contents: impl Into<InMemoryDir>,
) {
    let expected_contents = expected_contents.into();
    validate_crate_contents_(
        reader,
        expected_crate_name,
        expected_files,
        expected_contents,
    )
}

#[track_caller]
fn validate_crate_contents_(
    reader: impl Read,
    expected_crate_name: &str,
    expected_files: &[&str],
    expected_contents: InMemoryDir,
) {
    let mut rdr = GzDecoder::new(reader);
    snapbox::assert_data_eq!(rdr.header().unwrap().filename().unwrap(), {
        let expected: snapbox::Data = expected_crate_name.into();
        expected.raw()
    });

    let mut contents = Vec::new();
    rdr.read_to_end(&mut contents).unwrap();
    let mut ar = Archive::new(&contents[..]);
    let base_crate_name = Path::new(
        expected_crate_name
            .strip_suffix(".crate")
            .expect("must end with .crate"),
    );
    let actual_contents: InMemoryDir = ar
        .entries()
        .unwrap()
        .map(|entry| {
            let mut entry = entry.unwrap();
            let name = entry
                .path()
                .unwrap()
                .strip_prefix(base_crate_name)
                .unwrap()
                .to_owned();
            let mut contents = String::new();
            entry.read_to_string(&mut contents).unwrap();
            (name, contents)
        })
        .collect();
    let actual_files: HashSet<&Path> = actual_contents.paths().collect();
    let expected_files: HashSet<&Path> =
        expected_files.iter().map(|name| Path::new(name)).collect();
    let missing: Vec<&&Path> = expected_files.difference(&actual_files).collect();
    let extra: Vec<&&Path> = actual_files.difference(&expected_files).collect();
    if !missing.is_empty() || !extra.is_empty() {
        panic!(
            "uploaded archive does not match.\nMissing: {:?}\nExtra: {:?}\n",
            missing, extra
        );
    }
    actual_contents.assert_contains(&expected_contents);
}
