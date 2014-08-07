use std::os;

use ops;
use util::{CargoResult, process, ProcessError};
use core::source::Source;
use sources::PathSource;

pub fn run(manifest_path: &Path,
           bin: Option<&str>,
           options: &mut ops::CompileOptions,
           args: &[String]) -> CargoResult<Option<ProcessError>> {

    let mut src = try!(PathSource::for_path(&manifest_path.dir_path()));
    try!(src.update());
    let root = try!(src.get_root_package());

    try!(ops::compile(manifest_path, options));

    let exe = manifest_path.dir_path().join("target").join(match bin {
        None => root.get_name(),
        Some(bin) => bin,
    });
    let exe = match exe.path_relative_from(&os::getcwd()) {
        Some(path) => path,
        None => exe,
    };
    let process = process(exe).args(args);

    try!(options.shell.status("Running", process.to_string()));
    Ok(process.exec().err())
}
