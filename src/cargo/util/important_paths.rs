use crate::util::errors::{self, CargoResult};
use crate::util::Config;
use cargo_util::paths;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Finds the root `Cargo.toml`.
pub fn find_root_manifest_for_wd(config: &Config) -> CargoResult<PathBuf> {
    let valid_cargo_toml_file_name = "Cargo.toml";
    let invalid_cargo_toml_file_name = "cargo.toml";
    let mut invalid_cargo_toml_path_exists = false;
    let safe_directories = config.safe_directories()?;
    let cwd = config.cwd();

    for current in paths::ancestors(cwd, None) {
        let manifest = current.join(valid_cargo_toml_file_name);
        if manifest.exists() {
            check_safe_manifest_path(config, &safe_directories, &manifest)?;
            return Ok(manifest);
        }
        if current.join(invalid_cargo_toml_file_name).exists() {
            invalid_cargo_toml_path_exists = true;
        }
    }

    if invalid_cargo_toml_path_exists {
        anyhow::bail!(
        "could not find `{}` in `{}` or any parent directory, but found cargo.toml please try to rename it to Cargo.toml",
        valid_cargo_toml_file_name,
        cwd.display()
    )
    } else {
        anyhow::bail!(
            "could not find `{}` in `{}` or any parent directory",
            valid_cargo_toml_file_name,
            cwd.display()
        )
    }
}

/// Returns the path to the `file` in `pwd`, if it exists.
pub fn find_project_manifest_exact(pwd: &Path, file: &str) -> CargoResult<PathBuf> {
    let manifest = pwd.join(file);

    if manifest.exists() {
        Ok(manifest)
    } else {
        anyhow::bail!("Could not find `{}` in `{}`", file, pwd.display())
    }
}

/// Checks whether or not the given manifest path is owned by a different user.
pub fn check_safe_manifest_path(
    config: &Config,
    safe_directories: &HashSet<PathBuf>,
    path: &Path,
) -> CargoResult<()> {
    if !config.safe_directories_enabled() {
        return Ok(());
    }
    paths::validate_ownership(path, safe_directories).map_err(|e| {
        match e.downcast_ref::<paths::OwnershipError>() {
            Some(e) => {
                let to_add = e.path.parent().unwrap();
                errors::ownership_error(e, "manifests", to_add, config)
            }
            None => e,
        }
    })
}
