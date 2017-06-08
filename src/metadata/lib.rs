extern crate regex;
extern crate rustc_serialize;

use std::collections::HashMap;
use std::error::Error as StdError;
use std::fmt;

use regex::Regex;
use rustc_serialize::{Decodable, Decoder};
use rustc_serialize::json::{decode, DecoderError};

#[derive(Debug, PartialEq)]
enum InnerError {
    DecoderError(DecoderError),
    VersionError(u32),
}

#[derive(Debug, PartialEq)]
pub struct Error {
    error: InnerError,
}

impl StdError for Error {
    fn description(&self) -> &str {
        match self.error {
            InnerError::DecoderError(ref e) => e.description(),
            InnerError::VersionError(_) => "version error",
        }
    }

    fn cause(&self) -> Option<&StdError> {
        match self.error {
            InnerError::DecoderError(ref e) => Some(e),
            InnerError::VersionError(_) => None,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&self, f)
    }
}

impl From<DecoderError> for Error {
    fn from(e: DecoderError) -> Error {
        Error { error: InnerError::DecoderError(e) }
    }
}

#[derive(Debug, Eq, PartialEq, RustcDecodable)]
pub struct CargoMetadata {
    packages: Vec<Package>,
    workspace_members: Option<Vec<()>>,
    resolve: Option<Resolve>,
    version: u32,
}

impl CargoMetadata {
    pub fn from_json(json: &str) -> Result<CargoMetadata, Error> {
        #[derive(RustcDecodable)]
        struct MetadataVersion {
            version: u32,
        }
        let d: MetadataVersion = try!(decode(json));
        if d.version != 1 {
            return Err(Error { error: InnerError::VersionError(d.version) });
        }
        decode(json).map_err(|e| e.into())
    }
}

#[derive(Debug, Eq, PartialEq, RustcDecodable)]
pub struct Package {
    dependencies: Vec<Dependency>,
    features: HashMap<String, Vec<String>>,
    id: PackageId,
    manifest_path: String,
    name: String,
    source: Option<String>,
    targets: Vec<Target>,
    version: String,
}

#[derive(Debug, PartialEq, Eq)]
pub struct PackageId {
    name: String,
    version: String,
    source_id: String,
}

impl Decodable for PackageId {
    fn decode<D: Decoder>(d: &mut D) -> Result<PackageId, D::Error> {
        let string: String = try!(Decodable::decode(d));
        let regex = Regex::new(r"^([^ ]+) ([^ ]+) \(([^\)]+)\)$").unwrap();
        let captures = try!(regex.captures(&string).ok_or_else(|| {
            d.error("invalid serialized PackageId")
        }));

        let name = captures.at(1).unwrap();
        let version = captures.at(2).unwrap();
        let url = captures.at(3).unwrap();

        Ok(PackageId {
            name: name.to_string(),
            version: version.to_string(),
            source_id: url.to_string(),
        })
    }
}

#[derive(Debug, Eq, PartialEq, RustcDecodable)]
pub struct Dependency {
    features: Vec<()>,
    kind: Option<String>,
    name: String,
    optional: bool,
    req: String,
    source: Option<String>,
    target: (),
    uses_default_features: bool,
}

#[derive(Debug, Eq, PartialEq, RustcDecodable)]
pub struct Target {
    kind: Vec<String>,
    name: String,
    src_path: String,
}

#[derive(Debug, Eq, PartialEq, RustcDecodable)]
pub struct Resolve {
    nodes: Vec<Node>,
    root: PackageId,
}

#[derive(Debug, Eq, PartialEq, RustcDecodable)]
pub struct Node {
    dependencies: Vec<PackageId>,
    id: PackageId,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashMap, HashSet};

    #[test]
    fn test_no_deps() {
        let json = include_str!("no-deps.json");
        let decoded = CargoMetadata::from_json(json).unwrap();
        assert_eq!(CargoMetadata {
            packages: vec!(Package {
                dependencies: vec!(Dependency {
                    features: vec!(),
                    kind: None,
                    name: "rustc-serialize".to_string(),
                    optional: false,
                    req: "^0.3".to_string(),
                    source: Some("registry+https://github.com/rust-lang/crates.io-index"
                        .to_string()),
                    target: (),
                    uses_default_features: true,
                }),
                features: HashMap::new(),
                id: PackageId {
                    name: "cargo-metadata".to_string(),
                    version: "0.1.0".to_string(),
                    source_id: "path+file:///home/user/cargo/src/metadata".to_string(),
                },
                manifest_path: "/home/user/cargo/src/metadata/Cargo.toml".to_string(),
                name: "cargo-metadata".to_string(),
                source: None,
                targets: vec!(Target {
                    kind: vec!("lib".to_string()),
                    name: "cargo_metadata".to_string(),
                    src_path: "lib.rs".to_string(),
                }),
                version: "0.1.0".to_string(),
            }),
            workspace_members: None,
            resolve: None,
            version: 1,
        }, decoded);
    }

    #[test]
    fn test_deps() {
        let json = include_str!("deps.json");
        let decoded = CargoMetadata::from_json(json).unwrap();
        assert_eq!(CargoMetadata {
            packages: vec!(Package {
                dependencies: vec!(Dependency {
                    features: vec!(),
                    kind: None,
                    name: "rustc-serialize".to_string(),
                    optional: false,
                    req: "^0.3".to_string(),
                    source: Some("registry+https://github.com/rust-lang/crates.io-index"
                        .to_string()),
                    target: (),
                    uses_default_features: true,
                }),
                features: HashMap::new(),
                id: PackageId {
                    name: "cargo-metadata".to_string(),
                    version: "0.1.0".to_string(),
                    source_id: "path+file:///home/user/cargo/src/metadata".to_string(),
                },
                manifest_path: "/home/user/cargo/src/metadata/Cargo.toml".to_string(),
                name: "cargo-metadata".to_string(),
                source: None,
                targets: vec!(Target {
                    kind: vec!("lib".to_string()),
                    name: "cargo_metadata".to_string(),
                    src_path: "lib.rs".to_string(),
                }),
                version: "0.1.0".to_string(),
            },
            Package {
                dependencies: vec!(Dependency {
                    features: vec!(),
                    kind: Some("dev".to_string()),
                    name: "rand".to_string(),
                    optional: false,
                    req: "^0.3".to_string(),
                    source: Some("registry+https://github.com/rust-lang/crates.io-index"
                        .to_string()),
                    target: (),
                    uses_default_features: true,
                }),
                features: HashMap::new(),
                id: PackageId {
                    name: "rustc-serialize".to_string(),
                    version: "0.3.19".to_string(),
                    source_id: "registry+https://github.com/rust-lang/crates.io-index".to_string(),
                },
                manifest_path: "/home/user/.cargo/registry/src/github.com-1ecc6299db9ec823/\
rustc-serialize-0.3.19/Cargo.toml".to_string(),
                name: "rustc-serialize".to_string(),
                source: Some("registry+https://github.com/rust-lang/crates.io-index".to_string()),
                targets: vec!(Target {
                    kind: vec!("lib".to_string()),
                    name: "rustc-serialize".to_string(),
                    src_path: "/home/user/.cargo/registry/src/github.com-1ecc6299db9ec823/\
rustc-serialize-0.3.19/src/lib.rs".to_string(),
                },
                Target {
                    kind: vec!("bench".to_string()),
                    name: "hex".to_string(),
                    src_path: "/home/user/.cargo/registry/src/github.com-1ecc6299db9ec823/\
rustc-serialize-0.3.19/benches/hex.rs".to_string(),
                },
                Target {
                    kind: vec!("bench".to_string()),
                    name: "base64".to_string(),
                    src_path: "/home/user/.cargo/registry/src/github.com-1ecc6299db9ec823/\
rustc-serialize-0.3.19/benches/base64.rs".to_string(),
                },
                Target {
                    kind: vec!("bench".to_string()),
                    name: "json".to_string(),
                    src_path: "/home/user/.cargo/registry/src/github.com-1ecc6299db9ec823/\
rustc-serialize-0.3.19/benches/json.rs".to_string(),
                }),
                version: "0.3.19".to_string(),
            }),
            workspace_members: None,
            resolve: Some(Resolve {
                nodes: vec!(Node {
                    dependencies: vec!(PackageId {
                        name: "rustc-serialize".to_string(),
                        version: "0.3.19".to_string(),
                        source_id: "registry+https://github.com/rust-lang/crates.io-index"
                            .to_string(),
                    }),
                    id: PackageId {
                        name: "cargo-metadata".to_string(),
                        version: "0.1.0".to_string(),
                        source_id: "path+file:///home/user/cargo/src/metadata".to_string(),
                    },
                },
                Node {
                    dependencies: vec!(),
                    id: PackageId {
                        name: "rustc-serialize".to_string(),
                        version: "0.3.19".to_string(),
                        source_id: "registry+https://github.com/rust-lang/crates.io-index"
                            .to_string(),
                    },
                }),
                root: PackageId {
                    name: "cargo-metadata".to_string(),
                    version: "0.1.0".to_string(),
                    source_id: "path+file:///home/user/cargo/src/metadata".to_string(),
                },
            }),
            version: 1,
        }, decoded);
    }

    #[test]
    fn test_rustc() {
        let json = include_str!("rustc.json");
        let decoded = CargoMetadata::from_json(json).unwrap();
        let resolve = decoded.resolve.as_ref().unwrap();
        let root = resolve.root.name.as_str();
        assert_eq!("rustc-main", root);
        let mut todo = vec!(root);
        let mut trans = HashSet::new();
        trans.insert(root);
        while let Some(next) = todo.pop() {
            let node = resolve.nodes.iter().find(|x| x.id.name == next).unwrap();
            todo.extend(node.dependencies.iter().map(|x| x.name.as_str()));
            trans.extend(node.dependencies.iter().map(|x| x.name.as_str()));
        }
        let expected: HashSet<&str> = vec!(
            "arena", "rustc_resolve", "log", "proc_macro", "rustc_privacy", "rustc_typeck",
            "rustc_metadata", "rustc_borrowck", "rustc_passes", "rustc_plugin", "rustc_const_eval",
            "rustc_macro", "graphviz", "rustc_driver", "rustc_llvm", "rustc_errors", "rustc",
            "gcc", "rustc_mir", "rustc-main", "rustc_data_structures", "rustc_const_math",
            "rustc_save_analysis", "build_helper", "rustc_back", "serialize", "rustc_trans",
            "rustc_platform_intrinsics", "rustdoc", "flate", "syntax_ext", "rustc_lint",
            "syntax_pos", "fmt_macros", "rustc_incremental", "syntax", "rustc_bitflags"
        ).into_iter().collect();
        assert_eq!(expected, trans);
    }

    #[test]
    fn test_std_shim() {
        let json = include_str!("std_shim.json");
        let decoded = CargoMetadata::from_json(json).unwrap();
        let resolve = decoded.resolve.as_ref().unwrap();
        let root = resolve.root.name.as_str();
        assert_eq!("std_shim", root);
        let mut todo = vec!(root);
        let mut trans = HashSet::new();
        trans.insert(root);
        while let Some(next) = todo.pop() {
            let node = resolve.nodes.iter().find(|x| x.id.name == next).unwrap();
            todo.extend(node.dependencies.iter().map(|x| x.name.as_str()));
            trans.extend(node.dependencies.iter().map(|x| x.name.as_str()));
        }
        let expected: HashSet<&str> = vec!(
            "std_shim", "rustc_unicode", "build_helper", "rand", "gcc", "libc", "panic_abort",
            "std", "alloc_system", "collections", "compiler_builtins", "core", "unwind", "alloc",
            "panic_unwind"
        ).into_iter().collect();
        assert_eq!(expected, trans);
    }

    #[test]
    fn test_test_shim() {
        let json = include_str!("test_shim.json");
        let decoded = CargoMetadata::from_json(json).unwrap();
        let resolve = decoded.resolve.as_ref().unwrap();
        let root = resolve.root.name.as_str();
        assert_eq!("test_shim", root);
        let mut todo = vec!(root);
        let mut trans = HashSet::new();
        trans.insert(root);
        while let Some(next) = todo.pop() {
            let node = resolve.nodes.iter().find(|x| x.id.name == next).unwrap();
            todo.extend(node.dependencies.iter().map(|x| x.name.as_str()));
            trans.extend(node.dependencies.iter().map(|x| x.name.as_str()));
        }
        let expected: HashSet<&str> = vec!(
            "test", "term", "getopts", "test_shim"
        ).into_iter().collect();
        assert_eq!(expected, trans);
    }

    #[test]
    fn test_invalid_version() {
        let json = r#"{ "version": 2 }"#;
        assert!(CargoMetadata::from_json(json).is_err());
    }
}
