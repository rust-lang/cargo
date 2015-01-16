use std::collections::HashSet;
use std::error::FromError;
use std::io::fs::PathExtensions;
use std::io::{self, File, fs};

use core::{Package,Manifest,SourceId};
use util::{self, CargoResult, human, Config, ChainError};
use util::important_paths::find_project_manifest_exact;
use util::toml::{Layout, project_layout};

pub fn read_manifest(contents: &[u8], layout: Layout, source_id: &SourceId,
                     config: &Config)
                     -> CargoResult<(Manifest, Vec<Path>)> {
    let root = layout.root.clone();
    util::toml::to_manifest(contents, source_id, layout, config).chain_error(|| {
        human(format!("failed to parse manifest at `{:?}`",
                      root.join("Cargo.toml")))
    })
}

pub fn read_package(path: &Path, source_id: &SourceId, config: &Config)
                    -> CargoResult<(Package, Vec<Path>)> {
    log!(5, "read_package; path={}; source-id={}", path.display(), source_id);
    let mut file = try!(File::open(path));
    let data = try!(file.read_to_end());

    let layout = project_layout(&path.dir_path());
    let (manifest, nested) =
        try!(read_manifest(data.as_slice(), layout, source_id, config));

    Ok((Package::new(manifest, path, source_id), nested))
}

pub fn read_packages(path: &Path, source_id: &SourceId, config: &Config)
                     -> CargoResult<Vec<Package>> {
    let mut all_packages = HashSet::new();
    let mut visited = HashSet::<Path>::new();

    log!(5, "looking for root package: {}, source_id={}", path.display(), source_id);

    try!(walk(path, |dir| {
        log!(5, "looking for child package: {}", dir.display());

        // Don't recurse into git databases
        if dir.filename_str() == Some(".git") { return Ok(false); }

        // Don't automatically discover packages across git submodules
        if dir != path && dir.join(".git").exists() { return Ok(false); }

        // Don't ever look at target directories
        if dir.filename_str() == Some("target") && has_manifest(&dir.dir_path()) {
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
        log!(5, "all packages: {:?}", all_packages);
        Ok(all_packages.into_iter().collect())
    }
}

fn walk<F>(path: &Path, mut callback: F) -> CargoResult<()>
    where F: FnMut(&Path) -> CargoResult<bool>
{
    walk_inner(path, &mut callback)
}

fn walk_inner<F>(path: &Path, callback: &mut F) -> CargoResult<()>
    where F: FnMut(&Path) -> CargoResult<bool>
{
    if path.is_dir() {
        let continues = try!(callback(path));
        if !continues {
            log!(5, "not processing {}", path.display());
            return Ok(());
        }

        // Ignore any permission denied errors because temporary directories
        // can often have some weird permissions on them.
        let dirs = match fs::readdir(path) {
            Ok(dirs) => dirs,
            Err(ref e) if e.kind == io::PermissionDenied => return Ok(()),
            Err(e) => return Err(FromError::from_error(e)),
        };
        for dir in dirs.iter() {
            try!(walk_inner(dir, callback));
        }
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
                        visited: &mut HashSet<Path>) -> CargoResult<()> {
    if !visited.insert(path.clone()) { return Ok(()) }

    let manifest = try!(find_project_manifest_exact(path, "Cargo.toml"));

    let (pkg, nested) = try!(read_package(&manifest, source_id, config));
    all_packages.insert(pkg);

    // Registry sources are not allowed to have `path=` dependencies because
    // they're all translated to actual registry dependencies.
    if !source_id.is_registry() {
        for p in nested.iter() {
            try!(read_nested_packages(&path.join(p), all_packages, source_id,
                                      config, visited));
        }
    }

    Ok(())
}
