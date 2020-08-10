use super::unit::Unit;
use super::{Context, UnitDep};
use crate::core::compiler::build_config::CompileMode;
use crate::core::dependency::DepKind;
use crate::core::manifest::TargetKind;
use crate::core::Dependency;
use crate::core::PackageId;
use crate::util::errors::CargoResult;
use crate::util::interning::InternedString;
use crate::Config;
use log::trace;

use std::collections::{HashMap, HashSet};

pub type AllowedKinds = HashSet<DepKind>;

#[derive(serde::Deserialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
/// Lint levels
///
/// Note that order is important here
pub enum LintLevel {
    // Allow isn't mentioned as the unused dependencies message
    // isn't emitted if the lint is set to allow.
    Warn,
    Deny,
    Forbid,
}

#[derive(serde::Deserialize, Debug)]
pub struct UnusedExterns {
    lint_level: LintLevel,
    unused_extern_names: Vec<String>,
}

#[derive(Default, Clone)]
struct State {
    /// All externs passed to units
    externs: HashMap<InternedString, Option<Dependency>>,
    /// The used externs so far.
    /// The DepKind is included so that we can tell when
    /// a proper dependency should actually be a dev-dependency
    used_externs: HashSet<(InternedString, DepKind)>,
    reports_needed_by: HashSet<Unit>,
}

#[derive(Clone)]
pub struct UnusedDepState {
    states: HashMap<(PackageId, Option<DepKind>), State>,
    /// The worst encountered lint level so far
    worst_lint_level: LintLevel,
    /// Tracking for which units we have received reports from.
    ///
    /// When we didn't receive reports, e.g. because of an error,
    /// or because the compiler segfaulted, etc., we don't emit
    /// any warnings for missing dependencies for the specific
    /// class.
    reports_obtained: HashSet<Unit>,
}

fn dep_kind_desc(kind: Option<DepKind>) -> &'static str {
    match kind {
        Some(kind) => match kind {
            DepKind::Normal => "",
            DepKind::Development => "dev-",
            DepKind::Build => "build-",
        },
        None => "internal-",
    }
}

fn dep_kind_of(unit: &Unit) -> DepKind {
    match unit.target.kind() {
        TargetKind::Lib(_) => match unit.mode {
            // To support lib.rs with #[cfg(test)] use foo_crate as _;
            CompileMode::Test => DepKind::Development,
            // To correctly register dev-dependencies
            CompileMode::Doctest => DepKind::Development,
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

impl UnusedDepState {
    pub fn new_with_graph(cx: &mut Context<'_, '_>) -> Self {
        let mut states = HashMap::<_, State>::new();

        let roots_without_build = &cx.bcx.roots;

        // Compute the build scripts of the roots so that we can
        // lint for unused [build-dependencies].
        // First iterate on the root's dependencies,
        // searching for the build-script-run units.
        // Obtain the build-script-build units from those by
        // another iteration, as only they depend on the
        // [build-dependencies] of a package.
        let mut build_root_runs = HashSet::new();
        for root in roots_without_build.iter() {
            for dep in cx.unit_deps(root).iter() {
                if dep.unit.pkg.package_id() != root.pkg.package_id() {
                    continue;
                }
                if !dep.unit.target.is_custom_build() {
                    continue;
                }
                build_root_runs.insert(dep.unit.clone());
            }
        }
        let mut build_roots = HashSet::new();
        for root in build_root_runs.iter() {
            for dep in cx.unit_deps(root).iter() {
                if dep.unit.pkg.package_id() != root.pkg.package_id() {
                    continue;
                }
                if !dep.unit.target.is_custom_build() {
                    continue;
                }
                if dep.unit.mode != CompileMode::Build {
                    continue;
                }
                build_roots.insert(dep.unit.clone());
            }
        }

        // Now build the datastructures
        for root in roots_without_build.iter().chain(build_roots.iter()) {
            let pkg_id = root.pkg.package_id();
            trace!(
                "Udeps root package {} tgt {}",
                root.pkg.name(),
                unit_desc(root),
            );
            if root.mode == CompileMode::Doctest {
                //trace!("    -> skipping doctest");
                //continue;
            }
            for dep in cx.unit_deps(root).iter() {
                trace!(
                    "    => {} {}",
                    dep.unit.pkg.name(),
                    dep.dependency.0.is_some()
                );
                let dependency = if let Some(dependency) = &dep.dependency.0 {
                    Some(dependency.clone())
                } else if dep.unit.pkg.package_id() == root.pkg.package_id() {
                    None
                } else {
                    continue;
                };
                let kind = dependency.as_ref().map(|dependency| dependency.kind());
                let state = states
                    .entry((pkg_id, kind))
                    .or_insert_with(Default::default);
                state.externs.insert(dep.extern_crate_name, dependency);
                state.reports_needed_by.insert(root.clone());
            }
        }

        Self {
            states,
            worst_lint_level: LintLevel::Warn,
            reports_obtained: HashSet::new(),
        }
    }
    /// Records the unused externs coming from the compiler by first inverting them to the used externs
    /// and then updating the global list of used externs
    pub fn record_unused_externs_for_unit(
        &mut self,
        unit_deps: &[UnitDep],
        unit: &Unit,
        unused_externs: UnusedExterns,
    ) {
        self.reports_obtained.insert(unit.clone());
        self.worst_lint_level = self.worst_lint_level.max(unused_externs.lint_level);

        let usable_deps_iter = unit_deps
            .iter()
            // compare with similar check in extern_args
            .filter(|dep| dep.unit.target.is_linkable() && !dep.unit.mode.is_doc());

        let unused_externs_set = unused_externs
            .unused_extern_names
            .iter()
            .map(|ex| InternedString::new(ex))
            .collect::<HashSet<InternedString>>();
        let used_deps_iter =
            usable_deps_iter.filter(|dep| !unused_externs_set.contains(&dep.extern_crate_name));
        let pkg_id = unit.pkg.package_id();
        for used_dep in used_deps_iter {
            trace!(
                "Used extern {} for pkg {} v{} tgt {}",
                used_dep.extern_crate_name,
                pkg_id.name(),
                pkg_id.version(),
                unit_desc(unit),
            );
            let kind = if let Some(dependency) = &used_dep.dependency.0 {
                Some(dependency.kind())
            } else if used_dep.unit.pkg.package_id() == unit.pkg.package_id() {
                // Deps within the same crate have no dependency entry
                None
            } else {
                continue;
            };
            if let Some(state) = self.states.get_mut(&(pkg_id, kind)) {
                let record_kind = dep_kind_of(unit);
                trace!(
                    "   => updating state of {}dep",
                    dep_kind_desc(Some(record_kind)),
                );
                state
                    .used_externs
                    .insert((used_dep.extern_crate_name, record_kind));
            }
        }
    }
    pub fn emit_unused_early_warnings(&self, cx: &mut Context<'_, '_>) -> CargoResult<()> {
        self.emit_unused_warnings_inner(cx.bcx.config, Some(&cx.bcx.allowed_kinds))
    }
    pub fn emit_unused_late_warnings(&self, config: &Config) -> CargoResult<()> {
        self.emit_unused_warnings_inner(config, None)
    }
    fn emit_unused_warnings_inner(
        &self,
        config: &Config,
        allowed_kinds_or_late: Option<&AllowedKinds>,
    ) -> CargoResult<()> {
        trace!(
            "Allowed dependency kinds for the unused deps check: {:?}",
            allowed_kinds_or_late
        );

        let mut error_count = 0;
        {
            let mut emit_lint: Box<dyn FnMut(String) -> CargoResult<()>> =
                if self.worst_lint_level == LintLevel::Warn {
                    Box::new(|msg| config.shell().warn(msg))
                } else {
                    Box::new(|msg| {
                        error_count += 1;
                        config.shell().error(msg)
                    })
                };

            // Sort the states to have a consistent output
            let mut states_sorted = self.states.iter().collect::<Vec<_>>();
            states_sorted.sort_by_key(|(k, _v)| k.clone());
            for ((pkg_id, dep_kind), state) in states_sorted.iter() {
                let outstanding_reports = state
                    .reports_needed_by
                    .iter()
                    .filter(|report| !self.reports_obtained.contains(report))
                    .collect::<Vec<_>>();
                if !outstanding_reports.is_empty() {
                    trace!("Supressing unused deps warning of pkg {} v{} mode '{}dep' due to outstanding reports {:?}", pkg_id.name(), pkg_id.version(), dep_kind_desc(*dep_kind),
                outstanding_reports.iter().map(|unit|
                unit_desc(unit)).collect::<Vec<_>>());

                    // Some compilations errored without printing the unused externs.
                    // Don't print the warning in order to reduce false positive
                    // spam during errors.
                    continue;
                }
                // Sort the externs to have a consistent output
                let mut externs_sorted = state.externs.iter().collect::<Vec<_>>();
                externs_sorted.sort_by_key(|(k, _v)| k.clone());
                for (ext, dependency) in externs_sorted.iter() {
                    let dep_kind = if let Some(dep_kind) = dep_kind {
                        dep_kind
                    } else {
                        // Internal dep_kind isn't interesting to us
                        continue;
                    };
                    if state.used_externs.contains(&(**ext, *dep_kind)) {
                        // The dependency is used
                        continue;
                    }
                    // Implicitly added dependencies (in the same crate) aren't interesting
                    let dependency = if let Some(dependency) = dependency {
                        dependency
                    } else {
                        continue;
                    };
                    if let Some(allowed_kinds) = allowed_kinds_or_late {
                        if !allowed_kinds.contains(dep_kind) {
                            // We can't warn for dependencies of this target kind
                            // as we aren't compiling all the units
                            // that use the dependency kind
                            trace!("Supressing unused deps warning of {} in pkg {} v{} as mode '{}dep' not allowed", dependency.name_in_toml(), pkg_id.name(), pkg_id.version(), dep_kind_desc(Some(*dep_kind)));
                            continue;
                        }
                    } else {
                    }
                    if dependency.name_in_toml().starts_with("_") {
                        // Dependencies starting with an underscore
                        // are marked as ignored
                        trace!(
                            "Supressing unused deps warning of {} in pkg {} v{} due to name",
                            dependency.name_in_toml(),
                            pkg_id.name(),
                            pkg_id.version()
                        );
                        continue;
                    }
                    if dep_kind == &DepKind::Normal
                        && state.used_externs.contains(&(**ext, DepKind::Development))
                    {
                        // The dependency is used but only by dev targets,
                        // which means it should be a dev-dependency instead
                        emit_lint(format!(
                            "dependency {} in package {} v{} is only used by dev targets",
                            dependency.name_in_toml(),
                            pkg_id.name(),
                            pkg_id.version()
                        ))?;
                        continue;
                    }

                    emit_lint(format!(
                        "unused {}dependency {} in package {} v{}",
                        dep_kind_desc(Some(*dep_kind)),
                        dependency.name_in_toml(),
                        pkg_id.name(),
                        pkg_id.version()
                    ))?;
                }
            }
        }
        if error_count > 0 {
            anyhow::bail!(
                "exiting because of {} unused dependencies error(s)",
                error_count
            );
        }
        Ok(())
    }
}
