use rustc_serialize::json;
use core::{PackageId, Target};

#[derive(RustcEncodable)]
pub struct FromCompiler<'a> {
    reason: &'static str,
    package_id: &'a PackageId,
    target: &'a Target,
    message: json::Json,
}

impl<'a> FromCompiler<'a> {
    pub fn new(package_id: &'a PackageId,
               target: &'a Target,
               message: json::Json)
               -> FromCompiler<'a> {
        FromCompiler {
            reason: "compiler-message",
            package_id: package_id,
            target: target,
            message: message,
        }
    }

    pub fn emit(self) {
        let json = json::encode(&self).unwrap();
        println!("{}", json);
    }
}

