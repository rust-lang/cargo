use std::path::Path;

use ops::{self, ExecEngine, CompileFilter};
use util::{CargoResult, human, process, ProcessError};
use core::source::Source;
use sources::PathSource;

pub fn run(manifest_path: &Path,
           options: &ops::CompileOptions,
           args: &[String]) -> CargoResult<Option<ProcessError>> {
    let config = options.config;
    let mut src = try!(PathSource::for_path(&manifest_path.parent().unwrap(),
                                            config));
    try!(src.update());
    let root = try!(src.root_package());

    // Make sure that we're only running at most one binary. The `compile` step
    // will verify that we're buliding at least one binary, so we don't check
    // for that form of existence here.
    let mut bins = root.manifest().targets().iter().filter(|a| {
        options.filter.matches(a) && !a.is_lib() && !a.is_custom_build()
    });
    let _ = bins.next();
    if bins.next().is_some() {
        match options.filter {
            CompileFilter::Everything => {
                return Err(human("`cargo run` requires that a project only have \
                                  one executable; use the `--bin` option to \
                                  specify which one to run"))
            }
            CompileFilter::Only { .. } => {
                return Err(human("`cargo run` can run at most one executable, \
                                  but multiple were specified"))
            }
        }
    }

    let compile = try!(ops::compile(manifest_path, options));
    let exe = &compile.binaries[0];
    let exe = match exe.relative_from(config.cwd()) {
        Some(path) => path,
        None => &**exe,
    };
    let mut process = try!(compile.target_process(exe, &root))
                                  .into_process_builder();
    process.args(args).cwd(config.cwd());

    try!(config.shell().status("Running", process.to_string()));
    Ok(process.exec().err())
}
