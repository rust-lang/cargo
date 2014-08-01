use core::Source;
use sources::PathSource;
use ops;
use util::{process, CargoResult, ProcessError};

pub fn run_tests(manifest_path: &Path,
                 options: &mut ops::CompileOptions,
                 args: &[String]) -> CargoResult<Option<ProcessError>> {
    let mut source = PathSource::for_path(&manifest_path.dir_path());
    try!(source.update());
    let package = try!(source.get_root_package());

    try!(ops::compile(manifest_path, options));

    let mut exes = package.get_targets().iter().filter_map(|target| {
        if !target.get_profile().is_test() { return None }
        let root = package.get_root().join("target");
        let root = match target.get_profile().get_dest() {
            Some(dest) => root.join(dest),
            None => root,
        };
        Some(root.join(target.file_stem()))
    });

    for exe in exes {
        match process(exe).args(args).exec() {
            Ok(()) => {}
            Err(e) => return Ok(Some(e))
        }
    }

    Ok(None)
}
