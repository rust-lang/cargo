use std::collections::HashSet;
use std::io::{File, fs};
use std::io::fs::PathExtensions;

use core::{Package,Manifest,SourceId};
use util::{mod, CargoResult, human};
use util::important_paths::find_project_manifest_exact;
use util::toml::{Layout, project_layout};

pub fn read_manifest(contents: &[u8], layout: Layout, source_id: &SourceId)
    -> CargoResult<(Manifest, Vec<Path>)> {
    util::toml::to_manifest(contents, source_id, layout).map_err(human)
}

pub fn read_package(path: &Path, source_id: &SourceId)
    -> CargoResult<(Package, Vec<Path>)> {
    log!(5, "read_package; path={}; source-id={}", path.display(), source_id);
    let mut file = try!(File::open(path));
    let data = try!(file.read_to_end());

    let layout = project_layout(&path.dir_path());
    let (manifest, nested) =
        try!(read_manifest(data.as_slice(), layout, source_id));

    Ok((Package::new(manifest, path, source_id), nested))
}

pub fn read_packages(path: &Path,
                     source_id: &SourceId) -> CargoResult<Vec<Package>> {
    let mut all_packages = Vec::new();
    let mut visited = HashSet::<Path>::new();

    log!(5, "looking for root package: {}, source_id={}", path.display(), source_id);
    try!(process_possible_package(path, &mut all_packages, source_id, &mut visited));

    try!(walk(path, true, |root, dir| {
        log!(5, "looking for child package: {}", dir.display());
        if root && dir.join("target").is_dir() { return Ok(false); }
        if root { return Ok(true) }
        if dir.filename_str() == Some(".git") { return Ok(false); }
        if dir.join(".git").exists() { return Ok(false); }
        try!(process_possible_package(dir, &mut all_packages, source_id,
                                      &mut visited));
        Ok(true)
    }));

    if all_packages.is_empty() {
        Err(human(format!("Could not find Cargo.toml in `{}`", path.display())))
    } else {
        log!(5, "all packages: {}", all_packages);
        Ok(all_packages)
    }
}

fn walk(path: &Path, is_root: bool,
        callback: |bool, &Path| -> CargoResult<bool>) -> CargoResult<()> {
    if path.is_dir() {
        let continues = try!(callback(is_root, path));
        if !continues {
            log!(5, "not processing {}", path.display());
            return Ok(());
        }

        for dir in try!(fs::readdir(path)).iter() {
            try!(walk(dir, false, |a, x| callback(a, x)))
        }
    }

    Ok(())
}

fn process_possible_package(dir: &Path,
                            all_packages: &mut Vec<Package>,
                            source_id: &SourceId,
                            visited: &mut HashSet<Path>) -> CargoResult<()> {

    if !has_manifest(dir) { return Ok(()); }

    let packages = try!(read_nested_packages(dir, source_id, visited));
    push_all(all_packages, packages);

    Ok(())
}

fn has_manifest(path: &Path) -> bool {
    find_project_manifest_exact(path, "Cargo.toml").is_ok()
}

fn read_nested_packages(path: &Path, source_id: &SourceId,
                 visited: &mut HashSet<Path>) -> CargoResult<Vec<Package>> {
    if !visited.insert(path.clone()) { return Ok(Vec::new()) }

    let manifest = try!(find_project_manifest_exact(path, "Cargo.toml"));

    let (pkg, nested) = try!(read_package(&manifest, source_id));
    let mut ret = vec![pkg];

    for p in nested.iter() {
        ret.extend(try!(read_nested_packages(&path.join(p),
                                        source_id,
                                        visited)).into_iter());
    }

    Ok(ret)
}

fn push_all(set: &mut Vec<Package>, packages: Vec<Package>) {
    for package in packages.into_iter() {
        if set.contains(&package) { continue; }

        set.push(package)
    }
}
