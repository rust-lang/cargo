use std::env;
use std::old_io::fs::PathExtensions;
use util::{CargoResult, human, ChainError};

/// Iteratively search for `file` in `pwd` and its parents, returning
/// the path of the directory.
pub fn find_project(pwd: &Path, file: &str) -> CargoResult<Path> {
    find_project_manifest(pwd, file).map(|mut p| {
        // remove the file, leaving just the directory
        p.pop();
        p
    })
}

/// Iteratively search for `file` in `pwd` and its parents, returning
/// the path to the file.
pub fn find_project_manifest(pwd: &Path, file: &str) -> CargoResult<Path> {
    let mut current = pwd.clone();

    loop {
        let manifest = current.join(file);
        if manifest.exists() {
            return Ok(manifest)
        }

        if !current.pop() { break; }
    }

    Err(human(format!("Could not find `{}` in `{}` or any parent directory",
                      file, pwd.display())))
}

/// Find the root Cargo.toml
pub fn find_root_manifest_for_cwd(manifest_path: Option<String>)
                                  -> CargoResult<Path> {
    let cwd = try!(env::current_dir().chain_error(|| {
        human("Couldn't determine the current working directory")
    }));
    match manifest_path {
        Some(path) => Ok(cwd.join(path)),
        None => find_project_manifest(&cwd, "Cargo.toml"),
    }
}

/// Return the path to the `file` in `pwd`, if it exists.
pub fn find_project_manifest_exact(pwd: &Path, file: &str) -> CargoResult<Path> {
    let manifest = pwd.join(file);

    if manifest.exists() {
        Ok(manifest)
    } else {
        Err(human(format!("Could not find `{}` in `{}`",
                          file, pwd.display())))
    }
}
