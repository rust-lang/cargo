//! cargo-sbom precursor files for external tools to create SBOM files from.
//! See [`build_sbom_graph`] for more.

use cargo_util::collections::{HashMap, HashSet};
use std::collections::{BTreeMap, BTreeSet};

use cargo_util_schemas::sbom::{
    Sbom, SbomCrate, SbomDependency, SbomDependencyType, SbomFormatVersion, SbomIndex, SbomRustc,
    SbomTargetKind,
};
use itertools::Itertools;

use crate::CargoResult;
use crate::util::Rustc;
use crate::workspace::TargetKind;

use super::{BuildRunner, CompileMode, Unit};

impl From<TargetKind> for SbomTargetKind {
    fn from(t: TargetKind) -> Self {
        use TargetKind::*;
        match t {
            Lib(c) => SbomTargetKind::Lib(c),
            Bin => SbomTargetKind::Bin,
            Test => SbomTargetKind::Test,
            Bench => SbomTargetKind::Bench,
            ExampleBin | ExampleLib(_) => SbomTargetKind::Example,
            CustomBuild => SbomTargetKind::CustomBuild,
        }
    }
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

/// Build an [`Sbom`] for the given [`Unit`].
#[tracing::instrument(skip_all)]
pub fn build_sbom(build_runner: &BuildRunner<'_, '_>, root: &Unit) -> CargoResult<Sbom> {
    let bcx = build_runner.bcx;
    let rustc: SbomRustc = bcx.rustc().into();

    let mut crates = Vec::new();
    let sbom_graph = build_sbom_graph(build_runner, root);

    // Build set of indices for each node in the graph for fast lookup.
    let indices: HashMap<&Unit, SbomIndex> = sbom_graph
        .keys()
        .enumerate()
        .map(|(i, dep)| (*dep, SbomIndex(i)))
        .collect();

    // Add a item to the crates list for each node in the graph.
    for (unit, edges) in sbom_graph {
        let package_id = unit.pkg.package_id().to_spec();
        let features = unit.features.iter().map(|f| f.to_string()).collect_vec();
        let target_kind = unit.target.kind().clone();

        let mut krate = SbomCrate::new(package_id, features, SbomTargetKind::from(target_kind));
        for (dep, kind) in edges {
            krate.dependencies.push(SbomDependency {
                index: indices[dep],
                kind: kind,
            });
        }
        crates.push(krate);
    }
    let target = match root.kind {
        super::CompileKind::Host => build_runner.bcx.host_triple(),
        super::CompileKind::Target(target) => target.rustc_target(),
    };
    Ok(Sbom {
        version: SbomFormatVersion(1),
        crates,
        root: indices[root],
        rustc,
        target: target.to_string(),
    })
}

/// List all dependencies, including transitive ones. A dependency can also appear multiple times
/// if it's using different settings, e.g. profile, features or crate versions.
///
/// Returns a graph of dependencies.
#[tracing::instrument(skip_all)]
fn build_sbom_graph<'a>(
    build_runner: &'a BuildRunner<'_, '_>,
    root: &'a Unit,
) -> BTreeMap<&'a Unit, BTreeSet<(&'a Unit, SbomDependencyType)>> {
    tracing::trace!("building sbom graph for {}", root.pkg.package_id());

    let mut queue = Vec::new();
    let mut sbom_graph: BTreeMap<&Unit, BTreeSet<(&Unit, SbomDependencyType)>> = BTreeMap::new();
    let mut visited = HashSet::default();

    // Search to collect all dependencies of the root unit.
    queue.push((root, root, false));
    while let Some((node, parent, is_build_dep)) = queue.pop() {
        let dependencies = sbom_graph.entry(parent).or_default();
        for dep in build_runner.unit_deps(node) {
            let dep = &dep.unit;
            let (next_parent, next_is_build_dep) = if dep.mode == CompileMode::RunCustomBuild {
                // Nodes in the SBOM graph for building/running build scripts are moved on to their parent as build dependencies.
                (parent, true)
            } else {
                // Proc-macros and build scripts are marked as build dependencies.
                let dep_type = match is_build_dep || dep.target.proc_macro() {
                    false => SbomDependencyType::Normal,
                    true => SbomDependencyType::Build,
                };
                dependencies.insert((dep, dep_type));
                (dep, false)
            };
            if visited.insert(dep) {
                queue.push((dep, next_parent, next_is_build_dep));
            }
        }
    }
    sbom_graph
}
