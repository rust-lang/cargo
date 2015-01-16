use std::os;
use std::io::fs::PathExtensions;
use util::{CargoResult, human, ChainError};

/// Iteratively search for `file` in `pwd` and its parents, returning
/// the path of the directory.
pub fn find_project(pwd: &Path, file: &str) -> CargoResult<Path> {
    find_project_manifest(pwd, file)
        .map(|mut p| {
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

    Err(human(format!("could not find `{}` in `{}` or any parent directory",
                      file, pwd.display())))
}

/// Find the root Cargo.toml
pub fn find_root_manifest_for_cwd(manifest_path: Option<String>)
                                  -> CargoResult<Path> {
    match manifest_path {
        Some(s) => {
            let path = Path::new(s);
            if path.filename() != Some(b"Cargo.toml") {
                return Err(human("the manifest-path must be a path to a \
                                  Cargo.toml file"))
            }
            let path = try!(os::make_absolute(&path).chain_error(|| {
                human("could not determine the absolute path of the manifest")
            }));
            if !path.exists() {
                return Err(human(format!("manifest path `{}` does not exist",
                                         path.display())))
            }
            Ok(path)
        }
        None => {
            os::getcwd().chain_error(|| {
                human("couldn't determine the current working directory")
            }).and_then(|cwd| {
                find_project_manifest(&cwd, "Cargo.toml")
            })
        }
    }
}

/// Return the path to the `file` in `pwd`, if it exists.
pub fn find_project_manifest_exact(pwd: &Path, file: &str) -> CargoResult<Path> {
    let manifest = pwd.join(file);

    if manifest.exists() {
        Ok(manifest)
    } else {
        Err(human(format!("could not find `{}` in `{}`", file, pwd.display())))
    }
}
