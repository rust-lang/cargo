use serialize::Decodable;
use std::collections::HashMap;
use std::str;
use toml;
use url::Url;
use url;

use core::{SourceId,GitKind};
use core::manifest::{LibKind,Lib};
use core::{Summary,Manifest,Target,Dependency,PackageId};
use util::{CargoResult, Require, human};

pub fn to_manifest(contents: &[u8],
                   source_id: &SourceId) -> CargoResult<(Manifest, Vec<Path>)> {
    let contents = try!(str::from_utf8(contents).require(|| {
        human("Cargo.toml is not valid UTF-8")
    }));
    let root = try!(parse(contents, "Cargo.toml"));
    let mut d = toml::Decoder::new(toml::Table(root));
    let toml_manifest: TomlManifest = match Decodable::decode(&mut d) {
        Ok(t) => t,
        Err(e) => return Err(human(format!("Cargo.toml is not a valid \
                                            manifest\n\n{}", e)))
    };

    toml_manifest.to_manifest(source_id).map_err(|err| {
        human(format!("Cargo.toml is not a valid manifest\n\n{}", err))
    })
}

pub fn parse(toml: &str, file: &str) -> CargoResult<toml::Table> {
    let mut parser = toml::Parser::new(toml.as_slice());
    match parser.parse() {
        Some(toml) => Ok(toml),
        None => {
            let mut error_str = format!("could not parse input TOML\n");
            for error in parser.errors.iter() {
                let (loline, locol) = parser.to_linecol(error.lo);
                let (hiline, hicol) = parser.to_linecol(error.hi);
                error_str.push_str(format!("{}:{}:{}{} {}",
                                           file,
                                           loline + 1, locol + 1,
                                           if loline != hiline || locol != hicol {
                                               format!("-{}:{}", hiline + 1,
                                                       hicol + 1)
                                           } else {
                                               "".to_string()
                                           },
                                           error.desc).as_slice());
            }
            Err(human(error_str))
        }
    }
}

type TomlLibTarget = TomlTarget;
type TomlBinTarget = TomlTarget;

/*
 * TODO: Make all struct fields private
 */

#[deriving(Encodable,Decodable,PartialEq,Clone,Show)]
pub enum TomlDependency {
    SimpleDep(String),
    DetailedDep(DetailedTomlDependency)
}

#[deriving(Encodable,Decodable,PartialEq,Clone,Show)]
pub struct DetailedTomlDependency {
    version: String,
    path: Option<String>,
    git: Option<String>,
}

#[deriving(Encodable,Decodable,PartialEq,Clone)]
pub struct TomlManifest {
    package: Option<Box<TomlProject>>,
    project: Option<Box<TomlProject>>,
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
                            let new_source_id = details.git.as_ref().map(|git| {
                                // TODO: Don't unwrap here
                                let kind = GitKind("master".to_str());
                                let url = url::from_str(git.as_slice()).unwrap();
                                let source_id = SourceId::new(kind, url);
                                // TODO: Don't do this for path
                                sources.push(source_id.clone());
                                source_id
                            }).or_else(|| {
                                details.path.as_ref().map(|path| {
                                    nested_paths.push(Path::new(path.as_slice()));
                                    source_id.clone()
                                })
                            }).unwrap_or(SourceId::for_central());

                            (details.version.clone(), new_source_id)
                        }
                    };

                    deps.push(try!(Dependency::parse(n.as_slice(),
                                                     version.as_slice(),
                                                     &source_id)))
                }
            }
            None => ()
        }

        let project = self.project.as_ref().or_else(|| self.package.as_ref());
        let project = try!(project.require(|| human("No `package` or `project` section found.")));

        Ok((Manifest::new(
                &Summary::new(&project.to_package_id(source_id.get_url()),
                              deps.as_slice()),
                targets.as_slice(),
                &Path::new("target"),
                sources,
                project.build.clone()),
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
