//! cargo-sbom precursor files for external tools to create SBOM files from.
//! See [`output_sbom`] for more.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::PathBuf;

use cargo_util_schemas::core::PackageIdSpec;
use itertools::Itertools;
use serde::Serialize;

use crate::core::profiles::{DebugInfo, Lto, PanicStrategy, Profile};
use crate::core::TargetKind;
use crate::util::interning::InternedString;
use crate::util::Rustc;
use crate::CargoResult;

use super::{BuildOutput, CompileMode};
use super::{BuildRunner, Unit};

/// Typed version of a SBOM format version number.
#[derive(Serialize, Copy, Clone, Debug, Ord, PartialOrd, Eq, PartialEq)]
pub struct SbomFormatVersion(u32);

/// A profile can be overriden for individual packages.
///
/// This wraps a [`Profile`] object.
/// See <https://doc.rust-lang.org/nightly/cargo/reference/profiles.html#overrides>
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
struct SbomProfile {
    name: InternedString,
    opt_level: InternedString,
    lto: Lto,
    #[serde(skip_serializing_if = "Option::is_none")]
    codegen_backend: Option<InternedString>,
    debuginfo: DebugInfo,
    #[serde(skip_serializing_if = "Option::is_none")]
    split_debuginfo: Option<InternedString>,
    debug_assertions: bool,
    overflow_checks: bool,
    rpath: bool,
    panic: PanicStrategy,
}

impl From<&Profile> for SbomProfile {
    fn from(profile: &Profile) -> Self {
        let Profile {
            name,
            opt_level,
            root: _,
            lto,
            codegen_backend,
            codegen_units: _,
            debuginfo,
            split_debuginfo,
            debug_assertions,
            overflow_checks,
            rpath,
            incremental: _,
            panic,
            strip: _,
            rustflags: _,
            trim_paths: _,
        } = profile.clone();

        Self {
            name,
            opt_level,
            lto,
            codegen_backend,
            debuginfo,
            split_debuginfo,
            debug_assertions,
            overflow_checks,
            rpath,
            panic,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, PartialOrd, Eq, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
enum SbomDependencyType {
    /// A dependency linked to the artifact produced by this unit.
    Normal,
    /// A dependency needed to run the build for this unit (e.g. a build script or proc-macro).
    /// The dependency is not linked to the artifact produced by this unit.
    Build,
}

#[derive(Serialize, Copy, Clone, Debug, Ord, PartialOrd, Eq, PartialEq)]
struct SbomIndex(usize);

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
struct SbomDependency {
    index: SbomIndex,
    kind: SbomDependencyType,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
struct SbomPackage {
    id: PackageIdSpec,
    profile: SbomProfile,
    features: Vec<String>,
    cfgs: Vec<String>,
    dependencies: Vec<SbomDependency>,
}

impl SbomPackage {
    pub fn new(unit: &Unit, build_script_output: Option<&BuildOutput>) -> Self {
        let package_id = unit.pkg.package_id().to_spec();
        let features = unit.features.iter().map(|f| f.to_string()).collect_vec();
        let cfgs = build_script_output
            .map(|b| b.cfgs.clone())
            .unwrap_or_default();

        Self {
            id: package_id,
            profile: (&unit.profile).into(),
            features,
            cfgs,
            dependencies: Vec::new(),
        }
    }
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "snake_case")]
struct SbomRustc {
    version: String,
    wrapper: Option<PathBuf>,
    workspace_wrapper: Option<PathBuf>,
    commit_hash: Option<String>,
    host: String,
    verbose_version: String,
}

impl From<&Rustc> for SbomRustc {
    fn from(rustc: &Rustc) -> Self {
        Self {
            version: rustc.version.to_string(),
            wrapper: rustc.wrapper.clone(),
            workspace_wrapper: rustc.workspace_wrapper.clone(),
            commit_hash: rustc.commit_hash.clone(),
            host: rustc.host.to_string(),
            verbose_version: rustc.verbose_version.clone(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub struct Sbom {
    version: SbomFormatVersion,
    root: SbomIndex,
    packages: Vec<SbomPackage>,
    rustc: SbomRustc,
}

/// Build an [`Sbom`] for the given [`Unit`].
pub fn build_sbom(build_runner: &mut BuildRunner<'_, '_>, root: &Unit) -> CargoResult<Sbom> {
    let bcx = build_runner.bcx;
    let rustc: SbomRustc = bcx.rustc().into();

    let mut packages = Vec::new();

    let build_script_outputs = build_runner.build_script_outputs.lock().unwrap();
    let sbom_graph = build_sbom_graph(build_runner, root);

    // Build set of indicies for each node in the graph for fast lookup.
    let indicies: HashMap<&Unit, SbomIndex> = sbom_graph
        .keys()
        .enumerate()
        .map(|(i, dep)| (*dep, SbomIndex(i)))
        .collect();

    // Add a item to the packages list for each node in the graph.
    for (unit, edges) in sbom_graph {
        let build_script_output = build_runner
            .find_build_script_metadata(unit)
            .and_then(|meta| build_script_outputs.get(meta));
        let mut krate = SbomPackage::new(unit, build_script_output);
        for (dep, kind) in &edges {
            krate.dependencies.push(SbomDependency {
                index: indicies[dep],
                kind: *kind,
            });
        }
        packages.push(krate);
    }

    Ok(Sbom {
        version: SbomFormatVersion(1),
        packages,
        root: indicies[root],
        rustc,
    })
}

/// List all dependencies, including transitive ones. A dependency can also appear multiple times
/// if it's using different settings, e.g. profile, features or crate versions.
///
/// Returns a graph of dependencies.
fn build_sbom_graph<'a>(
    build_runner: &'a BuildRunner<'_, '_>,
    root: &'a Unit,
) -> BTreeMap<&'a Unit, BTreeSet<(&'a Unit, SbomDependencyType)>> {
    let mut queue = Vec::new();
    let mut sbom_graph: BTreeMap<&Unit, BTreeSet<(&Unit, SbomDependencyType)>> = BTreeMap::new();

    // Depth-first search to collect all dependencies of the root unit.
    queue.push((root, root, false));
    while let Some((node, parent, is_build_dep)) = queue.pop() {
        let dependencies = sbom_graph.entry(parent).or_default();
        for child in build_runner.unit_deps(node) {
            let child = &child.unit;
            if child.mode == CompileMode::RunCustomBuild
                || *child.target.kind() == TargetKind::CustomBuild
            {
                // Nodes in the SBOM graph for building/running build scripts are moved on to their parent as build dependencies.
                queue.push((child, parent, true));
            } else if child.pkg == parent.pkg {
                // Nodes in the SBOM graph within the same package are moved to the parents.
                queue.push((child, parent, false));
            } else {
                queue.push((child, child, false));
                // Proc-macros and build scripts are marked as build dependencies.
                let dep_type = match is_build_dep || child.target.proc_macro() {
                    false => SbomDependencyType::Normal,
                    true => SbomDependencyType::Build,
                };
                dependencies.insert((child, dep_type));
                tracing::trace!(
                    "adding sbom build edge {} -> {} ({:?})",
                    parent.pkg.package_id(),
                    child.pkg.package_id(),
                    dep_type
                );
            }
        }
    }
    sbom_graph
}
