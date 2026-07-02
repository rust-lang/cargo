use std::collections::BTreeSet;

use crate::util::data_structures::IndexMap;
use crate::util::data_structures::IndexSet;
use tracing::{instrument, trace};

use super::BuildContext;
use super::unit::Unit;
use crate::core::Dependency;
use crate::core::PackageId;
use crate::core::compiler::build_config::CompileMode;
use crate::core::dependency::DepKind;
use crate::core::manifest::TargetKind;
use crate::util::interning::InternedString;

/// Track and translate `unused_externs` to `unused_dependencies`
pub struct UnusedDepState {
    pub states: IndexMap<PackageId, IndexMap<DepKind, DependenciesState>>,
}

impl UnusedDepState {
    #[instrument(name = "UnusedDepState::new", skip_all)]
    pub fn new(bcx: &BuildContext<'_, '_>) -> Self {
        // Find all units for a package that can report unused externs
        let mut root_build_script_builds = IndexSet::default();
        let roots = &bcx.roots;
        for root in roots.iter() {
            for build_script_run in bcx.unit_graph[root].iter() {
                if !build_script_run.unit.target.is_custom_build()
                    && build_script_run.unit.pkg.package_id() != root.pkg.package_id()
                {
                    continue;
                }
                for build_script_build in bcx.unit_graph[&build_script_run.unit].iter() {
                    if !build_script_build.unit.target.is_custom_build()
                        && build_script_build.unit.pkg.package_id() != root.pkg.package_id()
                    {
                        continue;
                    }
                    if build_script_build.unit.mode != CompileMode::Build {
                        continue;
                    }
                    root_build_script_builds.insert(build_script_build.unit.clone());
                }
            }
        }

        trace!("selected dep kinds: {:?}", bcx.selected_dep_kinds);
        let mut states = IndexMap::<_, IndexMap<_, DependenciesState>>::default();
        for root in roots.iter().chain(root_build_script_builds.iter()) {
            let pkg_id = root.pkg.package_id();
            let dep_kind = dep_kind_of(root);
            if !bcx.selected_dep_kinds.contains(dep_kind) {
                trace!(
                    "pkg {} v{} ({dep_kind:?}): ignoring unused deps due to non-exhaustive units",
                    pkg_id.name(),
                    pkg_id.version(),
                );
                continue;
            }
            trace!(
                "tracking root {} {} ({:?})",
                root.pkg.name(),
                unit_desc(root),
                dep_kind
            );

            let state = states
                .entry(pkg_id)
                .or_default()
                .entry(dep_kind)
                .or_default();
            state.needed_units += 1;
            for dep in bcx.unit_graph[root].iter() {
                trace!(
                    "    => {} (deps={})",
                    dep.unit.pkg.name(),
                    dep.manifest_deps.0.is_some()
                );
                let manifest_deps = if let Some(manifest_deps) = &dep.manifest_deps.0 {
                    Some(manifest_deps.clone())
                } else if dep.unit.pkg.package_id() == root.pkg.package_id() {
                    None
                } else {
                    continue;
                };
                state.externs.insert(
                    dep.extern_crate_name,
                    ExternState {
                        unit: dep.unit.clone(),
                        manifest_deps,
                    },
                );
            }
        }

        Self { states }
    }

    pub fn record_unused_externs_for_unit(
        &mut self,
        unit: &Unit,
        unused_externs: BTreeSet<InternedString>,
    ) {
        let pkg_id = unit.pkg.package_id();
        let dep_kind = dep_kind_of(unit);
        trace!(
            "pkg {} v{} ({dep_kind:?}): unused externs {unused_externs:?}",
            pkg_id.name(),
            pkg_id.version(),
        );
        let state = self
            .states
            .entry(pkg_id)
            .or_default()
            .entry(dep_kind)
            .or_default();
        state.seen_units.push(unit.clone());
        if let Some(existing) = state.unused_externs.as_mut() {
            existing.retain(|ext| unused_externs.contains(ext));
        } else {
            state.unused_externs = Some(unused_externs);
        }
    }
}

/// Track a package's [`DepKind`]
#[derive(Default)]
pub struct DependenciesState {
    /// All declared dependencies
    pub externs: IndexMap<InternedString, ExternState>,
    /// Expected [`Self::seen_units`] entries to know we've received them all
    ///
    /// To avoid warning in cases where we didn't,
    /// e.g. if a [`Unit`] errored and didn't report unused externs.
    pub needed_units: usize,
    /// Units that have reported their unused externs
    pub seen_units: Vec<Unit>,
    /// Intersection of unused externs across all [`Self::seen_units`]
    pub unused_externs: Option<BTreeSet<InternedString>>,
}

#[derive(Clone)]
pub struct ExternState {
    pub unit: Unit,
    pub manifest_deps: Option<Vec<Dependency>>,
}

fn dep_kind_of(unit: &Unit) -> DepKind {
    match unit.target.kind() {
        TargetKind::Lib(_) => match unit.mode {
            // To support lib.rs with #[cfg(test)] use foo_crate as _;
            CompileMode::Test => DepKind::Development,
            _ => DepKind::Normal,
        },
        TargetKind::Bin => DepKind::Normal,
        TargetKind::Test => DepKind::Development,
        TargetKind::Bench => DepKind::Development,
        TargetKind::ExampleLib(_) => DepKind::Development,
        TargetKind::ExampleBin => DepKind::Development,
        TargetKind::CustomBuild => DepKind::Build,
    }
}

fn unit_desc(unit: &Unit) -> String {
    format!(
        "{}/{}+{:?}",
        unit.target.name(),
        unit.target.kind().description(),
        unit.mode,
    )
}
