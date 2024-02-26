//! cargo-sbom precursor files for external tools to create SBOM files from.
//! See [`output_sbom`] for more.

use std::{
    collections::BTreeSet,
    io::{BufWriter, Write},
    path::PathBuf,
};

use cargo_util::paths::{self};
use cargo_util_schemas::core::PackageIdSpec;
use itertools::Itertools;
use serde::Serialize;

use crate::{
    core::{compiler::FileFlavor, profiles::Profile, Target, TargetKind},
    util::Rustc,
    CargoResult,
};

use super::{unit_graph::UnitDep, BuildRunner, CrateType, Unit};

/// Typed version of a SBOM format version number.
pub struct SbomFormatVersion<const V: u32>;

impl<const V: u32> Serialize for SbomFormatVersion<V> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_u32(V)
    }
}

#[derive(Serialize, Clone, Debug)]
struct SbomDependency {
    package_id: PackageIdSpec,
    package: String,
    version: String,
    features: Vec<String>,
    extern_crate_name: String,
}

impl From<&UnitDep> for SbomDependency {
    fn from(dep: &UnitDep) -> Self {
        let features = dep
            .unit
            .features
            .iter()
            .map(|dep| dep.to_string())
            .collect_vec();

        Self {
            package_id: dep.unit.pkg.package_id().to_spec(),
            package: dep.unit.pkg.package_id().name().to_string(),
            version: dep.unit.pkg.package_id().version().to_string(),
            features,
            extern_crate_name: dep.extern_crate_name.to_string(),
        }
    }
}

#[derive(Serialize)]
struct SbomTarget {
    kind: TargetKind,
    crate_type: Option<CrateType>,
    name: String,
    edition: String,
}

impl From<&Target> for SbomTarget {
    fn from(target: &Target) -> Self {
        SbomTarget {
            kind: target.kind().clone(),
            crate_type: target.kind().rustc_crate_types().first().cloned(),
            name: target.name().to_string(),
            edition: target.edition().to_string(),
        }
    }
}

#[derive(Serialize)]
struct SbomRustc {
    version: String,
    wrapper: Option<PathBuf>,
    commit_hash: Option<String>,
    host: String,
}

impl From<&Rustc> for SbomRustc {
    fn from(rustc: &Rustc) -> Self {
        Self {
            version: rustc.version.to_string(),
            wrapper: rustc.wrapper.clone(),
            commit_hash: rustc.commit_hash.clone(),
            host: rustc.host.to_string(),
        }
    }
}

#[derive(Serialize)]
struct Sbom {
    format_version: SbomFormatVersion<1>,
    package_id: PackageIdSpec,
    name: String,
    version: String,
    source: String,
    target: SbomTarget,
    profile: Profile,
    dependencies: Vec<SbomDependency>,
    features: Vec<String>,
    rustc: SbomRustc,
}

impl Sbom {
    pub fn new(unit: &Unit, dependencies: Vec<SbomDependency>, rustc: SbomRustc) -> Self {
        let package_id = unit.pkg.summary().package_id().to_spec();
        let name = unit.pkg.name().to_string();
        let version = unit.pkg.version().to_string();
        let source = unit.pkg.package_id().source_id().to_string();
        let target = (&unit.target).into();
        let profile = unit.profile.clone();
        let features = unit.features.iter().map(|f| f.to_string()).collect();

        Self {
            format_version: SbomFormatVersion,
            package_id,
            name,
            version,
            source,
            target,
            profile,
            dependencies,
            features,
            rustc,
        }
    }
}

/// Saves a `<artifact>.cargo-sbom.json` file for the given [`Unit`].
///
pub fn output_sbom(build_runner: &mut BuildRunner<'_, '_>, unit: &Unit) -> CargoResult<()> {
    let bcx = build_runner.bcx;

    let dependencies = fetch_dependencies(build_runner, unit);

    // TODO collect build & unit data, then transform into JSON output
    for output in build_runner
        .outputs(unit)?
        .iter()
        .filter(|o| matches!(o.flavor, FileFlavor::Normal | FileFlavor::Linkable))
    {
        if let Some(ref link_dst) = output.hardlink {
            let output_path = link_dst.with_extension("cargo-sbom.json");

            let rustc = bcx.rustc().into();
            let sbom = Sbom::new(unit, dependencies.clone(), rustc);

            let mut outfile = BufWriter::new(paths::create(output_path.clone())?);
            let output = serde_json::to_string_pretty(&sbom)?;
            write!(outfile, "{}", output)?;
        }
    }

    Ok(())
}

/// Fetch all dependencies, including transitive ones. A dependency can also appear multiple times
/// if it's included with different versions.
fn fetch_dependencies(build_runner: &mut BuildRunner<'_, '_>, unit: &Unit) -> Vec<SbomDependency> {
    let unit_graph = &build_runner.bcx.unit_graph;
    let root_deps = build_runner.unit_deps(unit);

    let mut result = Vec::new();
    let mut queue: BTreeSet<&UnitDep> = root_deps.iter().collect();
    let mut visited: BTreeSet<&UnitDep> = BTreeSet::new();

    while let Some(dependency) = queue.pop_first() {
        // ignore any custom build scripts.
        if dependency.unit.mode.is_run_custom_build() {
            continue;
        }
        if visited.contains(dependency) {
            continue;
        }

        result.push(dependency);
        visited.insert(dependency);

        let mut dependencies: BTreeSet<&UnitDep> = unit_graph[&dependency.unit].iter().collect();
        queue.append(&mut dependencies);
    }

    result.into_iter().map(|d| d.into()).collect_vec()
}
