use std::fmt;
use std::fmt::{Show,Formatter};
use std::collections::HashMap;
use semver::Version;
use serialize::{Encoder,Encodable};
use core::{
    Dependency,
    NameVer,
    Package,
    Summary
};
use core::dependency::SerializedDependency;
use util::CargoResult;

#[deriving(PartialEq,Clone)]
pub struct Manifest {
    summary: Summary,
    authors: Vec<String>,
    targets: Vec<Target>,
    target_dir: Path,
}

impl Show for Manifest {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "Manifest({}, authors={}, targets={}, target_dir={})", self.summary, self.authors, self.targets, self.target_dir.display())
    }
}

#[deriving(PartialEq,Clone,Encodable)]
pub struct SerializedManifest {
    name: String,
    version: String,
    dependencies: Vec<SerializedDependency>,
    authors: Vec<String>,
    targets: Vec<Target>,
    target_dir: String
}

impl<E, S: Encoder<E>> Encodable<S, E> for Manifest {
    fn encode(&self, s: &mut S) -> Result<(), E> {
        SerializedManifest {
            name: self.summary.get_name().to_str(),
            version: self.summary.get_version().to_str(),
            dependencies: self.summary.get_dependencies().iter().map(|d| SerializedDependency::from_dependency(d)).collect(),
            authors: self.authors.clone(),
            targets: self.targets.clone(),
            target_dir: self.target_dir.display().to_str()
        }.encode(s)
    }
}

#[deriving(Show,Clone,PartialEq,Encodable)]
pub enum TargetKind {
    LibTarget,
    BinTarget
}

#[deriving(Clone,PartialEq)]
pub struct Target {
    kind: TargetKind,
    name: String,
    path: Path
}

#[deriving(Encodable)]
pub struct SerializedTarget {
    kind: &'static str,
    name: String,
    path: String
}

impl<E, S: Encoder<E>> Encodable<S, E> for Target {
    fn encode(&self, s: &mut S) -> Result<(), E> {
        let kind = match self.kind {
            LibTarget => "lib",
            BinTarget => "bin"
        };

        SerializedTarget {
            kind: kind,
            name: self.name.clone(),
            path: self.path.display().to_str()
        }.encode(s)
    }
}

impl Show for Target {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{}(name={}, path={})", self.kind, self.name, self.path.display())
    }
}

impl Manifest {
    pub fn new(summary: &Summary, targets: &[Target], target_dir: &Path) -> Manifest {
        Manifest {
            summary: summary.clone(),
            authors: Vec::new(),
            targets: Vec::from_slice(targets),
            target_dir: target_dir.clone()
        }
    }

    pub fn get_summary<'a>(&'a self) -> &'a Summary {
        &self.summary
    }

    pub fn get_name<'a>(&'a self) -> &'a str {
        self.get_summary().get_name_ver().get_name()
    }

    pub fn get_version<'a>(&'a self) -> &'a Version {
        self.get_summary().get_name_ver().get_version()
    }

    pub fn get_authors<'a>(&'a self) -> &'a [String] {
        self.authors.as_slice()
    }

    pub fn get_dependencies<'a>(&'a self) -> &'a [Dependency] {
        self.get_summary().get_dependencies()
    }

    pub fn get_targets<'a>(&'a self) -> &'a [Target] {
        self.targets.as_slice()
    }

    pub fn get_target_dir<'a>(&'a self) -> &'a Path {
        &self.target_dir
    }
}

impl Target {
    pub fn lib_target(name: &str, path: &Path) -> Target {
        Target {
            kind: LibTarget,
            name: name.to_str(),
            path: path.clone()
        }
    }

    pub fn bin_target(name: &str, path: &Path) -> Target {
        Target {
            kind: BinTarget,
            name: name.to_str(),
            path: path.clone()
        }
    }

    pub fn get_path<'a>(&'a self) -> &'a Path {
        &self.path
    }

    pub fn is_lib(&self) -> bool {
        match self.kind {
            LibTarget => true,
            _ => false
        }
    }

    pub fn is_bin(&self) -> bool {
        match self.kind {
            BinTarget => true,
            _ => false
        }
    }

    pub fn rustc_crate_type(&self) -> &'static str {
        match self.kind {
            LibTarget => "lib",
            BinTarget => "bin"
        }
    }
}

/*
 *
 * ===== Serialized =====
 *
 */

type TomlLibTarget = TomlTarget;
type TomlBinTarget = TomlTarget;

#[deriving(Decodable,Encodable,PartialEq,Clone,Show)]
pub struct Project {
    pub name: String,
    pub version: String,
    pub authors: Vec<String>
}

/*
 * TODO: Make all struct fields private
 */

#[deriving(Decodable,Encodable,PartialEq,Clone)]
pub struct TomlManifest {
    project: Box<Project>,
    lib: Option<~[TomlLibTarget]>,
    bin: Option<~[TomlBinTarget]>,
    dependencies: Option<HashMap<String, String>>,
}

impl TomlManifest {
    pub fn to_package(&self, path: &str) -> CargoResult<Package> {
        // TODO: Convert hte argument to take a Path
        let path = Path::new(path);

        // Get targets
        let targets = normalize(&self.lib, &self.bin);

        if targets.is_empty() {
            debug!("manifest has no build targets; project={}", self.project);
        }

        let mut deps = Vec::new();

        // Collect the deps
        match self.dependencies {
            Some(ref dependencies) => {
                for (n, v) in dependencies.iter() {
                    deps.push(try!(Dependency::parse(n.as_slice(), v.as_slice())));
                }
            }
            None => ()
        }

        // TODO: https://github.com/mozilla/rust/issues/14049
        let root = Path::new(path.dirname());

        Ok(Package::new(
            &Manifest::new(
                &Summary::new(&self.project.to_name_ver(), deps.as_slice()),
                targets.as_slice(),
                &Path::new("target")),
            &root))
    }
}

impl Project {
    fn to_name_ver(&self) -> NameVer {
        NameVer::new(self.name.as_slice(), self.version.as_slice())
    }
}

#[deriving(Decodable,Encodable,PartialEq,Clone,Show)]
struct TomlTarget {
    name: String,
    path: Option<String>
}

fn normalize(lib: &Option<~[TomlLibTarget]>, bin: &Option<~[TomlBinTarget]>) -> Vec<Target> {
    log!(4, "normalizing toml targets; lib={}; bin={}", lib, bin);

    fn lib_targets(dst: &mut Vec<Target>, libs: &[TomlLibTarget]) {
        let l = &libs[0];
        let path = l.path.clone().unwrap_or_else(|| format!("src/{}.rs", l.name));
        dst.push(Target::lib_target(l.name.as_slice(), &Path::new(path)));
    }

    fn bin_targets(dst: &mut Vec<Target>, bins: &[TomlBinTarget], default: |&TomlBinTarget| -> String) {
        for bin in bins.iter() {
            let path = bin.path.clone().unwrap_or_else(|| default(bin));
            dst.push(Target::bin_target(bin.name.as_slice(), &Path::new(path)));
        }
    }

    let mut ret = Vec::new();

    match (lib, bin) {
        (&Some(ref libs), &Some(ref bins)) => {
            lib_targets(&mut ret, libs.as_slice());
            bin_targets(&mut ret, bins.as_slice(), |bin| format!("src/bin/{}.rs", bin.name));
        },
        (&Some(ref libs), &None) => {
            lib_targets(&mut ret, libs.as_slice());
        },
        (&None, &Some(ref bins)) => {
            bin_targets(&mut ret, bins.as_slice(), |bin| format!("src/{}.rs", bin.name));
        },
        (&None, &None) => ()
    }

    ret
}
