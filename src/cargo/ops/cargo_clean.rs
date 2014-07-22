use std::io::fs::{rmdir_recursive};

use core::source::Source;
use sources::PathSource;
use util::{CargoResult, human, ChainError};

/// Cleans the project from build artifacts.

pub fn clean(manifest_path: &Path) -> CargoResult<()> {
    let mut src = PathSource::for_path(&manifest_path.dir_path());
    try!(src.update());
    let root = try!(src.get_root_package());
    let manifest = root.get_manifest();

    let build_dir = manifest.get_target_dir();
    if build_dir.exists() {
        try!(rmdir_recursive(build_dir).chain_error(|| {
            human("Could not remove build directory")
        }))
    }

    let doc_dir = manifest.get_doc_dir();
    if doc_dir.exists() {
        try!(rmdir_recursive(doc_dir).chain_error(|| {
            human("Could not remove documentation directory")
        }))
    }

    Ok(())
}
