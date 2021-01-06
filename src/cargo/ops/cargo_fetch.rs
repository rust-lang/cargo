use crate::core::compiler::{BuildConfig, CompileMode, RustcTargetData};
use crate::core::{PackageSet, Resolve, Workspace};
use crate::ops;
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
    let (packages, resolve) = ops::resolve_ws(ws)?;

    let jobs = Some(1);
    let config = ws.config();
    let build_config = BuildConfig::new(config, jobs, &options.targets, CompileMode::Build)?;
    let data = RustcTargetData::new(ws, &build_config.requested_kinds)?;
    let mut fetched_packages = HashSet::new();
    let mut deps_to_fetch = ws.members().map(|p| p.package_id()).collect::<Vec<_>>();
    let mut to_download = Vec::new();

    while let Some(id) = deps_to_fetch.pop() {
        if !fetched_packages.insert(id) {
            continue;
        }

        to_download.push(id);

        for (id, deps) in resolve.deps(id) {
            let mut accepted = false;
            'accepting_loop: for d in deps {
                // If no target was specified then all dependencies are
                // fetched.
                if options.targets.is_empty() {
                    accepted = true;
                    break;
                }

                // Otherwise we only download this dependency if any of the
                // requested platforms would match this dependency. Note
                // that this is a bit lossy because not all dependencies are
                // always compiled for all platforms, but it should be
                // "close enough" for now.
                for kind in build_config.requested_kinds {
                    if data.dep_platform_activated(d, kind)? {
                        accepted = true;
                        break 'accepting_loop;
                    }
                }
            }
            if accepted {
                deps_to_fetch.push(id);
            }
        }
    }
    packages.get_many(to_download)?;

    Ok((resolve, packages))
}
