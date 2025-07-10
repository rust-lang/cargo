//! `Cargo.lock` / Lock file schema definition

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use url::Url;

use crate::core::{PackageIdSpec, SourceKind};

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "kebab-case")]
#[cfg_attr(feature = "unstable-schema", derive(schemars::JsonSchema))]
pub struct NormalizedResolve {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package: Option<Vec<NormalizedDependency>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root: Option<NormalizedDependency>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Metadata>,
    #[serde(default, skip_serializing_if = "NormalizedPatch::is_empty")]
    pub patch: NormalizedPatch,
}

pub type Metadata = BTreeMap<String, String>;

#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(rename_all = "kebab-case")]
#[cfg_attr(feature = "unstable-schema", derive(schemars::JsonSchema))]
pub struct NormalizedPatch {
    pub unused: Vec<NormalizedDependency>,
}

impl NormalizedPatch {
    fn is_empty(&self) -> bool {
        self.unused.is_empty()
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "kebab-case")]
#[cfg_attr(feature = "unstable-schema", derive(schemars::JsonSchema))]
pub struct NormalizedDependency {
    pub id: PackageIdSpec,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checksum: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dependencies: Option<Vec<NormalizedPackageId>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replace: Option<NormalizedPackageId>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "kebab-case")]
#[cfg_attr(feature = "unstable-schema", derive(schemars::JsonSchema))]
pub struct NormalizedPackageId {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<NormalizedSourceId>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "kebab-case")]
#[cfg_attr(feature = "unstable-schema", derive(schemars::JsonSchema))]
pub struct NormalizedSourceId {
    #[cfg_attr(feature = "unstable-schema", schemars(with = "String"))]
    pub url: Url,
    pub kind: SourceKind,
}

#[cfg(feature = "unstable-schema")]
#[test]
fn dump_resolve_schema() {
    let schema = schemars::schema_for!(crate::resolve::NormalizedResolve);
    let dump = serde_json::to_string_pretty(&schema).unwrap();
    snapbox::assert_data_eq!(dump, snapbox::file!("../resolve.schema.json").raw());
}
