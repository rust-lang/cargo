use std::os;

use ops;
use util::{CargoResult, human, process, ProcessError};
use core::source::Source;
use sources::PathSource;

pub fn run(manifest_path: &Path,
           options: &mut ops::CompileOptions,
           args: &[String]) -> CargoResult<Option<ProcessError>> {
    if !manifest_path.dir_path().join("src").join("main.rs").exists() {
        return Err(human("`src/main.rs` must be present for `cargo run`"))
    }

    let mut src = try!(PathSource::for_path(&manifest_path.dir_path()));
    try!(src.update());
    let root = try!(src.get_root_package());

    try!(ops::compile(manifest_path, options));
    let exe = manifest_path.dir_path().join("target").join(root.get_name());
    let exe = match exe.path_relative_from(&os::getcwd()) {
        Some(path) => path,
        None => exe,
    };
    let process = process(exe).args(args);

    try!(options.shell.status("Running", process.to_string()));
    Ok(process.exec().err())
}
