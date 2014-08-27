use std::os;

use ops;
use util::{CargoResult, human, process, ProcessError, Require};
use core::source::Source;
use sources::PathSource;

pub fn run(manifest_path: &Path,
           options: &mut ops::CompileOptions,
           args: &[String]) -> CargoResult<Option<ProcessError>> {
    let mut src = try!(PathSource::for_path(&manifest_path.dir_path()));
    try!(src.update());
    let root = try!(src.get_root_package());
    let env = options.env;
    let mut bins = root.get_manifest().get_targets().iter().filter(|a| {
        a.is_bin() && a.get_profile().get_env() == env
    });
    let bin = try!(bins.next().require(|| {
        human("a bin target must be available for `cargo run`")
    }));
    match bins.next() {
        Some(..) => return Err(human("`cargo run` requires that a project only \
                                      have one executable")),
        None => {}
    }

    let compile = try!(ops::compile(manifest_path, options));
    let dst = manifest_path.dir_path().join("target");
    let dst = match options.target {
        Some(target) => dst.join(target),
        None => dst,
    };
    let exe = match bin.get_profile().get_dest() {
        Some(s) => dst.join(s).join(bin.get_name()),
        None => dst.join(bin.get_name()),
    };
    let exe = match exe.path_relative_from(&os::getcwd()) {
        Some(path) => path,
        None => exe,
    };
    let process = compile.process(exe, &root).args(args).cwd(os::getcwd());

    try!(options.shell.status("Running", process.to_string()));
    Ok(process.exec().err())
}
