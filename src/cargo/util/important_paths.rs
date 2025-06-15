use crate::util::errors::CargoResult;
use cargo_util::paths;
use std::path::{Path, PathBuf};

use super::GlobalContext;

/// Finds the root `Cargo.toml`.
pub fn find_root_manifest_for_wd(gctx: &GlobalContext, cwd: &Path) -> CargoResult<PathBuf> {
    let valid_cargo_toml_file_name = "Cargo.toml";
    let invalid_cargo_toml_file_name = "cargo.toml";
    let mut invalid_cargo_toml_path_exists = false;

    let search_route = gctx.find_package_manifest_search_route(cwd);
    for current in paths::ancestors(&search_route.start, search_route.root.as_deref()) {
        let manifest = current.join(valid_cargo_toml_file_name);
        if manifest.exists() {
            // In case we are running outside of any root directory, the directory for the
            // first root manifest we find will become the fallback root. This is part of
            // a safety trade-off that allows us to traverse unknown ancestors to find
            // a package, but limits the risk of continuing to traverse and load manifests
            // that we might not own (such as `/tmp/Cargo.toml`)
            gctx.ensure_fallback_root(current);
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
