use crate::compare::{assert_match_exact, find_json_mismatch};
use crate::registry::{self, alt_api_path, FeatureMap};
use flate2::read::GzDecoder;
use std::collections::{HashMap, HashSet};
use std::fs;
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
    let base_crate_name = Path::new(
        expected_crate_name
            .strip_suffix(".crate")
            .expect("must end with .crate"),
    );
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

pub(crate) fn create_index_line(
    name: serde_json::Value,
    vers: &str,
    deps: Vec<serde_json::Value>,
    cksum: &str,
    features: crate::registry::FeatureMap,
    yanked: bool,
    links: Option<String>,
    rust_version: Option<&str>,
    v: Option<u32>,
) -> String {
    // This emulates what crates.io does to retain backwards compatibility.
    let (features, features2) = split_index_features(features.clone());
    let mut json = serde_json::json!({
        "name": name,
        "vers": vers,
        "deps": deps,
        "cksum": cksum,
        "features": features,
        "yanked": yanked,
        "links": links,
    });
    if let Some(f2) = &features2 {
        json["features2"] = serde_json::json!(f2);
        json["v"] = serde_json::json!(2);
    }
    if let Some(v) = v {
        json["v"] = serde_json::json!(v);
    }
    if let Some(rust_version) = rust_version {
        json["rust_version"] = serde_json::json!(rust_version);
    }

    json.to_string()
}

pub(crate) fn write_to_index(registry_path: &Path, name: &str, line: String, local: bool) {
    let file = cargo_util::registry::make_dep_path(name, false);

    // Write file/line in the index.
    let dst = if local {
        registry_path.join("index").join(&file)
    } else {
        registry_path.join(&file)
    };
    let prev = fs::read_to_string(&dst).unwrap_or_default();
    t!(fs::create_dir_all(dst.parent().unwrap()));
    t!(fs::write(&dst, prev + &line[..] + "\n"));

    // Add the new file to the index.
    if !local {
        let repo = t!(git2::Repository::open(&registry_path));
        let mut index = t!(repo.index());
        t!(index.add_path(Path::new(&file)));
        t!(index.write());
        let id = t!(index.write_tree());

        // Commit this change.
        let tree = t!(repo.find_tree(id));
        let sig = t!(repo.signature());
        let parent = t!(repo.refname_to_id("refs/heads/master"));
        let parent = t!(repo.find_commit(parent));
        t!(repo.commit(
            Some("HEAD"),
            &sig,
            &sig,
            "Another commit",
            &tree,
            &[&parent]
        ));
    }
}

fn split_index_features(mut features: FeatureMap) -> (FeatureMap, Option<FeatureMap>) {
    let mut features2 = FeatureMap::new();
    for (feat, values) in features.iter_mut() {
        if values
            .iter()
            .any(|value| value.starts_with("dep:") || value.contains("?/"))
        {
            let new_values = values.drain(..).collect();
            features2.insert(feat.clone(), new_values);
        }
    }
    if features2.is_empty() {
        (features, None)
    } else {
        (features, Some(features2))
    }
}
