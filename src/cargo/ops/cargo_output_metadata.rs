use crate::core::compiler::{CompileKind, CompileTarget, TargetInfo};
use crate::core::resolver::{Resolve, ResolveOpts};
use crate::core::{Dependency, Package, PackageId, Workspace};
use crate::ops::{self, Packages};
use crate::util::CargoResult;
use cargo_platform::Cfg;
use serde::ser;
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;

const VERSION: u32 = 1;

pub struct OutputMetadataOptions {
    pub features: Vec<String>,
    pub no_default_features: bool,
    pub all_features: bool,
    pub no_deps: bool,
    pub version: u32,
    pub filter_platform: Option<String>,
}

/// Loads the manifest, resolves the dependencies of the package to the concrete
/// used versions - considering overrides - and writes all dependencies in a JSON
/// format to stdout.
pub fn output_metadata(ws: &Workspace<'_>, opt: &OutputMetadataOptions) -> CargoResult<ExportInfo> {
    if opt.version != VERSION {
        failure::bail!(
            "metadata version {} not supported, only {} is currently supported",
            opt.version,
            VERSION
        );
    }
    let (packages, resolve) = if opt.no_deps {
        let packages = ws.members().cloned().collect();
        (packages, None)
    } else {
        let specs = Packages::All.to_package_id_specs(ws)?;
        let opts = ResolveOpts::new(
            /*dev_deps*/ true,
            &opt.features,
            opt.all_features,
            !opt.no_default_features,
        );
        let ws_resolve = ops::resolve_ws_with_opts(ws, opts, &specs)?;
        let mut package_map = HashMap::new();
        for pkg in ws_resolve
            .pkg_set
            .get_many(ws_resolve.pkg_set.package_ids())?
        {
            package_map.insert(pkg.package_id(), pkg.clone());
        }
        let packages = package_map.values().map(|p| (*p).clone()).collect();
        let rustc = ws.config().load_global_rustc(Some(ws))?;
        let (target, cfg) = match &opt.filter_platform {
            Some(platform) => {
                if platform == "host" {
                    let ti =
                        TargetInfo::new(ws.config(), CompileKind::Host, &rustc, CompileKind::Host)?;
                    (
                        Some(rustc.host.as_str().to_string()),
                        Some(ti.cfg().iter().cloned().collect()),
                    )
                } else {
                    let kind = CompileKind::Target(CompileTarget::new(platform)?);
                    let ti = TargetInfo::new(ws.config(), kind, &rustc, kind)?;
                    (
                        Some(platform.clone()),
                        Some(ti.cfg().iter().cloned().collect()),
                    )
                }
            }
            None => (None, None),
        };
        let resolve = Some(MetadataResolve {
            helper: ResolveHelper {
                packages: package_map,
                resolve: ws_resolve.targeted_resolve,
                target,
                cfg,
            },
            root: ws.current_opt().map(|pkg| pkg.package_id()),
        });
        (packages, resolve)
    };

    Ok(ExportInfo {
        packages,
        workspace_members: ws.members().map(|pkg| pkg.package_id()).collect(),
        resolve,
        target_directory: ws.target_dir().into_path_unlocked(),
        version: VERSION,
        workspace_root: ws.root().to_path_buf(),
    })
}

#[derive(Serialize)]
pub struct ExportInfo {
    packages: Vec<Package>,
    workspace_members: Vec<PackageId>,
    resolve: Option<MetadataResolve>,
    target_directory: PathBuf,
    version: u32,
    workspace_root: PathBuf,
}

/// Newtype wrapper to provide a custom `Serialize` implementation.
/// The one from lock file does not fit because it uses a non-standard
/// format for `PackageId`s
#[derive(Serialize)]
struct MetadataResolve {
    #[serde(rename = "nodes", serialize_with = "serialize_resolve")]
    helper: ResolveHelper,
    root: Option<PackageId>,
}

struct ResolveHelper {
    packages: HashMap<PackageId, Package>,
    resolve: Resolve,
    target: Option<String>,
    cfg: Option<Vec<Cfg>>,
}

fn serialize_resolve<S>(helper: &ResolveHelper, s: S) -> Result<S::Ok, S::Error>
where
    S: ser::Serializer,
{
    let ResolveHelper {
        packages,
        resolve,
        target,
        cfg,
    } = helper;

    #[derive(Serialize)]
    struct Dep {
        name: String,
        pkg: PackageId,
    }

    #[derive(Serialize)]
    struct Node<'a> {
        id: PackageId,
        dependencies: Vec<PackageId>,
        deps: Vec<Dep>,
        features: Vec<&'a str>,
    }

    // A filter for removing platform dependencies.
    let dep_filter = |(_pkg, deps): &(PackageId, &[Dependency])| match (target, cfg) {
        (Some(target), Some(cfg)) => deps.iter().any(|dep| {
            let platform = match dep.platform() {
                Some(p) => p,
                None => return true,
            };
            platform.matches(target, cfg)
        }),
        (None, None) => true,
        _ => unreachable!(),
    };

    s.collect_seq(resolve.iter().map(|id| {
        Node {
            id,
            dependencies: resolve
                .deps(id)
                .filter(dep_filter)
                .map(|(pkg, _deps)| pkg)
                .collect(),
            deps: resolve
                .deps(id)
                .filter(dep_filter)
                .filter_map(|(pkg, _deps)| {
                    packages
                        .get(&pkg)
                        .and_then(|pkg| pkg.targets().iter().find(|t| t.is_lib()))
                        .and_then(|lib_target| resolve.extern_crate_name(id, pkg, lib_target).ok())
                        .map(|name| Dep { name, pkg })
                })
                .collect(),
            features: resolve.features_sorted(id),
        }
    }))
}
