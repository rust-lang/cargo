//! cargo-sbom precursor files for external tools to create SBOM files from.
//! See [`output_sbom`] for more.

use std::collections::BTreeSet;
use std::io::BufWriter;
use std::path::PathBuf;

use cargo_util::paths::{self};
use cargo_util_schemas::core::PackageIdSpec;
use itertools::Itertools;
use semver::Version;
use serde::Serialize;

use crate::core::profiles::{DebugInfo, Lto, Profile};
use crate::core::{Target, TargetKind};
use crate::util::Rustc;
use crate::CargoResult;

use super::{unit_graph::UnitDep, BuildRunner, CrateType, Unit};

#[derive(Serialize, Clone, Debug, Copy)]
#[serde(rename_all = "kebab-case")]
enum SbomBuildType {
    /// A package dependency
    Normal,
    /// A build script dependency
    Build,
}

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

/// A profile can be overriden for individual packages.
///
/// This wraps a [`Profile`] object.
/// See <https://doc.rust-lang.org/nightly/cargo/reference/profiles.html#overrides>
#[derive(Serialize, Clone, Debug)]
struct SbomProfile {
    name: String,
    opt_level: String,
    lto: Lto,
    codegen_backend: Option<String>,
    codegen_units: Option<u32>,
    debuginfo: DebugInfo,
    split_debuginfo: Option<String>,
    debug_assertions: bool,
    overflow_checks: bool,
    rpath: bool,
    incremental: bool,
    panic: String,
    #[serde(skip_serializing_if = "Vec::is_empty")] // remove when `rustflags` is stablized
    rustflags: Vec<String>,
}

impl From<&Profile> for SbomProfile {
    fn from(profile: &Profile) -> Self {
        let rustflags = profile
            .rustflags
            .iter()
            .map(|x| x.to_string())
            .collect_vec();

        Self {
            name: profile.name.to_string(),
            opt_level: profile.opt_level.to_string(),
            lto: profile.lto,
            codegen_backend: profile.codegen_backend.map(|x| x.to_string()),
            codegen_units: profile.codegen_units.clone(),
            debuginfo: profile.debuginfo.clone(),
            split_debuginfo: profile.split_debuginfo.map(|x| x.to_string()),
            debug_assertions: profile.debug_assertions,
            overflow_checks: profile.overflow_checks,
            rpath: profile.rpath,
            incremental: profile.incremental,
            panic: profile.panic.to_string(),
            rustflags,
        }
    }
}

/// Describes a package dependency
#[derive(Serialize, Clone, Debug)]
struct SbomPackage {
    package_id: PackageIdSpec,
    package: String,
    /// Package profile is given when it differs from the root level [`Profile`].
    #[serde(skip_serializing_if = "Option::is_none")]
    profile: Option<SbomProfile>,
    version: Option<Version>,
    features: Vec<String>,
    build_type: SbomBuildType,
    extern_crate_name: String,
    /// Indices into the `packages` array
    dependencies: Vec<usize>,
}

impl SbomPackage {
    pub fn new(unit_dep: &UnitDep, root_profile: &Profile) -> Self {
        let package_id = unit_dep.unit.pkg.package_id().to_spec();
        let package = package_id.name().to_string();
        let profile = if &unit_dep.unit.profile != root_profile {
            Some((&unit_dep.unit.profile).into())
        } else {
            None
        };
        let build_type = if unit_dep.unit.mode.is_run_custom_build() {
            SbomBuildType::Build
        } else {
            SbomBuildType::Normal
        };

        let version = package_id.version();
        let features = unit_dep
            .unit
            .features
            .iter()
            .map(|f| f.to_string())
            .collect_vec();

        Self {
            package_id,
            package,
            profile,
            version,
            features,
            build_type,
            extern_crate_name: unit_dep.extern_crate_name.to_string(),
            dependencies: Vec::new(),
        }
    }
}

#[derive(Serialize)]
struct SbomTarget {
    kind: TargetKind,
    crate_types: Vec<CrateType>,
    name: String,
    edition: String,
}

impl From<&Target> for SbomTarget {
    fn from(target: &Target) -> Self {
        SbomTarget {
            kind: target.kind().clone(),
            crate_types: target.kind().rustc_crate_types().clone(),
            name: target.name().to_string(),
            edition: target.edition().to_string(),
        }
    }
}

#[derive(Serialize, Clone)]
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
struct Sbom {
    format_version: SbomFormatVersion<1>,
    package_id: PackageIdSpec,
    name: String,
    version: String,
    source: String,
    target: SbomTarget,
    profile: SbomProfile,
    packages: Vec<SbomPackage>,
    features: Vec<String>,
    rustc: SbomRustc,
}

impl Sbom {
    pub fn new(unit: &Unit, packages: Vec<SbomPackage>, rustc: SbomRustc) -> Self {
        let package_id = unit.pkg.summary().package_id().to_spec();
        let name = unit.pkg.name().to_string();
        let version = unit.pkg.version().to_string();
        let source = unit.pkg.package_id().source_id().to_string();
        let target = (&unit.target).into();
        let profile = (&unit.profile).into();
        let features = unit.features.iter().map(|f| f.to_string()).collect();

        Self {
            format_version: SbomFormatVersion,
            package_id,
            name,
            version,
            source,
            target,
            profile,
            packages,
            features,
            rustc,
        }
    }
}

/// Saves a `<artifact>.cargo-sbom.json` file for the given [`Unit`].
///
pub fn output_sbom(build_runner: &mut BuildRunner<'_, '_>, unit: &Unit) -> CargoResult<()> {
    let bcx = build_runner.bcx;
    let rustc: SbomRustc = bcx.rustc().into();

    let packages = collect_packages(build_runner, unit);

    for sbom_output_file in build_runner.sbom_output_files(unit)? {
        let sbom = Sbom::new(unit, packages.clone(), rustc.clone());

        let outfile = BufWriter::new(paths::create(sbom_output_file)?);
        serde_json::to_writer(outfile, &sbom)?;
    }

    Ok(())
}

/// Fetch all dependencies, including transitive ones. A dependency can also appear multiple times
/// if it's using different settings, e.g. profile, features or crate versions.
///
/// A package's `dependencies` is a list of indices into the `packages` array.
fn collect_packages(build_runner: &mut BuildRunner<'_, '_>, unit: &Unit) -> Vec<SbomPackage> {
    let unit_graph = &build_runner.bcx.unit_graph;
    let root_deps = build_runner.unit_deps(unit);
    let root_profile = &unit.profile;

    let mut packages = Vec::new();
    let mut queue: BTreeSet<&UnitDep> = root_deps.iter().collect();
    let mut visited_dependencies = BTreeSet::new();

    // each package should appear only once in the `packages` block
    while let Some(dependency) = queue.pop_first() {
        if visited_dependencies.contains(dependency) {
            continue;
        }

        let mut dependencies: BTreeSet<&UnitDep> = unit_graph[&dependency.unit].iter().collect();

        // each seen package is returned
        packages.push(SbomPackage::new(dependency, root_profile));

        // append dependencies to back of queue
        queue.append(&mut dependencies);
        visited_dependencies.insert(dependency);
    }

    // Collect all indices for each package's dependencies
    //
    // At this point `visited_packages` contains the [`UnitDep`] the `packages` Vec is generated with.
    // They have the same number of objects in the same order, therefore using an index is fine.
    for (index, package) in visited_dependencies.iter().enumerate() {
        let dependencies: BTreeSet<&UnitDep> = unit_graph[&package.unit].iter().collect();

        let mut indices = dependencies
            .iter()
            .filter_map(|dep| {
                visited_dependencies
                    .iter()
                    .position(|unit_dep| dep == unit_dep)
            })
            .collect::<Vec<_>>();

        if let Some(package) = packages.get_mut(index) {
            package.dependencies.append(&mut indices);
        }
    }

    packages
}
