use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::core::{CrateType, PackageIdSpec};

/// Typed version of a SBOM format version number.
#[derive(Serialize, Deserialize, Copy, Clone, Debug, Ord, PartialOrd, Eq, PartialEq)]
pub struct SbomFormatVersion(pub u32);

#[derive(Debug, Copy, Clone, PartialEq, PartialOrd, Eq, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SbomDependencyType {
    /// A dependency linked to the artifact produced by this unit.
    Normal,
    /// A dependency needed to run the build for this unit (e.g. a build script or proc-macro).
    /// The dependency is not linked to the artifact produced by this unit.
    Build,
}

#[derive(Serialize, Deserialize, Copy, Clone, Debug, Ord, PartialOrd, Eq, PartialEq)]
pub struct SbomIndex(pub usize);

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub struct SbomDependency {
    pub index: SbomIndex,
    pub kind: SbomDependencyType,
}

#[derive(Clone, Debug)]
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
        let refs: Vec<&str> = kind.iter().map(|s| s.as_str()).collect();
        Ok(match refs.as_slice() {
            ["bin"] => Bin,
            ["example"] => Example,
            ["test"] => Test,
            ["custom-build"] => CustomBuild,
            ["bench"] => Bench,
            [..] => Lib(kind.into_iter().map(|k| CrateType::from(k)).collect()),
        })
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
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
pub struct Sbom {
    pub version: SbomFormatVersion,
    pub root: SbomIndex,
    pub crates: Vec<SbomCrate>,
    pub rustc: SbomRustc,
    pub target: String,
}
