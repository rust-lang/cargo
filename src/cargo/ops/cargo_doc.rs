use std::collections::HashMap;
use std::fs;
use std::path::Path;

use failure::Fail;
use opener;

use crate::core::Workspace;
use crate::ops;
use crate::util::CargoResult;

/// Strongly typed options for the `cargo doc` command.
#[derive(Debug)]
pub struct DocOptions<'a> {
    /// Whether to attempt to open the browser after compiling the docs
    pub open_result: bool,
    /// Options to pass through to the compiler
    pub compile_opts: ops::CompileOptions<'a>,
}

/// Main method for `cargo doc`.
pub fn doc(ws: &Workspace<'_>, options: &DocOptions<'_>) -> CargoResult<()> {
    let specs = options.compile_opts.spec.to_package_id_specs(ws)?;
    let resolve = ops::resolve_ws_precisely(
        ws,
        &options.compile_opts.features,
        options.compile_opts.all_features,
        options.compile_opts.no_default_features,
        &specs,
    )?;
    let (packages, resolve_with_overrides) = resolve;

    let ids = specs
        .iter()
        .map(|s| s.query(resolve_with_overrides.iter()))
        .collect::<CargoResult<Vec<_>>>()?;
    let pkgs = packages.get_many(ids)?;

    let mut lib_names = HashMap::new();
    let mut bin_names = HashMap::new();
    let mut names = Vec::new();
    for package in &pkgs {
        for target in package.targets().iter().filter(|t| t.documented()) {
            if target.is_lib() {
                if let Some(prev) = lib_names.insert(target.crate_name(), package) {
                    failure::bail!(
                        "The library `{}` is specified by packages `{}` and \
                         `{}` but can only be documented once. Consider renaming \
                         or marking one of the targets as `doc = false`.",
                        target.crate_name(),
                        prev,
                        package
                    );
                }
            } else if let Some(prev) = bin_names.insert(target.crate_name(), package) {
                failure::bail!(
                    "The binary `{}` is specified by packages `{}` and \
                     `{}` but can be documented only once. Consider renaming \
                     or marking one of the targets as `doc = false`.",
                    target.crate_name(),
                    prev,
                    package
                );
            }
            names.push(target.crate_name());
        }
    }

    ops::compile(ws, &options.compile_opts)?;

    if options.open_result {
        let name = match names.first() {
            Some(s) => s.to_string(),
            None => return Ok(()),
        };

        // Don't bother locking here as if this is getting deleted there's
        // nothing we can do about it and otherwise if it's getting overwritten
        // then that's also ok!
        let mut target_dir = ws.target_dir();
        if let Some(ref triple) = options.compile_opts.build_config.requested_target {
            target_dir.push(Path::new(triple).file_stem().unwrap());
        }
        let path = target_dir.join("doc").join(&name).join("index.html");
        let path = path.into_path_unlocked();
        if fs::metadata(&path).is_ok() {
            let mut shell = options.compile_opts.config.shell();
            shell.status("Opening", path.display())?;
            if let Err(e) = opener::open(&path) {
                shell.warn(format!("Couldn't open docs: {}", e))?;
                for cause in (&e as &dyn Fail).iter_chain() {
                    shell.warn(format!("Caused by:\n {}", cause))?;
                }
            }
        }
    }

    Ok(())
}
