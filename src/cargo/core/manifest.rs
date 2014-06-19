use std::result;
use std::fmt;
use std::fmt::{Show,Formatter};
use semver::Version;
use serialize::{Encoder,Encodable};
use core::source::SourceId;
use core::{
    Dependency,
    PackageId,
    Summary
};
use core::dependency::SerializedDependency;
use util::{CargoResult, human};

#[deriving(PartialEq,Clone)]
pub struct Manifest {
    summary: Summary,
    authors: Vec<String>,
    targets: Vec<Target>,
    target_dir: Path,
    sources: Vec<SourceId>,
    build: Option<String>,
}

impl Show for Manifest {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "Manifest({}, authors={}, targets={}, target_dir={}, \
                   build={})",
               self.summary, self.authors, self.targets,
               self.target_dir.display(), self.build)
    }
}

#[deriving(PartialEq,Clone,Encodable)]
pub struct SerializedManifest {
    name: String,
    version: String,
    dependencies: Vec<SerializedDependency>,
    authors: Vec<String>,
    targets: Vec<Target>,
    target_dir: String,
    build: Option<String>,
}

impl<E, S: Encoder<E>> Encodable<S, E> for Manifest {
    fn encode(&self, s: &mut S) -> Result<(), E> {
        SerializedManifest {
            name: self.summary.get_name().to_str(),
            version: self.summary.get_version().to_str(),
            dependencies: self.summary.get_dependencies().iter().map(|d| {
                SerializedDependency::from_dependency(d)
            }).collect(),
            authors: self.authors.clone(),
            targets: self.targets.clone(),
            target_dir: self.target_dir.display().to_str(),
            build: self.build.clone(),
        }.encode(s)
    }
}

#[deriving(Show,Clone,PartialEq,Encodable)]
pub enum LibKind {
    Lib,
    Rlib,
    Dylib,
    StaticLib
}

impl LibKind {
    pub fn from_str(string: &str) -> CargoResult<LibKind> {
        match string {
            "lib" => Ok(Lib),
            "rlib" => Ok(Rlib),
            "dylib" => Ok(Dylib),
            "staticlib" => Ok(StaticLib),
            _ => Err(human(format!("{} was not one of lib|rlib|dylib|staticlib",
                                   string)))
        }
    }

    pub fn from_strs<S: Str>(strings: Vec<S>) -> CargoResult<Vec<LibKind>> {
        result::collect(strings.iter().map(|s| LibKind::from_str(s.as_slice())))
    }

    pub fn crate_type(&self) -> &'static str {
        match *self {
            Lib => "lib",
            Rlib => "rlib",
            Dylib => "dylib",
            StaticLib => "staticlib"
        }
    }
}

#[deriving(Show,Clone,PartialEq,Encodable)]
pub enum TargetKind {
    LibTarget(Vec<LibKind>),
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
    kind: Vec<&'static str>,
    name: String,
    path: String
}

impl<E, S: Encoder<E>> Encodable<S, E> for Target {
    fn encode(&self, s: &mut S) -> Result<(), E> {
        let kind = match self.kind {
            LibTarget(ref kinds) => kinds.iter().map(|k| k.crate_type()).collect(),
            BinTarget => vec!("bin")
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
        write!(f, "{}(name={}, path={})", self.kind, self.name,
               self.path.display())
    }
}

impl Manifest {
    pub fn new(summary: &Summary, targets: &[Target],
               target_dir: &Path, sources: Vec<SourceId>,
               build: Option<String>) -> Manifest {
        Manifest {
            summary: summary.clone(),
            authors: Vec::new(),
            targets: Vec::from_slice(targets),
            target_dir: target_dir.clone(),
            sources: sources,
            build: build,
        }
    }

    pub fn get_summary<'a>(&'a self) -> &'a Summary {
        &self.summary
    }

    pub fn get_package_id<'a>(&'a self) -> &'a PackageId {
        self.get_summary().get_package_id()
    }

    pub fn get_name<'a>(&'a self) -> &'a str {
        self.get_package_id().get_name()
    }

    pub fn get_version<'a>(&'a self) -> &'a Version {
        self.get_summary().get_package_id().get_version()
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

    pub fn get_source_ids<'a>(&'a self) -> &'a [SourceId] {
        self.sources.as_slice()
    }

    pub fn get_build<'a>(&'a self) -> Option<&'a str> {
        self.build.as_ref().map(|s| s.as_slice())
    }
}

impl Target {
    pub fn lib_target(name: &str, crate_targets: Vec<LibKind>,
                      path: &Path) -> Target {
        Target {
            kind: LibTarget(crate_targets),
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
            LibTarget(_) => true,
            _ => false
        }
    }

    pub fn is_bin(&self) -> bool {
        match self.kind {
            BinTarget => true,
            _ => false
        }
    }

    pub fn rustc_crate_types(&self) -> Vec<&'static str> {
        match self.kind {
            LibTarget(ref kinds) => {
                kinds.iter().map(|kind| kind.crate_type()).collect()
            },
            BinTarget => vec!("bin")
        }
    }
}
