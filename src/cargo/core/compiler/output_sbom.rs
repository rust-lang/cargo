//! cargo-sbom precursor files for external tools to create SBOM files from.
//! See [`output_sbom`] for more.

use std::{
    io::{BufWriter, Write},
    path::Path,
};

use cargo_util::paths::{self, normalize_path};
use serde::Serialize;

use crate::{core::compiler::FileFlavor, util::internal, CargoResult};

use super::{BuildRunner, Unit};

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
struct Test {
    format_version: SbomFormatVersion<1>,
    package_id: String,
}

/// Saves a `<artifact>.cargo-sbom.json` file for the given [`Unit`].
///
/// TODO add description
pub fn output_sbom(build_runner: &mut BuildRunner<'_, '_>, unit: &Unit) -> CargoResult<()> {
    let _bcx = build_runner.bcx;

    // TODO collcet build & unit data, then transform into JSON output
    for output in build_runner
        .outputs(unit)?
        .iter()
        .filter(|o| matches!(o.flavor, FileFlavor::Normal | FileFlavor::Linkable))
    {
        if let Some(ref link_dst) = output.hardlink {
            let output_path = link_dst.with_extension("cargo-sbom.json");

            let test = Test {
                format_version: SbomFormatVersion,
                package_id: "Test".to_string(),
            };

            let mut outfile = BufWriter::new(paths::create(output_path)?);
            let output = serde_json::to_string_pretty(&test)?;
            write!(outfile, "{}", output)?;
        }
    }

    Ok(())
}
