use std::io::fs;

use ops;
use util::CargoResult;
use core::source::Source;
use sources::PathSource;

pub struct DocOptions<'a> {
    pub all: bool,
    pub compile_opts: ops::CompileOptions<'a>,
}

pub fn doc(manifest_path: &Path,
           options: &mut DocOptions) -> CargoResult<()> {
    let mut src = PathSource::for_path(&manifest_path.dir_path());
    try!(src.update());
    let root = try!(src.get_root_package());
    let output = root.get_manifest().get_target_dir().join("doc");
    let _ = fs::rmdir_recursive(&output);
    try!(ops::compile(manifest_path, &mut options.compile_opts));
    Ok(())
}
