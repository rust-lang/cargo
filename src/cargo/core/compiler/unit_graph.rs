//! Serialization of [`UnitGraph`] for unstable option [`--unit-graph`].
//!
//! [`--unit-graph`]: https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#unit-graph

use crate::core::compiler::Unit;
use crate::core::compiler::{CompileKind, CompileMode};
use crate::core::profiles::{Profile, UnitFor};
use crate::core::{PackageId, Target};
use crate::util::interning::InternedString;
use crate::util::CargoResult;
use crate::Config;
use std::collections::HashMap;
use std::io::Write;

/// The dependency graph of Units.
pub type UnitGraph = HashMap<Unit, Vec<UnitDep>>;

/// A unit dependency.
#[derive(Debug, Clone, Hash, Eq, PartialEq, PartialOrd, Ord)]
pub struct UnitDep {
    /// The dependency unit.
    pub unit: Unit,
    /// The purpose of this dependency (a dependency for a test, or a build
    /// script, etc.). Do not use this after the unit graph has been built.
    pub unit_for: UnitFor,
    /// The name the parent uses to refer to this dependency.
    pub extern_crate_name: InternedString,
    /// If `Some`, the name of the dependency if renamed in toml.
    /// It's particularly interesting to artifact dependencies which rely on it
    /// for naming their environment variables. Note that the `extern_crate_name`
    /// cannot be used for this as it also may be the build target itself,
    /// which isn't always the renamed dependency name.
    pub dep_name: Option<InternedString>,
    /// Whether or not this is a public dependency.
    pub public: bool,
    /// If `true`, the dependency should not be added to Rust's prelude.
    pub noprelude: bool,
}

const VERSION: u32 = 1;

#[derive(serde::Serialize)]
struct SerializedUnitGraph<'a> {
    version: u32,
    units: Vec<SerializedUnit<'a>>,
    roots: Vec<usize>,
}

#[derive(serde::Serialize)]
struct SerializedUnit<'a> {
    pkg_id: PackageId,
    target: &'a Target,
    profile: &'a Profile,
    platform: CompileKind,
    mode: CompileMode,
    features: &'a Vec<InternedString>,
    #[serde(skip_serializing_if = "std::ops::Not::not")] // hide for unstable build-std
    is_std: bool,
    dependencies: Vec<SerializedUnitDep>,
}

#[derive(serde::Serialize)]
struct SerializedUnitDep {
    index: usize,
    extern_crate_name: InternedString,
    // This is only set on nightly since it is unstable.
    #[serde(skip_serializing_if = "Option::is_none")]
    public: Option<bool>,
    // This is only set on nightly since it is unstable.
    #[serde(skip_serializing_if = "Option::is_none")]
    noprelude: Option<bool>,
    // Intentionally not including `unit_for` because it is a low-level
    // internal detail that is mostly used for building the graph.
}

/// Outputs a JSON serialization of [`UnitGraph`] for given `root_units`
/// to the standard output.
pub fn emit_serialized_unit_graph(
    root_units: &[Unit],
    unit_graph: &UnitGraph,
    config: &Config,
) -> CargoResult<()> {
    let mut units: Vec<(&Unit, &Vec<UnitDep>)> = unit_graph.iter().collect();
    units.sort_unstable();
    // Create a map for quick lookup for dependencies.
    let indices: HashMap<&Unit, usize> = units
        .iter()
        .enumerate()
        .map(|(i, val)| (val.0, i))
        .collect();
    let roots = root_units.iter().map(|root| indices[root]).collect();
    let ser_units = units
        .iter()
        .map(|(unit, unit_deps)| {
            let dependencies = unit_deps
                .iter()
                .map(|unit_dep| {
                    // https://github.com/rust-lang/rust/issues/64260 when stabilized.
                    let (public, noprelude) = if config.nightly_features_allowed {
                        (Some(unit_dep.public), Some(unit_dep.noprelude))
                    } else {
                        (None, None)
                    };
                    SerializedUnitDep {
                        index: indices[&unit_dep.unit],
                        extern_crate_name: unit_dep.extern_crate_name,
                        public,
                        noprelude,
                    }
                })
                .collect();
            SerializedUnit {
                pkg_id: unit.pkg.package_id(),
                target: &unit.target,
                profile: &unit.profile,
                platform: unit.kind,
                mode: unit.mode,
                features: &unit.features,
                is_std: unit.is_std,
                dependencies,
            }
        })
        .collect();
    let s = SerializedUnitGraph {
        version: VERSION,
        units: ser_units,
        roots,
    };

    let stdout = std::io::stdout();
    let mut lock = stdout.lock();
    serde_json::to_writer(&mut lock, &s)?;
    drop(writeln!(lock));
    Ok(())
}
