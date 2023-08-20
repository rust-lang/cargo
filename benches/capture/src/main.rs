//! This tool helps to capture the `Cargo.toml` files of a workspace.
//!
//! Run it by passing a list of workspaces to capture.
//! Use the `-f` flag to allow it to overwrite existing captures.
//! The workspace will be saved in a `.tgz` file in the `../workspaces` directory.

use flate2::{Compression, GzBuilder};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let force = std::env::args().any(|arg| arg == "-f");
    let dest = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("workspaces");
    if !dest.exists() {
        panic!("expected {} to exist", dest.display());
    }
    for arg in std::env::args().skip(1).filter(|arg| !arg.starts_with("-")) {
        let source_root = fs::canonicalize(arg).unwrap();
        capture(&source_root, &dest, force);
    }
}

fn capture(source_root: &Path, dest: &Path, force: bool) {
    let name = Path::new(source_root.file_name().unwrap());
    let mut dest_gz = PathBuf::from(dest);
    dest_gz.push(name);
    dest_gz.set_extension("tgz");
    if dest_gz.exists() {
        if !force {
            panic!(
                "dest {:?} already exists, use -f to force overwriting",
                dest_gz
            );
        }
        fs::remove_file(&dest_gz).unwrap();
    }
    let vcs_info = capture_vcs_info(source_root, force);
    let dst = fs::File::create(&dest_gz).unwrap();
    let encoder = GzBuilder::new()
        .filename(format!("{}.tar", name.to_str().unwrap()))
        .write(dst, Compression::best());
    let mut ar = tar::Builder::new(encoder);
    ar.mode(tar::HeaderMode::Deterministic);
    if let Some(info) = &vcs_info {
        add_ar_file(&mut ar, &name.join(".cargo_vcs_info.json"), info);
    }

    // Gather all local packages.
    let metadata = cargo_metadata::MetadataCommand::new()
        .manifest_path(source_root.join("Cargo.toml"))
        .features(cargo_metadata::CargoOpt::AllFeatures)
        .exec()
        .expect("cargo_metadata failed");
    let mut found_root = false;
    for package in &metadata.packages {
        if package.source.is_some() {
            continue;
        }
        let manifest_path = package.manifest_path.as_std_path();
        copy_manifest(&manifest_path, &mut ar, name, &source_root);
        found_root |= manifest_path == source_root.join("Cargo.toml");
    }
    if !found_root {
        // A virtual workspace.
        let contents = fs::read_to_string(source_root.join("Cargo.toml")).unwrap();
        assert!(!contents.contains("[package]"));
        add_ar_file(&mut ar, &name.join("Cargo.toml"), &contents);
    }
    let lock = fs::read_to_string(source_root.join("Cargo.lock")).unwrap();
    add_ar_file(&mut ar, &name.join("Cargo.lock"), &lock);
    let encoder = ar.into_inner().unwrap();
    encoder.finish().unwrap();
    eprintln!("created {}", dest_gz.display());
}

fn copy_manifest<W: std::io::Write>(
    manifest_path: &Path,
    ar: &mut tar::Builder<W>,
    name: &Path,
    source_root: &Path,
) {
    let relative_path = manifest_path
        .parent()
        .unwrap()
        .strip_prefix(source_root)
        .expect("workspace member should be under workspace root");
    let relative_path = name.join(relative_path);
    let contents = fs::read_to_string(&manifest_path).unwrap();
    let mut manifest: toml::Value = toml::from_str(&contents).unwrap();
    let remove = |obj: &mut toml::Value, name| {
        let table = obj.as_table_mut().unwrap();
        if table.contains_key(name) {
            table.remove(name);
        }
    };
    remove(&mut manifest, "lib");
    remove(&mut manifest, "bin");
    remove(&mut manifest, "example");
    remove(&mut manifest, "test");
    remove(&mut manifest, "bench");
    remove(&mut manifest, "profile");
    if let Some(package) = manifest.get_mut("package") {
        remove(package, "default-run");
    }
    let contents = toml::to_string(&manifest).unwrap();
    add_ar_file(ar, &relative_path.join("Cargo.toml"), &contents);
    add_ar_file(ar, &relative_path.join("src").join("lib.rs"), "");
}

fn add_ar_file<W: std::io::Write>(ar: &mut tar::Builder<W>, path: &Path, contents: &str) {
    let mut header = tar::Header::new_gnu();
    header.set_entry_type(tar::EntryType::file());
    header.set_mode(0o644);
    header.set_size(contents.len() as u64);
    header.set_mtime(123456789);
    header.set_cksum();
    ar.append_data(&mut header, path, contents.as_bytes())
        .unwrap();
}

fn capture_vcs_info(ws_root: &Path, force: bool) -> Option<String> {
    let maybe_git = |command: &str| {
        Command::new("git")
            .current_dir(ws_root)
            .args(command.split_whitespace().collect::<Vec<_>>())
            .output()
            .expect("git should be installed")
    };
    assert!(ws_root.join("Cargo.toml").exists());
    let relative = maybe_git("ls-files --full-name Cargo.toml");
    if !relative.status.success() {
        if !force {
            panic!("git repository not detected, use -f to force");
        }
        return None;
    }
    let p = Path::new(std::str::from_utf8(&relative.stdout).unwrap().trim());
    let relative = p.parent().unwrap();
    if !force {
        let has_changes = !maybe_git("diff-index --quiet HEAD .").status.success();
        if has_changes {
            panic!("git repo appears to have changes, use -f to force, or clean the repo");
        }
    }
    let commit = maybe_git("rev-parse HEAD");
    assert!(commit.status.success());
    let commit = std::str::from_utf8(&commit.stdout).unwrap().trim();
    let remote = maybe_git("remote get-url origin");
    assert!(remote.status.success());
    let remote = std::str::from_utf8(&remote.stdout).unwrap().trim();
    let info = format!(
        "{{\n  \"git\": {{\n    \"sha1\": \"{}\",\n     \"remote\": \"{}\"\n  }},\
         \n  \"path_in_vcs\": \"{}\"\n}}\n",
        commit,
        remote,
        relative.display()
    );
    eprintln!("recording vcs info:\n{}", info);
    Some(info)
}
