use std::io::fs::{rmdir_recursive};
use core::{SourceId};
use util::{CargoResult, human, ChainError};
use ops::{read_manifest};
use std::io::{File};
use util::toml::{project_layout};

pub fn clean(path: &Path) -> CargoResult<()>
{
    let mut file = try!(File::open(path));
    let data = try!(file.read_to_end());
    let layout = project_layout(&path.dir_path());
    let (manifest, _) = try!(read_manifest(data.as_slice(), layout, &SourceId::for_path(path)));
    let build_dir = manifest.get_target_dir();

    if build_dir.exists() {
      return rmdir_recursive(build_dir).chain_error(|| human("Could not remove build directory"))
    }

    Ok(())
}
