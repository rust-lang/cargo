use crate::compare::{assert_match_exact, find_json_mismatch};
use crate::registry::{self, alt_api_path};
use flate2::read::GzDecoder;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{self, prelude::*, SeekFrom};
use std::path::{Path, PathBuf};
use tar::Archive;

fn read_le_u32<R>(mut reader: R) -> io::Result<u32>
where
    R: Read,
{
    let mut buf = [0; 4];
    reader.read_exact(&mut buf)?;
    Ok(u32::from_le_bytes(buf))
}

/// Checks the result of a crate publish.
pub fn validate_upload(expected_json: &str, expected_crate_name: &str, expected_files: &[&str]) {
    let new_path = registry::api_path().join("api/v1/crates/new");
    _validate_upload(
        &new_path,
        expected_json,
        expected_crate_name,
        expected_files,
        &[],
    );
}

/// Checks the result of a crate publish, along with the contents of the files.
pub fn validate_upload_with_contents(
    expected_json: &str,
    expected_crate_name: &str,
    expected_files: &[&str],
    expected_contents: &[(&str, &str)],
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

/// Checks the result of a crate publish to an alternative registry.
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
        &[],
    );
}

fn _validate_upload(
    new_path: &Path,
    expected_json: &str,
    expected_crate_name: &str,
    expected_files: &[&str],
    expected_contents: &[(&str, &str)],
) {
    let mut f = File::open(new_path).unwrap();
    // 32-bit little-endian integer of length of JSON data.
    let json_sz = read_le_u32(&mut f).expect("read json length");
    let mut json_bytes = vec![0; json_sz as usize];
    f.read_exact(&mut json_bytes).expect("read JSON data");
    let actual_json = serde_json::from_slice(&json_bytes).expect("uploaded JSON should be valid");
    let expected_json = serde_json::from_str(expected_json).expect("expected JSON does not parse");

    if let Err(e) = find_json_mismatch(&expected_json, &actual_json, None) {
        panic!("{}", e);
    }

    // 32-bit little-endian integer of length of crate file.
    let crate_sz = read_le_u32(&mut f).expect("read crate length");
    let mut krate_bytes = vec![0; crate_sz as usize];
    f.read_exact(&mut krate_bytes).expect("read crate data");
    // Check at end.
    let current = f.seek(SeekFrom::Current(0)).unwrap();
    assert_eq!(f.seek(SeekFrom::End(0)).unwrap(), current);

    // Verify the tarball.
    validate_crate_contents(
        &krate_bytes[..],
        expected_crate_name,
        expected_files,
        expected_contents,
    );
}

/// Checks the contents of a `.crate` file.
///
/// - `expected_crate_name` should be something like `foo-0.0.1.crate`.
/// - `expected_files` should be a complete list of files in the crate
///   (relative to expected_crate_name).
/// - `expected_contents` should be a list of `(file_name, contents)` tuples
///   to validate the contents of the given file. Only the listed files will
///   be checked (others will be ignored).
pub fn validate_crate_contents(
    reader: impl Read,
    expected_crate_name: &str,
    expected_files: &[&str],
    expected_contents: &[(&str, &str)],
) {
    let mut rdr = GzDecoder::new(reader);
    assert_eq!(
        rdr.header().unwrap().filename().unwrap(),
        expected_crate_name.as_bytes()
    );
    let mut contents = Vec::new();
    rdr.read_to_end(&mut contents).unwrap();
    let mut ar = Archive::new(&contents[..]);
    let files: HashMap<PathBuf, String> = ar
        .entries()
        .unwrap()
        .map(|entry| {
            let mut entry = entry.unwrap();
            let name = entry.path().unwrap().into_owned();
            let mut contents = String::new();
            entry.read_to_string(&mut contents).unwrap();
            (name, contents)
        })
        .collect();
    assert!(expected_crate_name.ends_with(".crate"));
    let base_crate_name = Path::new(&expected_crate_name[..expected_crate_name.len() - 6]);
    let actual_files: HashSet<PathBuf> = files.keys().cloned().collect();
    let expected_files: HashSet<PathBuf> = expected_files
        .iter()
        .map(|name| base_crate_name.join(name))
        .collect();
    let missing: Vec<&PathBuf> = expected_files.difference(&actual_files).collect();
    let extra: Vec<&PathBuf> = actual_files.difference(&expected_files).collect();
    if !missing.is_empty() || !extra.is_empty() {
        panic!(
            "uploaded archive does not match.\nMissing: {:?}\nExtra: {:?}\n",
            missing, extra
        );
    }
    if !expected_contents.is_empty() {
        for (e_file_name, e_file_contents) in expected_contents {
            let full_e_name = base_crate_name.join(e_file_name);
            let actual_contents = files
                .get(&full_e_name)
                .unwrap_or_else(|| panic!("file `{}` missing in archive", e_file_name));
            assert_match_exact(e_file_contents, actual_contents);
        }
    }
}
