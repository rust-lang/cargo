use crate::util::errors::CargoResult;
use crate::util::paths;
use std::path::{Path, PathBuf};

/// Finds the root `Cargo.toml`.
pub fn find_root_manifest_for_wd(cwd: &Path) -> CargoResult<PathBuf> {
    let file = "Cargo.toml";
    for current in paths::ancestors(cwd, None) {
        let manifest = current.join(file);
        if manifest.exists() {
            return Ok(manifest);
        }
    }

    anyhow::bail!(
        "could not find `{}` in `{}` or any parent directory",
        file,
        cwd.display()
    )
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
