use std::collections::{HashMap, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::core::{EitherManifest, Package, PackageId, SourceId};
use crate::util::errors::CargoResult;
use crate::util::important_paths::find_project_manifest_exact;
use crate::util::toml::read_manifest;
use crate::util::Config;
use cargo_util::paths;
use tracing::{info, trace};

pub fn read_package(
    path: &Path,
    source_id: SourceId,
    config: &Config,
) -> CargoResult<(Package, Vec<PathBuf>)> {
    trace!(
        "read_package; path={}; source-id={}",
        path.display(),
        source_id
    );
    let (manifest, nested) = read_manifest(path, source_id, config)?;
    let manifest = match manifest {
        EitherManifest::Real(manifest) => manifest,
        EitherManifest::Virtual(..) => anyhow::bail!(
            "found a virtual manifest at `{}` instead of a package \
             manifest",
            path.display()
        ),
    };

    Ok((Package::new(manifest, path), nested))
}

pub fn read_packages(
    path: &Path,
    source_id: SourceId,
    config: &Config,
) -> CargoResult<Vec<Package>> {
    let mut all_packages = HashMap::new();
    let mut visited = HashSet::<PathBuf>::new();
    let mut errors = Vec::<anyhow::Error>::new();

    trace!(
        "looking for root package: {}, source_id={}",
        path.display(),
        source_id
    );

    walk(path, &mut |dir| {
        trace!("looking for child package: {}", dir.display());

        // Don't recurse into hidden/dot directories unless we're at the toplevel
        if dir != path {
            let name = dir.file_name().and_then(|s| s.to_str());
            if name.map(|s| s.starts_with('.')) == Some(true) {
                return Ok(false);
            }

            // Don't automatically discover packages across git submodules
            if dir.join(".git").exists() {
                return Ok(false);
            }
        }

        // Don't ever look at target directories
        if dir.file_name().and_then(|s| s.to_str()) == Some("target")
            && has_manifest(dir.parent().unwrap())
        {
            return Ok(false);
        }

        if has_manifest(dir) {
            read_nested_packages(
                dir,
                &mut all_packages,
                source_id,
                config,
                &mut visited,
                &mut errors,
            )?;
        }
        Ok(true)
    })?;

    if all_packages.is_empty() {
        match errors.pop() {
            Some(err) => Err(err),
            None => {
                if find_project_manifest_exact(path, "cargo.toml").is_ok() {
                    Err(anyhow::format_err!(
                "Could not find Cargo.toml in `{}`, but found cargo.toml please try to rename it to Cargo.toml",
                path.display()
            ))
                } else {
                    Err(anyhow::format_err!(
                        "Could not find Cargo.toml in `{}`",
                        path.display()
                    ))
                }
            }
        }
    } else {
        Ok(all_packages.into_iter().map(|(_, v)| v).collect())
    }
}

fn walk(path: &Path, callback: &mut dyn FnMut(&Path) -> CargoResult<bool>) -> CargoResult<()> {
    if !callback(path)? {
        trace!("not processing {}", path.display());
        return Ok(());
    }

    // Ignore any permission denied errors because temporary directories
    // can often have some weird permissions on them.
    let dirs = match fs::read_dir(path) {
        Ok(dirs) => dirs,
        Err(ref e) if e.kind() == io::ErrorKind::PermissionDenied => return Ok(()),
        Err(e) => {
            let cx = format!("failed to read directory `{}`", path.display());
            let e = anyhow::Error::from(e);
            return Err(e.context(cx));
        }
    };
    for dir in dirs {
        let dir = dir?;
        if dir.file_type()?.is_dir() {
            walk(&dir.path(), callback)?;
        }
    }
    Ok(())
}

fn has_manifest(path: &Path) -> bool {
    find_project_manifest_exact(path, "Cargo.toml").is_ok()
}

fn read_nested_packages(
    path: &Path,
    all_packages: &mut HashMap<PackageId, Package>,
    source_id: SourceId,
    config: &Config,
    visited: &mut HashSet<PathBuf>,
    errors: &mut Vec<anyhow::Error>,
) -> CargoResult<()> {
    if !visited.insert(path.to_path_buf()) {
        return Ok(());
    }

    let manifest_path = find_project_manifest_exact(path, "Cargo.toml")?;

    let (manifest, nested) = match read_manifest(&manifest_path, source_id, config) {
        Err(err) => {
            // Ignore malformed manifests found on git repositories
            //
            // git source try to find and read all manifests from the repository
            // but since it's not possible to exclude folders from this search
            // it's safer to ignore malformed manifests to avoid
            //
            // TODO: Add a way to exclude folders?
            info!(
                "skipping malformed package found at `{}`",
                path.to_string_lossy()
            );
            errors.push(err.into());
            return Ok(());
        }
        Ok(tuple) => tuple,
    };

    let manifest = match manifest {
        EitherManifest::Real(manifest) => manifest,
        EitherManifest::Virtual(..) => return Ok(()),
    };
    let pkg = Package::new(manifest, &manifest_path);

    let pkg_id = pkg.package_id();
    use std::collections::hash_map::Entry;
    match all_packages.entry(pkg_id) {
        Entry::Vacant(v) => {
            v.insert(pkg);
        }
        Entry::Occupied(_) => {
            // We can assume a package with publish = false isn't intended to be seen
            // by users so we can hide the warning about those since the user is unlikely
            // to care about those cases.
            if pkg.publish().is_none() {
                let _ = config.shell().warn(format!(
                    "skipping duplicate package `{}` found at `{}`",
                    pkg.name(),
                    path.display()
                ));
            }
        }
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
            let path = paths::normalize_path(&path.join(p));
            let result =
                read_nested_packages(&path, all_packages, source_id, config, visited, errors);
            // Ignore broken manifests found on git repositories.
            //
            // A well formed manifest might still fail to load due to reasons
            // like referring to a "path" that requires an extra build step.
            //
            // See https://github.com/rust-lang/cargo/issues/6822.
            if let Err(err) = result {
                if source_id.is_git() {
                    info!(
                        "skipping nested package found at `{}`: {:?}",
                        path.display(),
                        &err,
                    );
                    errors.push(err);
                } else {
                    return Err(err);
                }
            }
        }
    }

    Ok(())
}
