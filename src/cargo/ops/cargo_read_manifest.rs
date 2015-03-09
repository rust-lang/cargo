use std::collections::HashSet;
use std::error::FromError;
use std::fs::{self, File};
use std::io::prelude::*;
use std::io;
use std::path::{Path, PathBuf};

use core::{Package,Manifest,SourceId};
use util::{self, CargoResult, human, Config, ChainError};
use util::important_paths::find_project_manifest_exact;
use util::toml::{Layout, project_layout};

pub fn read_manifest(contents: &[u8], layout: Layout, source_id: &SourceId,
                     config: &Config)
                     -> CargoResult<(Manifest, Vec<PathBuf>)> {
    let root = layout.root.clone();
    util::toml::to_manifest(contents, source_id, layout, config).chain_error(|| {
        human(format!("failed to parse manifest at `{}`",
                      root.join("Cargo.toml").display()))
    })
}

pub fn read_package(path: &Path, source_id: &SourceId, config: &Config)
                    -> CargoResult<(Package, Vec<PathBuf>)> {
    trace!("read_package; path={}; source-id={}", path.display(), source_id);
    let mut file = try!(File::open(path));
    let mut data = Vec::new();
    try!(file.read_to_end(&mut data));

    let layout = project_layout(path.parent().unwrap());
    let (manifest, nested) =
        try!(read_manifest(&data, layout, source_id, config));

    Ok((Package::new(manifest, path, source_id), nested))
}

pub fn read_packages(path: &Path, source_id: &SourceId, config: &Config)
                     -> CargoResult<Vec<Package>> {
    let mut all_packages = HashSet::new();
    let mut visited = HashSet::<PathBuf>::new();

    trace!("looking for root package: {}, source_id={}", path.display(), source_id);

    try!(walk(path, &mut |dir| {
        trace!("looking for child package: {}", dir.display());

        // Don't recurse into git databases
        if dir.file_name().and_then(|s| s.to_str()) == Some(".git") {
            return Ok(false)
        }

        // Don't automatically discover packages across git submodules
        if dir != path && fs::metadata(&dir.join(".git")).is_ok() {
            return Ok(false)
        }

        // Don't ever look at target directories
        if dir.file_name().and_then(|s| s.to_str()) == Some("target") &&
           has_manifest(dir.parent().unwrap()) {
            return Ok(false)
        }

        if has_manifest(dir) {
            try!(read_nested_packages(dir, &mut all_packages, source_id, config,
                                      &mut visited));
        }
        Ok(true)
    }));

    if all_packages.is_empty() {
        Err(human(format!("Could not find Cargo.toml in `{}`", path.display())))
    } else {
        Ok(all_packages.into_iter().collect())
    }
}

fn walk<F>(path: &Path, callback: &mut F) -> CargoResult<()>
    where F: FnMut(&Path) -> CargoResult<bool>
{
    if fs::metadata(&path).map(|m| m.is_dir()) != Ok(true) {
        return Ok(())
    }

    if !try!(callback(path)) {
        trace!("not processing {}", path.display());
        return Ok(())
    }

    // Ignore any permission denied errors because temporary directories
    // can often have some weird permissions on them.
    let dirs = match fs::read_dir(path) {
        Ok(dirs) => dirs,
        Err(ref e) if e.kind() == io::ErrorKind::PermissionDenied => {
            return Ok(())
        }
        Err(e) => return Err(FromError::from_error(e)),
    };
    for dir in dirs {
        let dir = try!(dir).path();
        try!(walk(&dir, callback));
    }
    Ok(())
}

fn has_manifest(path: &Path) -> bool {
    find_project_manifest_exact(path, "Cargo.toml").is_ok()
}

fn read_nested_packages(path: &Path,
                        all_packages: &mut HashSet<Package>,
                        source_id: &SourceId,
                        config: &Config,
                        visited: &mut HashSet<PathBuf>) -> CargoResult<()> {
    if !visited.insert(path.to_path_buf()) { return Ok(()) }

    let manifest = try!(find_project_manifest_exact(path, "Cargo.toml"));

    let (pkg, nested) = try!(read_package(&manifest, source_id, config));
    all_packages.insert(pkg);

    // Registry sources are not allowed to have `path=` dependencies because
    // they're all translated to actual registry dependencies.
    //
    // We normalize the path here ensure that we don't infinitely walk around
    // looking for crates. By normalizing we ensure that we visit this crate at
    // most once.
    //
    // TODO: filesystem/symlink implications?
    if !source_id.is_registry() {
        for p in nested.iter() {
            let path = util::normalize_path(&path.join(p));
            try!(read_nested_packages(&path, all_packages, source_id,
                                      config, visited));
        }
    }

    Ok(())
}
