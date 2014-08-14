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
use core::package_id::Metadata;
use core::dependency::SerializedDependency;
use util::{CargoResult, human};

#[deriving(PartialEq,Clone)]
pub struct Manifest {
    summary: Summary,
    authors: Vec<String>,
    targets: Vec<Target>,
    target_dir: Path,
    doc_dir: Path,
    sources: Vec<SourceId>,
    build: Vec<String>,
    unused_keys: Vec<String>,
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
    doc_dir: String,
    build: Option<Vec<String>>,
}

impl<E, S: Encoder<E>> Encodable<S, E> for Manifest {
    fn encode(&self, s: &mut S) -> Result<(), E> {
        SerializedManifest {
            name: self.summary.get_name().to_string(),
            version: self.summary.get_version().to_string(),
            dependencies: self.summary.get_dependencies().iter().map(|d| {
                SerializedDependency::from_dependency(d)
            }).collect(),
            authors: self.authors.clone(),
            targets: self.targets.clone(),
            target_dir: self.target_dir.display().to_string(),
            doc_dir: self.doc_dir.display().to_string(),
            build: if self.build.len() == 0 { None } else { Some(self.build.clone()) },
        }.encode(s)
    }
}

#[deriving(Show, Clone, PartialEq, Hash, Encodable)]
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

#[deriving(Show, Clone, Hash, PartialEq, Encodable)]
pub enum TargetKind {
    LibTarget(Vec<LibKind>),
    BinTarget
}

#[deriving(Encodable, Decodable, Clone, Hash, PartialEq, Show)]
pub struct Profile {
    env: String, // compile, test, dev, bench, etc.
    opt_level: uint,
    debug: bool,
    test: bool,
    doctest: bool,
    doc: bool,
    dest: Option<String>,
    plugin: bool,
}

impl Profile {
    fn default() -> Profile {
        Profile {
            env: String::new(),
            opt_level: 0,
            debug: false,
            test: false,
            doc: false,
            dest: None,
            plugin: false,
            doctest: false,
        }
    }

    pub fn default_dev() -> Profile {
        Profile {
            env: "compile".to_string(), // run in the default environment only
            opt_level: 0,
            debug: true,
            .. Profile::default()
        }
    }

    pub fn default_test() -> Profile {
        Profile {
            env: "test".to_string(),
            debug: true,
            test: true,
            dest: Some("test".to_string()),
            .. Profile::default()
        }
    }

    pub fn default_bench() -> Profile {
        Profile {
            env: "bench".to_string(),
            opt_level: 3,
            test: true,
            dest: Some("bench".to_string()),
            .. Profile::default()
        }
    }

    pub fn default_release() -> Profile {
        Profile {
            env: "release".to_string(),
            opt_level: 3,
            dest: Some("release".to_string()),
            .. Profile::default()
        }
    }

    pub fn default_doc() -> Profile {
        Profile {
            env: "doc".to_string(),
            dest: Some("doc-build".to_string()),
            doc: true,
            .. Profile::default()
        }
    }

    pub fn is_compile(&self) -> bool {
        self.env.as_slice() == "compile"
    }

    pub fn is_doc(&self) -> bool {
        self.doc
    }

    pub fn is_test(&self) -> bool {
        self.test
    }

    pub fn is_doctest(&self) -> bool {
        self.doctest
    }

    pub fn is_plugin(&self) -> bool {
        self.plugin
    }

    pub fn get_opt_level(&self) -> uint {
        self.opt_level
    }

    pub fn get_debug(&self) -> bool {
        self.debug
    }

    pub fn get_env(&self) -> &str {
        self.env.as_slice()
    }

    pub fn get_dest(&self) -> Option<&str> {
        self.dest.as_ref().map(|d| d.as_slice())
    }

    pub fn opt_level(mut self, level: uint) -> Profile {
        self.opt_level = level;
        self
    }

    pub fn debug(mut self, debug: bool) -> Profile {
        self.debug = debug;
        self
    }

    pub fn test(mut self, test: bool) -> Profile {
        self.test = test;
        self
    }

    pub fn doctest(mut self, doctest: bool) -> Profile {
        self.doctest = doctest;
        self
    }

    pub fn doc(mut self, doc: bool) -> Profile {
        self.doc = doc;
        self
    }

    pub fn plugin(mut self, plugin: bool) -> Profile {
        self.plugin = plugin;
        self
    }
}

#[deriving(Clone, Hash, PartialEq)]
pub struct Target {
    kind: TargetKind,
    name: String,
    src_path: Path,
    profile: Profile,
    metadata: Option<Metadata>,
}

#[deriving(Encodable)]
pub struct SerializedTarget {
    kind: Vec<&'static str>,
    name: String,
    src_path: String,
    profile: Profile,
    metadata: Option<Metadata>
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
            src_path: self.src_path.display().to_string(),
            profile: self.profile.clone(),
            metadata: self.metadata.clone()
        }.encode(s)
    }
}

impl Show for Target {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{}(name={}, path={}, profile={})", self.kind, self.name,
               self.src_path.display(), self.profile)
    }
}


impl Manifest {
    pub fn new(summary: &Summary, targets: &[Target],
               target_dir: &Path, doc_dir: &Path, sources: Vec<SourceId>,
               build: Vec<String>) -> Manifest {
        Manifest {
            summary: summary.clone(),
            authors: Vec::new(),
            targets: Vec::from_slice(targets),
            target_dir: target_dir.clone(),
            doc_dir: doc_dir.clone(),
            sources: sources,
            build: build,
            unused_keys: Vec::new(),
        }
    }

    pub fn get_summary(&self) -> &Summary {
        &self.summary
    }

    pub fn get_package_id(&self) -> &PackageId {
        self.get_summary().get_package_id()
    }

    pub fn get_name(&self) -> &str {
        self.get_package_id().get_name()
    }

    pub fn get_version(&self) -> &Version {
        self.get_summary().get_package_id().get_version()
    }

    pub fn get_authors(&self) -> &[String] {
        self.authors.as_slice()
    }

    pub fn get_dependencies(&self) -> &[Dependency] {
        self.get_summary().get_dependencies()
    }

    pub fn get_targets(&self) -> &[Target] {
        self.targets.as_slice()
    }

    pub fn get_target_dir(&self) -> &Path {
        &self.target_dir
    }

    pub fn get_doc_dir(&self) -> &Path {
        &self.doc_dir
    }

    pub fn get_source_ids(&self) -> &[SourceId] {
        self.sources.as_slice()
    }

    pub fn get_build(&self) -> &[String] {
        self.build.as_slice()
    }

    pub fn add_unused_key(&mut self, s: String) {
        self.unused_keys.push(s)
    }

    pub fn get_unused_keys(&self) -> &[String] {
        self.unused_keys.as_slice()
    }
}

impl Target {
    pub fn file_stem(&self) -> String {
        match self.metadata {
            Some(ref metadata) => format!("{}{}", self.name, metadata.extra_filename),
            None => self.name.clone()
        }
    }

    pub fn lib_target(name: &str, crate_targets: Vec<LibKind>,
                      src_path: &Path, profile: &Profile,
                      metadata: Metadata) -> Target {
        Target {
            kind: LibTarget(crate_targets),
            name: name.to_string(),
            src_path: src_path.clone(),
            profile: profile.clone(),
            metadata: Some(metadata)
        }
    }

    pub fn bin_target(name: &str, src_path: &Path, profile: &Profile,
                      metadata: Option<Metadata>) -> Target {
        Target {
            kind: BinTarget,
            name: name.to_string(),
            src_path: src_path.clone(),
            profile: profile.clone(),
            metadata: metadata,
        }
    }

    pub fn example_target(name: &str, src_path: &Path, profile: &Profile) -> Target {
        Target {
            kind: BinTarget,
            name: name.to_string(),
            src_path: src_path.clone(),
            profile: profile.clone(),
            metadata: None,
        }
    }

    pub fn test_target(name: &str, src_path: &Path,
                       profile: &Profile, metadata: Metadata) -> Target {
        Target {
            kind: BinTarget,
            name: name.to_string(),
            src_path: src_path.clone(),
            profile: profile.clone(),
            metadata: Some(metadata),
        }
    }

    pub fn bench_target(name: &str, src_path: &Path,
                        profile: &Profile, metadata: Metadata) -> Target {
        Target {
            kind: BinTarget,
            name: name.to_string(),
            src_path: src_path.clone(),
            profile: profile.clone(),
            metadata: Some(metadata),
        }
    }

    pub fn get_name(&self) -> &str {
        self.name.as_slice()
    }

    pub fn get_src_path(&self) -> &Path {
        &self.src_path
    }

    pub fn is_lib(&self) -> bool {
        match self.kind {
            LibTarget(_) => true,
            _ => false
        }
    }

    pub fn is_dylib(&self) -> bool {
        match self.kind {
            LibTarget(ref kinds) => kinds.iter().any(|&k| k == Dylib),
            _ => false
        }
    }

    pub fn is_rlib(&self) -> bool {
        match self.kind {
            LibTarget(ref kinds) =>
                kinds.iter().any(|&k| k == Rlib || k == Lib),
            _ => false
        }
    }

    pub fn is_staticlib(&self) -> bool {
        match self.kind {
            LibTarget(ref kinds) => kinds.iter().any(|&k| k == StaticLib),
            _ => false
        }
    }

    pub fn is_bin(&self) -> bool {
        match self.kind {
            BinTarget => true,
            _ => false
        }
    }

    pub fn get_profile(&self) -> &Profile {
        &self.profile
    }

    pub fn get_metadata(&self) -> Option<&Metadata> {
        self.metadata.as_ref()
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
