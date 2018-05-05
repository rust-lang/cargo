use core::resolver::Resolve;
use core::{Package, PackageId, PackageSet, Workspace};
use core::dependency;
use ops::{self, Packages};
use util::CargoResult;

const VERSION: u32 = 1;

pub struct OutputMetadataOptions {
    pub features: Vec<String>,
    pub no_default_features: bool,
    pub all_features: bool,
    pub no_deps: bool,
    pub version: u32,
}

/// Loads the manifest, resolves the dependencies of the project to the concrete
/// used versions - considering overrides - and writes all dependencies in a JSON
/// format to stdout.
pub fn output_metadata(ws: &Workspace, opt: &OutputMetadataOptions) -> CargoResult<ExportInfo> {
    if opt.version != VERSION {
        bail!(
            "metadata version {} not supported, only {} is currently supported",
            opt.version,
            VERSION
        );
    }
    if opt.no_deps {
        metadata_no_deps(ws, opt)
    } else {
        metadata_full(ws, opt)
    }
}

fn metadata_no_deps(ws: &Workspace, _opt: &OutputMetadataOptions) -> CargoResult<ExportInfo> {
    Ok(ExportInfo {
        packages: ws.members().cloned().collect(),
        workspace_members: ws.members().map(|pkg| pkg.package_id().clone()).collect(),
        resolve: None,
        target_directory: ws.target_dir().display().to_string(),
        version: VERSION,
        workspace_root: ws.root().display().to_string(),
    })
}

fn metadata_full(ws: &Workspace, opt: &OutputMetadataOptions) -> CargoResult<ExportInfo> {
    let specs = Packages::All.into_package_id_specs(ws)?;
    let deps = ops::resolve_ws_precisely(
        ws,
        None,
        &opt.features,
        opt.all_features,
        opt.no_default_features,
        &specs,
    )?;
    let (packages, resolve) = deps;

    let resolve = MetadataResolve::new(
        &packages,
        &resolve,
        ws.current_opt().map(|pkg| pkg.package_id().clone()),
    );
    let packages = packages
        .package_ids()
        .map(|i| packages.get(i).map(|p| p.clone()))
        .collect::<CargoResult<Vec<_>>>()?;
    Ok(ExportInfo {
        packages,
        workspace_members: ws.members().map(|pkg| pkg.package_id().clone()).collect(),
        resolve: Some(resolve),
        target_directory: ws.target_dir().display().to_string(),
        version: VERSION,
        workspace_root: ws.root().display().to_string(),
    })
}

#[derive(Serialize)]
pub struct ExportInfo {
    /// All packages for this project, with dependencies.
    packages: Vec<Package>,
    /// Packages which are direct members of the current project.
    workspace_members: Vec<PackageId>,
    /// A graph of the dependencies between packages.
    resolve: Option<MetadataResolve>,
    /// The directory where intermediate build artifacts will be stored.
    target_directory: String,
    /// Version of this JSON format
    version: u32,
    /// Path to the directory with the project.
    workspace_root: String,
}

// The serialization format is different from lockfile, because
// here we use different format for `PackageId`s, and give more
// information about dependencies.
#[derive(Serialize)]
struct MetadataResolve {
    /// Dependencies for each package from `ExportInfo::package`.
    nodes: Vec<Node>,
    /// Deprecated, use `ExportInfo::workspace_members`.
    root: Option<PackageId>,
}

/// Describes dependencies of a single package.
#[derive(Serialize)]
struct Node {
    /// The id of the package.
    id: PackageId,
    /// Deprecated, use `deps` field.
    dependencies: Vec<PackageId>,
    /// Dependencies of this package.
    deps: Vec<Dependency>,
    /// Features, enabled for this package.
    features: Vec<String>,
}

/// Describes a single dependency.
#[derive(Serialize)]
struct Dependency {
    /// The id of the dependency.
    id: PackageId,
    /// The name used for `extern crate` declaration of this dependency.
    name: String,
    /// Is this normal, dev or build dependency
    kind: dependency::Kind,
}

impl MetadataResolve {
    pub fn new(
        packages: &PackageSet,
        resolve: &Resolve,
        root: Option<PackageId>,
    ) -> MetadataResolve {
        let nodes = resolve
            .iter()
            .map(|pkg| {
                Node {
                    id: pkg.clone(),
                    dependencies: resolve.deps(pkg).map(|(dep, _)| dep.clone()).collect(),
                    deps: resolve
                        .deps(pkg)
                        .flat_map(|(id, deps)| {
                            let dep_name = packages.get(id).unwrap()
                                .lib_target().unwrap()
                                .crate_name();
                            deps.iter().map(|dep| {
                                Dependency {
                                    id: id.clone(),
                                    name: dep.rename().unwrap_or(&dep_name)
                                        .to_owned(),
                                    kind: dep.kind(),
                                }
                            }).collect::<Vec<_>>().into_iter()
                        })
                        .collect(),
                    features: resolve
                        .features_sorted(pkg)
                        .into_iter()
                        .map(|s| s.to_string())
                        .collect(),
                }
            })
            .collect();
        MetadataResolve { nodes, root }
    }
}
