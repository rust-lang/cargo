use std::path::PathBuf;

use serde::Serialize;

use crate::core::{CrateType, PackageIdSpec};

/// Typed version of a SBOM format version number.
#[derive(Serialize, Copy, Clone, Debug, Ord, PartialOrd, Eq, PartialEq)]
pub struct SbomFormatVersion(pub u32);

#[derive(Debug, Copy, Clone, PartialEq, PartialOrd, Eq, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SbomDependencyType {
    /// A dependency linked to the artifact produced by this unit.
    Normal,
    /// A dependency needed to run the build for this unit (e.g. a build script or proc-macro).
    /// The dependency is not linked to the artifact produced by this unit.
    Build,
}

#[derive(Serialize, Copy, Clone, Debug, Ord, PartialOrd, Eq, PartialEq)]
pub struct SbomIndex(pub usize);

#[derive(Serialize, Clone, Debug)]
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

#[derive(Serialize, Clone, Debug)]
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

#[derive(Serialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct SbomRustc {
    pub version: String,
    pub wrapper: Option<PathBuf>,
    pub workspace_wrapper: Option<PathBuf>,
    pub commit_hash: Option<String>,
    pub host: String,
    pub verbose_version: String,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub struct Sbom {
    pub version: SbomFormatVersion,
    pub root: SbomIndex,
    pub crates: Vec<SbomCrate>,
    pub rustc: SbomRustc,
    pub target: String,
}
