use std::ffi::OsString;
use std::iter;
use std::path::Path;

use crate::core::{TargetKind, Workspace};
use crate::ops;
use crate::util::{CargoResult, ProcessError};

pub fn run(
    ws: &Workspace<'_>,
    options: &ops::CompileOptions,
    args: &[OsString],
) -> CargoResult<Option<ProcessError>> {
    let config = ws.config();

    // We compute the `bins` here *just for diagnosis*. The actual set of
    // packages to be run is determined by the `ops::compile` call below.
    let packages = options.spec.get_packages(ws)?;
    let bins: Vec<_> = packages
        .into_iter()
        .flat_map(|pkg| {
            iter::repeat(pkg).zip(pkg.manifest().targets().iter().filter(|target| {
                !target.is_lib()
                    && !target.is_custom_build()
                    && if !options.filter.is_specific() {
                        target.is_bin()
                    } else {
                        options.filter.target_run(target)
                    }
            }))
        })
        .collect();

    if bins.is_empty() {
        if !options.filter.is_specific() {
            anyhow::bail!("a bin target must be available for `cargo run`")
        } else {
            // This will be verified in `cargo_compile`.
        }
    }

    if bins.len() == 1 {
        let target = bins[0].1;
        if let TargetKind::ExampleLib(..) = target.kind() {
            anyhow::bail!(
                "example target `{}` is a library and cannot be executed",
                target.name()
            )
        }
    }

    if bins.len() > 1 {
        if !options.filter.is_specific() {
            let names: Vec<&str> = bins
                .into_iter()
                .map(|(_pkg, target)| target.name())
                .collect();
            anyhow::bail!(
                "`cargo run` could not determine which binary to run. \
                 Use the `--bin` option to specify a binary, \
                 or the `default-run` manifest key.\n\
                 available binaries: {}",
                names.join(", ")
            )
        } else {
            anyhow::bail!(
                "`cargo run` can run at most one executable, but \
                 multiple were specified"
            )
        }
    }

    // `cargo run` is only compatible with one `--target` flag at most
    options.build_config.single_requested_kind()?;

    let compile = ops::compile(ws, options)?;
    assert_eq!(compile.binaries.len(), 1);
    let (unit, exe) = &compile.binaries[0];
    let exe = match exe.strip_prefix(config.cwd()) {
        Ok(path) if path.file_name() == Some(path.as_os_str()) => Path::new(".").join(path),
        Ok(path) => path.to_path_buf(),
        Err(_) => exe.to_path_buf(),
    };
    let pkg = bins[0].0;
    let mut process = compile.target_process(exe, unit.kind, pkg)?;
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
