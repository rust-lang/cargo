use crate::core::compiler::standard_lib;
use crate::core::compiler::{BuildConfig, CompileMode, RustcTargetData};
use crate::core::{PackageSet, Resolve, Workspace};
use crate::ops;
use crate::util::config::JobsConfig;
use crate::util::CargoResult;
use crate::util::Config;
use std::collections::HashSet;

pub struct FetchOptions<'a> {
    pub config: &'a Config,
    /// The target arch triple to fetch dependencies for
    pub targets: Vec<String>,
}

/// Executes `cargo fetch`.
pub fn fetch<'a>(
    ws: &Workspace<'a>,
    options: &FetchOptions<'a>,
) -> CargoResult<(Resolve, PackageSet<'a>)> {
    ws.emit_warnings()?;
    let (mut packages, resolve) = ops::resolve_ws(ws)?;

    let jobs = Some(JobsConfig::Integer(1));
    let keep_going = false;
    let config = ws.config();
    let build_config = BuildConfig::new(
        config,
        jobs,
        keep_going,
        &options.targets,
        CompileMode::Build,
    )?;
    let mut data = RustcTargetData::new(ws, &build_config.requested_kinds)?;
    let mut fetched_packages = HashSet::new();
    let mut deps_to_fetch = ws.members().map(|p| p.package_id()).collect::<Vec<_>>();
    let mut to_download = Vec::new();

    while let Some(id) = deps_to_fetch.pop() {
        if !fetched_packages.insert(id) {
            continue;
        }

        to_download.push(id);
        let deps = resolve
            .deps(id)
            .filter(|&(_id, deps)| {
                deps.iter().any(|d| {
                    // If no target was specified then all dependencies are
                    // fetched.
                    if options.targets.is_empty() {
                        return true;
                    }

                    // Otherwise we only download this dependency if any of the
                    // requested platforms would match this dependency. Note
                    // that this is a bit lossy because not all dependencies are
                    // always compiled for all platforms, but it should be
                    // "close enough" for now.
                    build_config
                        .requested_kinds
                        .iter()
                        .any(|kind| data.dep_platform_activated(d, *kind))
                })
            })
            .map(|(id, _deps)| id);
        deps_to_fetch.extend(deps);
    }

    // If -Zbuild-std was passed, download dependencies for the standard library.
    // We don't know ahead of time what jobs we'll be running, so tell `std_crates` that.
    if let Some(crates) = standard_lib::std_crates(config, None) {
        let (std_package_set, _, _) =
            standard_lib::resolve_std(ws, &mut data, &build_config, &crates)?;
        packages.add_set(std_package_set);
    }

    packages.get_many(to_download)?;

    Ok((resolve, packages))
}
