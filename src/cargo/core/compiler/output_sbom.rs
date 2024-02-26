//! cargo-sbom precursor files for external tools to create SBOM files from.
//! See [`output_sbom`] for more.

use std::io::{BufWriter, Write};

use cargo_util::paths::{self};
use cargo_util_schemas::core::PackageIdSpec;
use serde::Serialize;

use crate::{
    core::{compiler::FileFlavor, Target, TargetKind},
    CargoResult,
};

use super::{BuildRunner, CrateType, Unit};

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
struct Sbom {
    format_version: SbomFormatVersion<1>,
    package_id: PackageIdSpec,
    name: String,
    version: String,
    target: SbomTarget,
    dependencies: Vec<String>,
    features: Vec<String>,
}

impl Sbom {
    pub fn new(unit: &Unit) -> Self {
        let package_id = unit.pkg.summary().package_id().to_spec();
        let name = unit.pkg.name().to_string();
        let version = unit.pkg.version().to_string();
        let features = unit.features.iter().map(|f| f.to_string()).collect();
        let target: SbomTarget = (&unit.target).into();

        Self {
            format_version: SbomFormatVersion,
            package_id,
            name,
            version,
            target,
            dependencies: Vec::new(),
            features,
        }
    }
}

/// Saves a `<artifact>.cargo-sbom.json` file for the given [`Unit`].
///
pub fn output_sbom(build_runner: &mut BuildRunner<'_, '_>, unit: &Unit) -> CargoResult<()> {
    let _bcx = build_runner.bcx;

    // TODO collect build & unit data, then transform into JSON output
    for output in build_runner
        .outputs(unit)?
        .iter()
        .filter(|o| matches!(o.flavor, FileFlavor::Normal | FileFlavor::Linkable))
    {
        if let Some(ref link_dst) = output.hardlink {
            let output_path = link_dst.with_extension("cargo-sbom.json");

            let sbom = Sbom::new(unit);

            let mut outfile = BufWriter::new(paths::create(output_path)?);
            let output = serde_json::to_string_pretty(&sbom)?;
            write!(outfile, "{}", output)?;
        }
    }

    Ok(())
}
