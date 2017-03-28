use std::path::{PathBuf, Path};
use std::collections::HashMap;
use std::borrow::Cow;

use serde::ser;
use serde_json::{self, Value};

use core::{PackageId, Target, Profile};
use util::ProcessBuilder;

pub trait Message: ser::Serialize {
    fn reason(&self) -> &str;
}

pub fn emit<T: Message>(t: T) {
    let mut json: Value = serde_json::to_value(&t).unwrap();
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
    pub profile: &'a Profile,
    pub features: Vec<String>,
    pub filenames: Vec<String>,
    pub fresh: bool,
}

impl<'a> Message for Artifact<'a> {
    fn reason(&self) -> &str {
        "compiler-artifact"
    }
}

#[derive(Serialize)]
pub struct BuildScript<'a> {
    pub package_id: &'a PackageId,
    pub linked_libs: &'a [String],
    pub linked_paths: &'a [String],
    pub cfgs: &'a [String],
}

impl<'a> Message for BuildScript<'a> {
    fn reason(&self) -> &str {
        "build-script-executed"
    }
}

#[derive(Serialize)]
pub struct RunProfile<'a> {
    pub program:PathBuf,
    pub args: Vec<Cow<'a, str>>,
    pub env: HashMap<&'a str, Option<Cow<'a, str>>>,
    pub cwd: Option<&'a Path>,
}

impl<'a> RunProfile<'a> {
    pub fn new(process: &ProcessBuilder) -> RunProfile {
        let program = if let Some(cwd) = process.get_cwd() {
            cwd.join(process.get_program())
        } else {
            PathBuf::from(process.get_program())
        };
        assert!(program.is_absolute(), "Running program by relative path without cwd");
        RunProfile {
            program: program,
            args: process.get_args().iter().map(|s| s.to_string_lossy()).collect(),
            env: process.get_envs().iter().map(|(k, v)| {
                (k.as_str(), v.as_ref().map(|s| s.to_string_lossy()))
            }).collect(),
            cwd: process.get_cwd(),
        }
    }
}

impl<'a> Message for RunProfile<'a> {
    fn reason(&self) -> &str {
        "run-profile"
    }
}
