use serialize::Decodable;
use std::collections::HashMap;
use std::str;
use toml;

use core::{SourceId, GitKind};
use core::manifest::{LibKind, Lib, Profile};
use core::{Summary, Manifest, Target, Dependency, PackageId};
use core::source::Location;
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

    let pair = try!(toml_manifest.to_manifest(source_id).map_err(|err| {
        human(format!("Cargo.toml is not a valid manifest\n\n{}", err))
    }));
    let (mut manifest, paths) = pair;
    match d.toml {
        Some(ref toml) => add_unused_keys(&mut manifest, toml, "".to_string()),
        None => {}
    }
    return Ok((manifest, paths));

    fn add_unused_keys(m: &mut Manifest, toml: &toml::Value, key: String) {
        match *toml {
            toml::Table(ref table) => {
                for (k, v) in table.iter() {
                    add_unused_keys(m, v, if key.len() == 0 {
                        k.clone()
                    } else {
                        key + "." + k.as_slice()
                    })
                }
            }
            toml::Array(ref arr) => {
                for v in arr.iter() {
                    add_unused_keys(m, v, key.clone());
                }
            }
            _ => m.add_unused_key(key),
        }
    }


}

pub fn parse(toml: &str, file: &str) -> CargoResult<toml::Table> {
    let mut parser = toml::Parser::new(toml.as_slice());
    match parser.parse() {
        Some(toml) => return Ok(toml),
        None => {}
    }
    let mut error_str = format!("could not parse input TOML\n");
    for error in parser.errors.iter() {
        let (loline, locol) = parser.to_linecol(error.lo);
        let (hiline, hicol) = parser.to_linecol(error.hi);
        error_str.push_str(format!("{}:{}:{}{} {}\n",
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
    version: Option<String>,
    path: Option<String>,
    git: Option<String>,
    branch: Option<String>,
    tag: Option<String>,
    rev: Option<String>
}

#[deriving(Encodable,Decodable,PartialEq,Clone)]
pub struct TomlManifest {
    package: Option<Box<TomlProject>>,
    project: Option<Box<TomlProject>>,
    lib: Option<Vec<TomlLibTarget>>,
    bin: Option<Vec<TomlBinTarget>>,
    dependencies: Option<HashMap<String, TomlDependency>>,
    dev_dependencies: Option<HashMap<String, TomlDependency>>
}

#[deriving(Decodable,Encodable,PartialEq,Clone,Show)]
pub struct TomlProject {
    pub name: String,
    // FIXME #54: should be a Version to be able to be Decodable'd directly.
    pub version: String,
    pub authors: Vec<String>,
    build: Option<String>,
}

impl TomlProject {
    pub fn to_package_id(&self, namespace: &Location) -> CargoResult<PackageId> {
        PackageId::new(self.name.as_slice(), self.version.as_slice(), namespace)
    }
}

struct Context<'a> {
    deps: &'a mut Vec<Dependency>,
    source_id: &'a SourceId,
    source_ids: &'a mut Vec<SourceId>,
    nested_paths: &'a mut Vec<Path>
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

        {

            let mut cx = Context {
                deps: &mut deps,
                source_id: source_id,
                source_ids: &mut sources,
                nested_paths: &mut nested_paths
            };

            // Collect the deps
            try!(process_dependencies(&mut cx, false, self.dependencies.as_ref()));
            try!(process_dependencies(&mut cx, true, self.dev_dependencies.as_ref()));
        }

        let project = self.project.as_ref().or_else(|| self.package.as_ref());
        let project = try!(project.require(|| {
            human("No `package` or `project` section found.")
        }));

        let pkgid = try!(project.to_package_id(source_id.get_location()));
        let summary = Summary::new(&pkgid, deps.as_slice());
        Ok((Manifest::new(
                &summary,
                targets.as_slice(),
                &Path::new("target"),
                sources,
                project.build.clone()),
           nested_paths))
    }
}

fn process_dependencies<'a>(cx: &mut Context<'a>, dev: bool,
                            new_deps: Option<&HashMap<String, TomlDependency>>)
                            -> CargoResult<()> {
    let dependencies = match new_deps {
        Some(ref dependencies) => dependencies,
        None => return Ok(())
    };
    for (n, v) in dependencies.iter() {
        let (version, source_id) = match *v {
            SimpleDep(ref string) => {
                (Some(string.clone()), SourceId::for_central())
            },
            DetailedDep(ref details) => {
                let reference = details.branch.clone()
                    .or_else(|| details.tag.clone())
                    .or_else(|| details.rev.clone())
                    .unwrap_or_else(|| "master".to_str());

                let new_source_id = match details.git {
                    Some(ref git) => {
                        let kind = GitKind(reference.clone());
                        let loc = try!(Location::parse(git.as_slice()));
                        let source_id = SourceId::new(kind, loc);
                        // TODO: Don't do this for path
                        cx.source_ids.push(source_id.clone());
                        Some(source_id)
                    }
                    None => {
                        details.path.as_ref().map(|path| {
                            cx.nested_paths.push(Path::new(path.as_slice()));
                            cx.source_id.clone()
                        })
                    }
                }.unwrap_or(SourceId::for_central());

                (details.version.clone(), new_source_id)
            }
        };

        let mut dep = try!(Dependency::parse(n.as_slice(),
                       version.as_ref().map(|v| v.as_slice()),
                       &source_id));

        if dev { dep = dep.as_dev() }

        cx.deps.push(dep)
    }

    Ok(())
}

#[deriving(Decodable,Encodable,PartialEq,Clone,Show)]
struct TomlTarget {
    name: String,
    crate_type: Option<Vec<String>>,
    path: Option<String>,
    test: Option<bool>
}

fn normalize(lib: Option<&[TomlLibTarget]>,
             bin: Option<&[TomlBinTarget]>) -> Vec<Target> {
    log!(4, "normalizing toml targets; lib={}; bin={}", lib, bin);

    fn target_profiles(target: &TomlTarget) -> Vec<Profile> {
        let mut ret = vec!(Profile::default_dev(), Profile::default_release());

        match target.test {
            Some(true) | None => ret.push(Profile::default_test()),
            _ => {}
        };

        ret
    }

    fn lib_targets(dst: &mut Vec<Target>, libs: &[TomlLibTarget]) {
        let l = &libs[0];
        let path = l.path.clone().unwrap_or_else(|| format!("src/{}.rs", l.name));
        let crate_types = l.crate_type.clone().and_then(|kinds| {
            LibKind::from_strs(kinds).ok()
        }).unwrap_or_else(|| vec!(Lib));

        for profile in target_profiles(l).iter() {
            dst.push(Target::lib_target(l.name.as_slice(), crate_types.clone(),
                                        &Path::new(path.as_slice()), profile));
        }
    }

    fn bin_targets(dst: &mut Vec<Target>, bins: &[TomlBinTarget],
                   default: |&TomlBinTarget| -> String) {
        for bin in bins.iter() {
            let path = bin.path.clone().unwrap_or_else(|| default(bin));

            for profile in target_profiles(bin).iter() {
                dst.push(Target::bin_target(bin.name.as_slice(),
                                            &Path::new(path.as_slice()), profile));
            }
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
