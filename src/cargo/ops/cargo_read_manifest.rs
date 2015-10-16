use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::prelude::*;
use std::io;
use std::path::{Path, PathBuf};

use core::{Package, Manifest, SourceId, PackageId};
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

    Ok((Package::new(manifest, path), nested))
}

pub fn read_packages(path: &Path, source_id: &SourceId, config: &Config)
                     -> CargoResult<Vec<Package>> {
    let mut all_packages = HashMap::new();
    let mut visited = HashSet::<PathBuf>::new();

    trace!("looking for root package: {}, source_id={}", path.display(), source_id);

    try!(walk(path, &mut |dir| {
        trace!("looking for child package: {}", dir.display());

        // Don't recurse into hidden/dot directories unless we're at the toplevel
        if dir != path {
            let name = dir.file_name().and_then(|s| s.to_str());
            if name.map(|s| s.starts_with(".")) == Some(true) {
                return Ok(false)
            }

            // Don't automatically discover packages across git submodules
            if fs::metadata(&dir.join(".git")).is_ok() {
                return Ok(false)
            }
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
        Ok(all_packages.into_iter().map(|(_, v)| v).collect())
    }
}

fn walk(path: &Path, callback: &mut FnMut(&Path) -> CargoResult<bool>)
        -> CargoResult<()> {
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
        Err(e) => return Err(From::from(e)),
    };
    for dir in dirs {
        let dir = try!(dir);
        if try!(dir.file_type()).is_dir() {
            try!(walk(&dir.path(), callback));
        }
    }
    Ok(())
}

fn has_manifest(path: &Path) -> bool {
    find_project_manifest_exact(path, "Cargo.toml").is_ok()
}

fn read_nested_packages(path: &Path,
                        all_packages: &mut HashMap<PackageId, Package>,
                        source_id: &SourceId,
                        config: &Config,
                        visited: &mut HashSet<PathBuf>) -> CargoResult<()> {
    if !visited.insert(path.to_path_buf()) { return Ok(()) }

    let manifest = try!(find_project_manifest_exact(path, "Cargo.toml"));

    let (pkg, nested) = try!(read_package(&manifest, source_id, config));
    let pkg_id = pkg.package_id().clone();
    if !all_packages.contains_key(&pkg_id) {
        all_packages.insert(pkg_id, pkg);
    } else {
        info!("skipping nested package `{}` found at `{}`",
              pkg.name(), path.to_string_lossy());
    }

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
