use std::path::Path;

use ops::{self, CompileFilter, Packages};
use util::{self, human, CargoResult, ProcessError};
use core::Workspace;

pub fn run(ws: &Workspace,
           options: &ops::CompileOptions,
           args: &[String]) -> CargoResult<Option<ProcessError>> {
    let config = ws.config();

    let pkg = match options.spec {
        Packages::All => unreachable!("cargo run supports single package only"),
        Packages::Packages(xs) => match xs.len() {
            0 => ws.current()?,
            1 => ws.members()
                .find(|pkg| pkg.name() == xs[0])
                .ok_or_else(|| human(
                    format!("package `{}` is not a member of the workspace", xs[0])
                ))?,
            _ => unreachable!("cargo run supports single package only"),
        }
    };

    let mut bins = pkg.manifest().targets().iter().filter(|a| {
        !a.is_lib() && !a.is_custom_build() && match options.filter {
            CompileFilter::Everything { .. } => a.is_bin(),
            CompileFilter::Only { .. } => options.filter.matches(a),
        }
    });
    if bins.next().is_none() {
        match options.filter {
            CompileFilter::Everything { .. } => {
                bail!("a bin target must be available for `cargo run`")
            }
            CompileFilter::Only { .. } => {
                // this will be verified in cargo_compile
            }
        }
    }
    if bins.next().is_some() {
        match options.filter {
            CompileFilter::Everything { .. } => {
                bail!("`cargo run` requires that a project only have one \
                       executable; use the `--bin` option to specify which one \
                       to run")
            }
            CompileFilter::Only { .. } => {
                bail!("`cargo run` can run at most one executable, but \
                       multiple were specified")
            }
        }
    }

    let compile = ops::compile(ws, options)?;
    assert_eq!(compile.binaries.len(), 1);
    let exe = &compile.binaries[0];
    let exe = match util::without_prefix(&exe, config.cwd()) {
        Some(path) if path.file_name() == Some(path.as_os_str())
                   => Path::new(".").join(path).to_path_buf(),
        Some(path) => path.to_path_buf(),
        None => exe.to_path_buf(),
    };
    let mut process = compile.target_process(exe, &pkg)?;
    process.args(args).cwd(config.cwd());

    config.shell().status("Running", process.to_string())?;
    Ok(process.exec_replace().err())
}
