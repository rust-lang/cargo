use std::path::Path;

use ops::{self, Packages};
use util::{self, CargoResult, CargoError, ProcessError};
use util::errors::CargoErrorKind;
use core::Workspace;

pub fn run(ws: &Workspace,
           options: &ops::CompileOptions,
           args: &[String]) -> CargoResult<Option<ProcessError>> {
    let config = ws.config();

    let pkg = match options.spec {
        Packages::All => unreachable!("cargo run supports single package only"),
        Packages::OptOut(_) => unreachable!("cargo run supports single package only"),
        Packages::Packages(xs) => match xs.len() {
            0 => ws.current()?,
            1 => ws.members()
                .find(|pkg| pkg.name() == xs[0])
                .ok_or_else(||
                    CargoError::from(
                        format!("package `{}` is not a member of the workspace", xs[0]))
                )?,
            _ => unreachable!("cargo run supports single package only"),
        }
    };

    let bins: Vec<_> = pkg.manifest().targets().iter().filter(|a| {
        !a.is_lib() && !a.is_custom_build() && if !options.filter.is_specific() {
            a.is_bin()
        } else {
            options.filter.matches(a)
        }
    })
    .map(|bin| bin.name())
    .collect();

    if bins.len() == 0 {
        if !options.filter.is_specific() {
            bail!("a bin target must be available for `cargo run`")
        } else {
            // this will be verified in cargo_compile
        }
    }
    if bins.len() > 1 {
        if !options.filter.is_specific() {
            bail!("`cargo run` requires that a project only have one \
                   executable; use the `--bin` option to specify which one \
                   to run\navailable binaries: {}", bins.join(", "))
        } else {
            bail!("`cargo run` can run at most one executable, but \
                   multiple were specified")
        }
    }

    let compile = ops::compile(ws, options)?;
    assert_eq!(compile.binaries.len(), 1);
    let exe = &compile.binaries[0];
    let exe = match util::without_prefix(exe, config.cwd()) {
        Some(path) if path.file_name() == Some(path.as_os_str())
                   => Path::new(".").join(path).to_path_buf(),
        Some(path) => path.to_path_buf(),
        None => exe.to_path_buf(),
    };
    let mut process = compile.target_process(exe, pkg)?;
    process.args(args).cwd(config.cwd());

    config.shell().status("Running", process.to_string())?;

    let result = process.exec_replace();

    match result {
        Ok(()) => Ok(None),
        Err(CargoError(CargoErrorKind::ProcessErrorKind(e), ..)) => Ok(Some(e)),
        Err(e) => Err(e)
    }
}
