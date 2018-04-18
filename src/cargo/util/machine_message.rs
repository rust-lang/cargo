use serde::ser;
use serde_json::{self, Value};

use core::{PackageId, Target};

pub trait Message: ser::Serialize {
    fn reason(&self) -> &str;
}

pub fn emit<T: Message>(t: &T) {
    let mut json: Value = serde_json::to_value(t).unwrap();
    json["reason"] = json!(t.reason());
    println!("{}", json);
}

#[derive(Serialize)]
pub struct FromCompiler<'a> {
    pub package_id: &'a PackageId,
    pub target: &'a Target,
    pub message: serde_json::Value,
}

impl<'a> Message for FromCompiler<'a> {
    fn reason(&self) -> &str {
        "compiler-message"
    }
}

#[derive(Serialize)]
pub struct Artifact<'a> {
    pub package_id: &'a PackageId,
    pub target: &'a Target,
    pub profile: ArtifactProfile,
    pub features: Vec<String>,
    pub filenames: Vec<String>,
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
    pub package_id: &'a PackageId,
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
