use crate::core::compiler::BuildConfig;
use crate::core::compiler::RustcTargetData;
use crate::core::compiler::UserIntent;
use crate::core::compiler::standard_lib;
use crate::core::{PackageSet, Resolve, Workspace};
use crate::ops;
use crate::util::CargoResult;
use crate::util::GlobalContext;
use crate::util::context::JobsConfig;
use std::collections::HashSet;

pub struct FetchOptions<'a> {
    pub gctx: &'a GlobalContext,
    /// The target arch triple to fetch dependencies for
    pub targets: Vec<String>,
}

/// Executes `cargo fetch`.
pub fn fetch<'a>(
    ws: &Workspace<'a>,
    options: &FetchOptions<'a>,
) -> CargoResult<(Resolve, PackageSet<'a>)> {
    ws.emit_warnings()?;
    let dry_run = false;
    let (mut packages, resolve) = ops::resolve_ws(ws, dry_run)?;

    let jobs = Some(JobsConfig::Integer(1));
    let keep_going = false;
    let gctx = ws.gctx();
    let build_config =
        BuildConfig::new(gctx, jobs, keep_going, &options.targets, UserIntent::Build)?;
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
    if let Some(crates) = &gctx.cli_unstable().build_std {
        let (std_package_set, _, _) = standard_lib::resolve_std(
            ws,
            &mut data,
            &build_config,
            crates,
            &build_config.requested_kinds,
        )?;
        packages.add_set(std_package_set);
    }

    packages.get_many(to_download)?;
    crate::core::gc::auto_gc(gctx);

    Ok((resolve, packages))
}
