use std::path::{Path, PathBuf};

use serde::ser;
use serde::Serialize;
use serde_json::{self, json, value::RawValue};

use crate::core::{compiler::CompileMode, PackageId, Target};

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
    pub manifest_path: &'a Path,
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
    pub manifest_path: PathBuf,
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
    pub debuginfo: Option<ArtifactDebuginfo>,
    pub debug_assertions: bool,
    pub overflow_checks: bool,
    pub test: bool,
}

/// Internally this is an enum with different variants, but keep using 0/1/2 as integers for compatibility.
#[derive(Serialize)]
#[serde(untagged)]
pub enum ArtifactDebuginfo {
    Int(u32),
    Named(&'static str),
}

#[derive(Serialize)]
pub struct BuildScript<'a> {
    pub package_id: PackageId,
    pub linked_libs: &'a [String],
    pub linked_paths: &'a [String],
    pub cfgs: &'a [String],
    pub env: &'a [(String, String)],
    pub out_dir: &'a Path,
}

impl<'a> Message for BuildScript<'a> {
    fn reason(&self) -> &str {
        "build-script-executed"
    }
}

#[derive(Serialize)]
pub struct TimingInfo<'a> {
    pub package_id: PackageId,
    pub target: &'a Target,
    pub mode: CompileMode,
    pub duration: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rmeta_time: Option<f64>,
}

impl<'a> Message for TimingInfo<'a> {
    fn reason(&self) -> &str {
        "timing-info"
    }
}

#[derive(Serialize)]
pub struct BuildFinished {
    pub success: bool,
}

impl Message for BuildFinished {
    fn reason(&self) -> &str {
        "build-finished"
    }
}
