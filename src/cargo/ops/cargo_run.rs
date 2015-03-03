use std::path::Path;

use ops::{self, ExecEngine};
use util::{CargoResult, human, process, ProcessError, ChainError};
use core::manifest::TargetKind;
use core::source::Source;
use sources::PathSource;

pub fn run(manifest_path: &Path,
           target_kind: TargetKind,
           name: Option<String>,
           options: &ops::CompileOptions,
           args: &[String]) -> CargoResult<Option<ProcessError>> {
    let config = options.config;
    let mut src = try!(PathSource::for_path(&manifest_path.parent().unwrap(),
                                            config));
    try!(src.update());
    let root = try!(src.root_package());
    let env = options.env;
    let mut bins = root.manifest().targets().iter().filter(|a| {
        let matches_kind = match target_kind {
            TargetKind::Bin => a.is_bin(),
            TargetKind::Example => a.is_example(),
            TargetKind::Lib(_) => false,
        };
        let matches_name = name.as_ref().map_or(true, |n| *n == a.name());
        matches_kind && matches_name && a.profile().env() == env &&
            !a.profile().is_custom_build()
    });
    let bin = try!(bins.next().chain_error(|| {
        match (name.as_ref(), &target_kind) {
            (Some(name), &TargetKind::Bin) => {
                human(format!("no bin target named `{}` to run", name))
            }
            (Some(name), &TargetKind::Example) => {
                human(format!("no example target named `{}` to run", name))
            }
            (Some(_), &TargetKind::Lib(..)) => unreachable!(),
            (None, _) => human("a bin target must be available for `cargo run`"),
        }
    }));
    match bins.next() {
        Some(..) => return Err(
            human("`cargo run` requires that a project only have one executable. \
                   Use the `--bin` option to specify which one to run")),
        None => {}
    }

    let compile = try!(ops::compile(manifest_path, options));
    let dst = manifest_path.parent().unwrap().join("target");
    let dst = match options.target {
        Some(target) => dst.join(target),
        None => dst,
    };
    let exe = match (bin.profile().dest(), bin.is_example()) {
        (s, true) => dst.join(s).join("examples").join(bin.name()),
        (s, false) => dst.join(s).join(bin.name()),
    };
    let exe = match exe.relative_from(config.cwd()) {
        Some(path) => path,
        None => &*exe,
    };
    let mut process = try!(compile.target_process(exe, &root))
                                  .into_process_builder();
    process.args(args).cwd(config.cwd());

    try!(config.shell().status("Running", process.to_string()));
    Ok(process.exec().err())
}
