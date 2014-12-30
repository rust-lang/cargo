use std::os;

use ops::{mod, ExecEngine};
use util::{CargoResult, human, process, ProcessError, ChainError};
use core::manifest::TargetKind;
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
            TargetKind::Bin => a.is_bin(),
            TargetKind::Example => a.is_example(),
            TargetKind::Lib(_) => false,
        };
        let matches_name = name.as_ref().map_or(true, |n| n.as_slice() == a.get_name());
        matches_kind && matches_name && a.get_profile().get_env() == env &&
            !a.get_profile().is_custom_build()
    });
    let bin = try!(bins.next().chain_error(|| {
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
    let exe = match exe.path_relative_from(&try!(os::getcwd())) {
        Some(path) => path,
        None => exe,
    };
    let process = try!(try!(compile.target_process(exe, &root))
                              .into_process_builder())
                              .args(args)
                              .cwd(try!(os::getcwd()));

    try!(options.shell.status("Running", process.to_string()));
    Ok(process.exec().err())
}
