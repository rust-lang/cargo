use std::os;

use ops;
use util::{CargoResult, human, process, ProcessError, Require};
use core::manifest::{TargetKind, LibTarget, BinTarget, ExampleTarget};
use core::source::Source;
use sources::PathSource;

pub fn run(manifest_path: &Path,
           target_kind: TargetKind,
           name: Option<String>,
           options: &mut ops::CompileOptions,
           args: &[String]) -> CargoResult<Option<ProcessError>> {
    let mut src = try!(PathSource::for_path(&manifest_path.dir_path()));
    try!(src.update());
    let root = try!(src.get_root_package());
    let env = options.env;
    let mut bins = root.get_manifest().get_targets().iter().filter(|a| {
        let matches_kind = match target_kind {
            BinTarget => a.is_bin(),
            ExampleTarget => a.is_example(),
            LibTarget(_) => false,
        };
        let matches_name = name.as_ref().map_or(true, |n| n.as_slice() == a.get_name());
        matches_kind && matches_name && a.get_profile().get_env() == env &&
            !a.get_profile().is_custom_build()
    });
    let bin = try!(bins.next().require(|| {
        human("a bin target must be available for `cargo run`")
    }));
    match bins.next() {
        Some(..) => return Err(
            human("`cargo run` requires that a project only have one executable. \
                   Use the `--bin` option to specify which one to run")),
        None => {}
    }

    let compile = try!(ops::compile(manifest_path, options));
    let dst = manifest_path.dir_path().join("target");
    let dst = match options.target {
        Some(target) => dst.join(target),
        None => if bin.is_example() { dst.join("examples") } else { dst },
    };
    let exe = match bin.get_profile().get_dest() {
        Some(s) => dst.join(s).join(bin.get_name()),
        None => dst.join(bin.get_name()),
    };
    let exe = match exe.path_relative_from(&os::getcwd()) {
        Some(path) => path,
        None => exe,
    };
    let process = try!(compile.process(exe, &root))
                              .args(args).cwd(os::getcwd());

    try!(options.shell.status("Running", process.to_string()));
    Ok(process.exec().err())
}
