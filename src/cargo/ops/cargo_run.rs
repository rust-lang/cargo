use std::path::Path;

use ops;
use util::{self, CargoResult, ProcessError};
use core::{TargetKind, Workspace};

pub fn run(
    ws: &Workspace,
    options: &ops::CompileOptions,
    args: &[String],
) -> CargoResult<Option<ProcessError>> {
    let config = ws.config();

    let pkg = options.get_package(ws)?
        .unwrap_or_else(|| unreachable!("cargo run supports single package only"));

    // We compute the `bins` here *just for diagnosis*.  The actual set of packages to be run
    // is determined by the `ops::compile` call below.
    let bins: Vec<_> = pkg.manifest()
        .targets()
        .iter()
        .filter(|a| {
            !a.is_lib() && !a.is_custom_build() && if !options.filter.is_specific() {
                a.is_bin()
            } else {
                options.filter.target_run(a)
            }
        })
        .map(|bin| (bin.name(), bin.kind()))
        .collect();

    if bins.is_empty() {
        if !options.filter.is_specific() {
            bail!("a bin target must be available for `cargo run`")
        } else {
            // this will be verified in cargo_compile
        }
    }

    if bins.len() == 1 {
        let &(name, kind) = bins.first().unwrap();
        match kind {
            &TargetKind::ExampleLib(..) => { 
                bail!(
                    "example target `{}` is a library and cannot be executed",
                    name
                ) 
            },
            _ => { }
        };
    }

    if bins.len() > 1 {
        if !options.filter.is_specific() {
            let names: Vec<&str> = bins.into_iter().map(|bin| bin.0).collect();
            bail!(
                "`cargo run` requires that a project only have one \
                 executable; use the `--bin` option to specify which one \
                 to run\navailable binaries: {}",
                 names.join(", ")
            )
        } else {
            bail!(
                "`cargo run` can run at most one executable, but \
                 multiple were specified"
            )
        }
    }

    let compile = ops::compile(ws, options)?;
    assert_eq!(compile.binaries.len(), 1);
    let exe = &compile.binaries[0];
    let exe = match util::without_prefix(exe, config.cwd()) {
        Some(path) if path.file_name() == Some(path.as_os_str()) => {
            Path::new(".").join(path).to_path_buf()
        }
        Some(path) => path.to_path_buf(),
        None => exe.to_path_buf(),
    };
    let mut process = compile.target_process(exe, pkg)?;
    process.args(args).cwd(config.cwd());

    config.shell().status("Running", process.to_string())?;

    let result = process.exec_replace();

    match result {
        Ok(()) => Ok(None),
        Err(e) => {
            let err = e.downcast::<ProcessError>()?;
            Ok(Some(err))
        }
    }
}
