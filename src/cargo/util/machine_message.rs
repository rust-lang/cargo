use std::path::PathBuf;

use serde::ser;
use serde::Serialize;
use serde_json::{self, json, value::RawValue};

use crate::core::{PackageId, Target};

pub trait Message: ser::Serialize {
    fn reason(&self) -> &str;

    fn to_json_string(&self) -> String {
        let json = serde_json::to_string(self).unwrap();
        assert!(json.starts_with("{\""));
        let reason = json!(self.reason());
        format!("{{\"reason\":{},{}", reason, &json[1..])
    }
}

#[derive(Serialize)]
pub struct FromCompiler<'a> {
    pub package_id: PackageId,
    pub target: &'a Target,
    pub message: Box<RawValue>,
}

impl<'a> Message for FromCompiler<'a> {
    fn reason(&self) -> &str {
        "compiler-message"
    }
}

#[derive(Serialize)]
pub struct Artifact<'a> {
    pub package_id: PackageId,
    pub target: &'a Target,
    pub profile: ArtifactProfile,
    pub features: Vec<String>,
    pub filenames: Vec<PathBuf>,
    pub executable: Option<PathBuf>,
    pub fresh: bool,
}

impl<'a> Message for Artifact<'a> {
    fn reason(&self) -> &str {
        "compiler-artifact"
    }
}

/// This is different from the regular `Profile` to maintain backwards
/// compatibility (in particular, `test` is no longer in `Profile`, but we
/// still want it to be included here).
#[derive(Serialize)]
pub struct ArtifactProfile {
    pub opt_level: &'static str,
    pub debuginfo: Option<u32>,
    pub debug_assertions: bool,
    pub overflow_checks: bool,
    pub test: bool,
}

#[derive(Serialize)]
pub struct BuildScript<'a> {
    pub package_id: PackageId,
    pub linked_libs: &'a [String],
    pub linked_paths: &'a [String],
    pub cfgs: &'a [String],
    pub env: &'a [(String, String)],
}

impl<'a> Message for BuildScript<'a> {
    fn reason(&self) -> &str {
        "build-script-executed"
    }
}
