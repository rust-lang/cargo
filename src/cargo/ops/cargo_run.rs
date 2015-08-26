use std::path::Path;

use ops::{self, ExecEngine, CompileFilter};
use util::{self, CargoResult, human, process, ProcessError};
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

    let mut bins = root.manifest().targets().iter().filter(|a| {
        !a.is_lib() && !a.is_custom_build() && match options.filter {
            CompileFilter::Everything => a.is_bin(),
            CompileFilter::Only { .. } => options.filter.matches(a),
        }
    });
    if bins.next().is_none() {
        match options.filter {
            CompileFilter::Everything => {
                return Err(human("a bin target must be available for \
                                  `cargo run`"))
            }
            CompileFilter::Only { .. } => {
                // this will be verified in cargo_compile
            }
        }
    }
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
    let exe = match util::without_prefix(&exe, config.cwd()) {
        Some(path) => if path.as_os_str() == path.file_name().unwrap() { &**exe } 
                      else { path }, // workaround for being able to use
                                     // `cargo run` from the executable path itself
        None => &**exe,
    };
    let mut process = try!(compile.target_process(exe, &root))
                                  .into_process_builder();
    process.args(args).cwd(config.cwd());

    try!(config.shell().status("Running", process.to_string()));
    Ok(process.exec().err())
}
