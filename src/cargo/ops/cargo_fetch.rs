use core::compiler::{BuildConfig, CompileMode, Kind, TargetInfo};
use core::{PackageSet, Resolve, Workspace};
use ops;
use std::collections::HashSet;
use util::CargoResult;
use util::Config;

pub struct FetchOptions<'a> {
    pub config: &'a Config,
    /// The target arch triple to fetch dependencies for
    pub target: Option<String>,
}

/// Executes `cargo fetch`.
pub fn fetch<'a>(
    ws: &Workspace<'a>,
    options: &FetchOptions<'a>,
) -> CargoResult<(Resolve, PackageSet<'a>)> {
    let (packages, resolve) = ops::resolve_ws(ws)?;

    let jobs = Some(1);
    let config = ws.config();
    let build_config = BuildConfig::new(config, jobs, &options.target, CompileMode::Build)?;
    let rustc = config.rustc(Some(ws))?;
    let target_info =
        TargetInfo::new(config, &build_config.requested_target, &rustc, Kind::Target)?;
    {
        let mut fetched_packages = HashSet::new();
        let mut deps_to_fetch = ws.members().map(|p| p.package_id()).collect::<Vec<_>>();

        while let Some(id) = deps_to_fetch.pop() {
            if !fetched_packages.insert(id) {
                continue;
            }

            packages.get(id)?;
            let deps = resolve.deps(id)
                .filter(|&(_id, deps)| {
                    deps.iter()
                        .any(|d| {
                            // If no target was specified then all dependencies can
                            // be fetched.
                            let target = match options.target {
                                Some(ref t) => t,
                                None => return true,
                            };
                            // If this dependency is only available for certain
                            // platforms, make sure we're only fetching it for that
                            // platform.
                            let platform = match d.platform() {
                                Some(p) => p,
                                None => return true,
                            };
                            platform.matches(target, target_info.cfg())
                        })
                })
                .map(|(id, _deps)| id);
            deps_to_fetch.extend(deps);
        }
    }

    Ok((resolve, packages))
}
