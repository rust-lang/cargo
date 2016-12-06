use rustc_serialize::Encodable;
use rustc_serialize::json::{self, Json};

use core::{PackageId, Target, Profile};

pub trait Message: Encodable {
    fn reason(&self) -> &str;
}

pub fn emit<T: Message>(t: T) {
    let json = json::encode(&t).unwrap();
    let mut map = match json.parse().unwrap() {
        Json::Object(obj) => obj,
        _ => panic!("not a json object"),
    };
    map.insert("reason".to_string(), Json::String(t.reason().to_string()));
    println!("{}", Json::Object(map));
}

#[derive(RustcEncodable)]
pub struct FromCompiler<'a> {
    pub package_id: &'a PackageId,
    pub target: &'a Target,
    pub message: json::Json,
}

impl<'a> Message for FromCompiler<'a> {
    fn reason(&self) -> &str {
        "compiler-message"
    }
}

#[derive(RustcEncodable)]
pub struct Artifact<'a> {
    pub package_id: &'a PackageId,
    pub target: &'a Target,
    pub profile: &'a Profile,
    pub features: Vec<String>,
    pub filenames: Vec<String>,
}

impl<'a> Message for Artifact<'a> {
    fn reason(&self) -> &str {
        "compiler-artifact"
    }
}

#[derive(RustcEncodable)]
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
