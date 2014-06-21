use toml;
use url;
use url::Url;
use std::collections::HashMap;
use serialize::Decodable;

use core::{SourceId,GitKind};
use core::manifest::{LibKind,Lib};
use core::{Summary,Manifest,Target,Dependency,PackageId};
use util::{CargoResult, Require, human};

pub fn to_manifest(contents: &[u8],
                   source_id: &SourceId) -> CargoResult<(Manifest, Vec<Path>)> {
    let root = cargo_try!(toml::parse_from_bytes(contents).map_err(|_| {
        human("Cargo.toml is not valid Toml")
    }));
    let toml = cargo_try!(toml_to_manifest(root).map_err(|_| {
        human("Cargo.toml is not a valid manifest")
    }));

    toml.to_manifest(source_id)
}

fn toml_to_manifest(root: toml::Value) -> CargoResult<TomlManifest> {
    fn decode<T: Decodable<toml::Decoder,toml::Error>>(root: &toml::Value,
                                                       path: &str)
        -> Result<T, toml::Error>
    {
        let root = match root.lookup(path) {
            Some(val) => val,
            None => return Err(toml::ParseError)
        };
        toml::from_toml(root.clone())
    }

    let project = cargo_try!(decode(&root, "project"));
    let lib = decode(&root, "lib").ok();
    let bin = decode(&root, "bin").ok();

    let deps = root.lookup("dependencies");

    let deps = match deps {
        Some(deps) => {
            let table = cargo_try!(deps.get_table().require(|| {
                human("dependencies must be a table")
            })).clone();

            let mut deps: HashMap<String, TomlDependency> = HashMap::new();

            for (k, v) in table.iter() {
                match v {
                    &toml::String(ref string) => {
                        deps.insert(k.clone(), SimpleDep(string.clone()));
                    },
                    &toml::Table(ref table) => {
                        let mut details = HashMap::<String, String>::new();

                        for (k, v) in table.iter() {
                            let v = cargo_try!(v.get_str().require(|| {
                                human("dependency values must be string")
                            }));

                            details.insert(k.clone(), v.clone());
                        }

                        let version = cargo_try!(details.find_equiv(&"version")
                                           .require(|| {
                            human("dependencies must include a version")
                        })).clone();

                        deps.insert(k.clone(),
                                    DetailedDep(DetailedTomlDependency {
                            version: version,
                            other: details
                        }));
                    },
                    _ => ()
                }
            }

            Some(deps)
        },
        None => None
    };

    Ok(TomlManifest {
        project: box project,
        lib: lib,
        bin: bin,
        dependencies: deps,
    })
}

type TomlLibTarget = TomlTarget;
type TomlBinTarget = TomlTarget;

/*
 * TODO: Make all struct fields private
 */

#[deriving(Encodable,PartialEq,Clone,Show)]
pub enum TomlDependency {
    SimpleDep(String),
    DetailedDep(DetailedTomlDependency)
}

#[deriving(Encodable,PartialEq,Clone,Show)]
pub struct DetailedTomlDependency {
    version: String,
    other: HashMap<String, String>
}

#[deriving(Encodable,PartialEq,Clone)]
pub struct TomlManifest {
    project: Box<TomlProject>,
    lib: Option<Vec<TomlLibTarget>>,
    bin: Option<Vec<TomlBinTarget>>,
    dependencies: Option<HashMap<String, TomlDependency>>,
}

#[deriving(Decodable,Encodable,PartialEq,Clone,Show)]
pub struct TomlProject {
    pub name: String,
    pub version: String,
    pub authors: Vec<String>,
    build: Option<String>,
}

impl TomlProject {
    pub fn to_package_id(&self, namespace: &Url) -> PackageId {
        PackageId::new(self.name.as_slice(), self.version.as_slice(), namespace)
    }
}

impl TomlManifest {
    pub fn to_manifest(&self, source_id: &SourceId)
        -> CargoResult<(Manifest, Vec<Path>)>
    {
        let mut sources = vec!();
        let mut nested_paths = vec!();

        // Get targets
        let targets = normalize(self.lib.as_ref().map(|l| l.as_slice()),
                                self.bin.as_ref().map(|b| b.as_slice()));

        if targets.is_empty() {
            debug!("manifest has no build targets; project={}", self.project);
        }

        let mut deps = Vec::new();

        // Collect the deps
        match self.dependencies {
            Some(ref dependencies) => {
                for (n, v) in dependencies.iter() {
                    let (version, source_id) = match *v {
                        SimpleDep(ref string) => {
                            (string.clone(), SourceId::for_central())
                        },
                        DetailedDep(ref details) => {
                            let new_source_id = details.other.find_equiv(&"git");
                            let new_source_id = new_source_id.map(|git| {
                                // TODO: Don't unwrap here
                                let kind = GitKind("master".to_str());
                                let url = url::from_str(git.as_slice()).unwrap();
                                let source_id = SourceId::new(kind, url);
                                // TODO: Don't do this for path
                                sources.push(source_id.clone());
                                source_id
                            }).or_else(|| {
                                details.other.find_equiv(&"path").map(|path| {
                                    nested_paths.push(Path::new(path.as_slice()));
                                    source_id.clone()
                                })
                            }).unwrap_or(SourceId::for_central());

                            (details.version.clone(), new_source_id)
                        }
                    };

                    deps.push(cargo_try!(Dependency::parse(n.as_slice(),
                                                     version.as_slice(),
                                                     &source_id)))
                }
            }
            None => ()
        }

        Ok((Manifest::new(
                &Summary::new(&self.project.to_package_id(source_id.get_url()),
                              deps.as_slice()),
                targets.as_slice(),
                &Path::new("target"),
                sources,
                self.project.build.clone()),
           nested_paths))
    }
}

#[deriving(Decodable,Encodable,PartialEq,Clone,Show)]
struct TomlTarget {
    name: String,
    crate_type: Option<Vec<String>>,
    path: Option<String>
}

fn normalize(lib: Option<&[TomlLibTarget]>,
             bin: Option<&[TomlBinTarget]>) -> Vec<Target> {
    log!(4, "normalizing toml targets; lib={}; bin={}", lib, bin);

    fn lib_targets(dst: &mut Vec<Target>, libs: &[TomlLibTarget]) {
        let l = &libs[0];
        let path = l.path.clone().unwrap_or_else(|| format!("src/{}.rs", l.name));
        let crate_types = l.crate_type.clone().and_then(|kinds| {
            LibKind::from_strs(kinds).ok()
        }).unwrap_or_else(|| vec!(Lib));
        dst.push(Target::lib_target(l.name.as_slice(), crate_types,
                                    &Path::new(path)));
    }

    fn bin_targets(dst: &mut Vec<Target>, bins: &[TomlBinTarget],
                   default: |&TomlBinTarget| -> String) {
        for bin in bins.iter() {
            let path = bin.path.clone().unwrap_or_else(|| default(bin));
            dst.push(Target::bin_target(bin.name.as_slice(), &Path::new(path)));
        }
    }

    let mut ret = Vec::new();

    match (lib, bin) {
        (Some(ref libs), Some(ref bins)) => {
            lib_targets(&mut ret, libs.as_slice());
            bin_targets(&mut ret, bins.as_slice(),
                        |bin| format!("src/bin/{}.rs", bin.name));
        },
        (Some(ref libs), None) => {
            lib_targets(&mut ret, libs.as_slice());
        },
        (None, Some(ref bins)) => {
            bin_targets(&mut ret, bins.as_slice(),
                        |bin| format!("src/{}.rs", bin.name));
        },
        (None, None) => ()
    }

    ret
}
