use core::compiler::{BuildConfig, Kind, TargetInfo};
use core::{Package, PackageId, PackageSet, Resolve, Workspace};
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

    fetch_for_target(ws, options.config, &options.target, &resolve, &packages)?;

    Ok((resolve, packages))
}

fn fetch_for_target<'a, 'cfg: 'a>(
    ws: &'a Workspace<'cfg>,
    config: &'cfg Config,
    target: &Option<String>,
    resolve: &'a Resolve,
    packages: &'a PackageSet<'cfg>,
) -> CargoResult<HashSet<&'a PackageId>> {
    let mut fetched_packages = HashSet::new();
    let mut deps_to_fetch = Vec::new();
    let jobs = Some(1);
    let build_config = BuildConfig::new(config, jobs, target, None)?;
    let target_info = TargetInfo::new(config, &build_config, Kind::Target)?;
    let root_package_ids = ws.members().map(Package::package_id).collect::<Vec<_>>();

    deps_to_fetch.extend(root_package_ids);

    while let Some(id) = deps_to_fetch.pop() {
        if !fetched_packages.insert(id) {
            continue;
        }

        let package = packages.get(id)?;
        let deps = resolve.deps(id);
        let dependency_ids = deps.filter(|dep| {
            package
                .dependencies()
                .iter()
                .filter(|d| d.name() == dep.name() && d.version_req().matches(dep.version()))
                .any(|d| {
                    // If no target was specified then all dependencies can be fetched.
                    let target = match *target {
                        Some(ref t) => t,
                        None => return true,
                    };
                    // If this dependency is only available for certain platforms,
                    // make sure we're only fetching it for that platform.
                    let platform = match d.platform() {
                        Some(p) => p,
                        None => return true,
                    };
                    platform.matches(target, target_info.cfg())
                })
        }).collect::<Vec<_>>();

        deps_to_fetch.extend(dependency_ids);
    }

    Ok(fetched_packages)
}
