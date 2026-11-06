use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::core::{CrateType, PackageIdSpec};

/// Typed version of a SBOM format version number.
#[derive(Serialize, Deserialize, Copy, Clone, Debug, Ord, PartialOrd, Eq, PartialEq)]
#[cfg_attr(feature = "unstable-schema", derive(schemars::JsonSchema))]
pub struct SbomFormatVersion(pub u32);

#[derive(Debug, Copy, Clone, PartialEq, PartialOrd, Eq, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "unstable-schema", derive(schemars::JsonSchema))]
pub enum SbomDependencyType {
    /// A dependency linked to the artifact produced by this unit.
    Normal,
    /// A dependency needed to run the build for this unit (e.g. a build script or proc-macro).
    /// The dependency is not linked to the artifact produced by this unit.
    Build,
}

/// Zero-based index into `crates` array of the crate described by this SBOM.
#[derive(Serialize, Deserialize, Copy, Clone, Debug, Ord, PartialOrd, Eq, PartialEq)]
#[cfg_attr(feature = "unstable-schema", derive(schemars::JsonSchema))]
pub struct SbomIndex(pub usize);

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "unstable-schema", derive(schemars::JsonSchema))]
pub struct SbomDependency {
    pub index: SbomIndex,
    pub kind: SbomDependencyType,
}

#[derive(Clone, Debug)]
#[cfg_attr(
    feature = "unstable-schema",
    derive(schemars::JsonSchema),
    schemars(with = "Vec<String>")
)]
pub enum SbomTargetKind {
    Lib(Vec<CrateType>),
    Bin,
    Test,
    Bench,
    Example,
    CustomBuild,
}

impl Serialize for SbomTargetKind {
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        use self::SbomTargetKind::*;
        match self {
            Lib(kinds) => s.collect_seq(kinds.iter().map(|t| t.to_string())),
            Bin => ["bin"].serialize(s),
            Example => ["example"].serialize(s),
            Test => ["test"].serialize(s),
            CustomBuild => ["custom-build"].serialize(s),
            Bench => ["bench"].serialize(s),
        }
    }
}

impl<'de> Deserialize<'de> for SbomTargetKind {
    fn deserialize<D>(d: D) -> Result<Self, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        use self::SbomTargetKind::*;
        let kind: Vec<String> = Deserialize::deserialize(d)?;
        Ok(match kind.as_slice() {
            [name] if name == "bin" => Bin,
            [name] if name == "example" => Example,
            [name] if name == "test" => Test,
            [name] if name == "custom-build" => CustomBuild,
            [name] if name == "bench" => Bench,
            _ => Lib(kind.into_iter().map(CrateType::from).collect()),
        })
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "unstable-schema", derive(schemars::JsonSchema))]
pub struct SbomCrate {
    pub id: PackageIdSpec,
    pub features: Vec<String>,
    pub dependencies: Vec<SbomDependency>,
    pub kind: SbomTargetKind,
}

impl SbomCrate {
    pub fn new(package_id: PackageIdSpec, features: Vec<String>, kind: SbomTargetKind) -> Self {
        Self {
            id: package_id,
            features,
            dependencies: Vec::new(),
            kind,
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "unstable-schema", derive(schemars::JsonSchema))]
pub struct SbomRustc {
    pub version: String,
    pub wrapper: Option<PathBuf>,
    pub workspace_wrapper: Option<PathBuf>,
    pub commit_hash: Option<String>,
    pub host: String,
    pub verbose_version: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(
    feature = "unstable-schema",
    derive(schemars::JsonSchema),
    schemars(title = "Cargo SBOM precursor")
)]
/// This is the schema used to generate `*.cargo-sbom.json` files with -Zsbom enabled.
pub struct Sbom {
    pub version: SbomFormatVersion,
    pub root: SbomIndex,
    pub crates: Vec<SbomCrate>,
    pub rustc: SbomRustc,
    pub target: String,
}

#[cfg(feature = "unstable-schema")]
#[test]
fn matches_schema() {
    let schema = schemars::schema_for!(Sbom);
    let dump = serde_json::to_string_pretty(&schema).unwrap();
    snapbox::assert_data_eq!(dump, snapbox::file!("../cargo-sbom.schema.json").raw());
}
