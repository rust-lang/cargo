use std::hash;
use std::path::{PathBuf, Path};

use semver::Version;
use rustc_serialize::{Encoder,Encodable};

use core::{Dependency, PackageId, Summary};
use core::package_id::Metadata;
use core::dependency::SerializedDependency;
use util::{CargoResult, human};

/// Contains all the informations about a package, as loaded from a Cargo.toml.
#[derive(PartialEq,Clone, Debug)]
pub struct Manifest {
    summary: Summary,
    targets: Vec<Target>,
    target_dir: PathBuf,
    doc_dir: PathBuf,
    links: Option<String>,
    warnings: Vec<String>,
    exclude: Vec<String>,
    include: Vec<String>,
    metadata: ManifestMetadata,
}

/// General metadata about a package which is just blindly uploaded to the
/// registry.
///
/// Note that many of these fields can contain invalid values such as the
/// homepage, repository, documentation, or license. These fields are not
/// validated by cargo itself, but rather it is up to the registry when uploaded
/// to validate these fields. Cargo will itself accept any valid TOML
/// specification for these values.
#[derive(PartialEq, Clone, Debug)]
pub struct ManifestMetadata {
    pub authors: Vec<String>,
    pub keywords: Vec<String>,
    pub license: Option<String>,
    pub license_file: Option<String>,
    pub description: Option<String>,    // not markdown
    pub readme: Option<String>,         // file, not contents
    pub homepage: Option<String>,       // url
    pub repository: Option<String>,     // url
    pub documentation: Option<String>,  // url
}

#[derive(PartialEq,Clone,RustcEncodable)]
pub struct SerializedManifest {
    name: String,
    version: String,
    dependencies: Vec<SerializedDependency>,
    targets: Vec<Target>,
    target_dir: String,
    doc_dir: String,
}

impl Encodable for Manifest {
    fn encode<S: Encoder>(&self, s: &mut S) -> Result<(), S::Error> {
        SerializedManifest {
            name: self.summary.name().to_string(),
            version: self.summary.version().to_string(),
            dependencies: self.summary.dependencies().iter().map(|d| {
                SerializedDependency::from_dependency(d)
            }).collect(),
            targets: self.targets.clone(),
            target_dir: self.target_dir.display().to_string(),
            doc_dir: self.doc_dir.display().to_string(),
        }.encode(s)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, RustcEncodable, Copy)]
pub enum LibKind {
    Lib,
    Rlib,
    Dylib,
    StaticLib
}

impl LibKind {
    pub fn from_str(string: &str) -> CargoResult<LibKind> {
        match string {
            "lib" => Ok(LibKind::Lib),
            "rlib" => Ok(LibKind::Rlib),
            "dylib" => Ok(LibKind::Dylib),
            "staticlib" => Ok(LibKind::StaticLib),
            _ => Err(human(format!("{} was not one of lib|rlib|dylib|staticlib",
                                   string)))
        }
    }

    /// Returns the argument suitable for `--crate-type` to pass to rustc.
    pub fn crate_type(&self) -> &'static str {
        match *self {
            LibKind::Lib => "lib",
            LibKind::Rlib => "rlib",
            LibKind::Dylib => "dylib",
            LibKind::StaticLib => "staticlib"
        }
    }
}

#[derive(Debug, Clone, Hash, PartialEq, RustcEncodable, Eq)]
pub enum TargetKind {
    Lib(Vec<LibKind>),
    Bin,
    Example,
}

#[derive(RustcEncodable, RustcDecodable, Clone, PartialEq, Eq, Debug)]
pub struct Profile {
    env: String, // compile, test, dev, bench, etc.
    opt_level: u32,
    lto: bool,
    codegen_units: Option<u32>,    // None = use rustc default
    debug: bool,
    rpath: bool,
    test: bool,
    doctest: bool,
    doc: bool,
    dest: Option<String>,
    for_host: bool,
    harness: bool, // whether to use the test harness (--test)
    custom_build: bool,
}

impl Profile {
    fn default() -> Profile {
        Profile {
            env: String::new(),
            opt_level: 0,
            lto: false,
            codegen_units: None,
            debug: false,
            rpath: false,
            test: false,
            doc: false,
            dest: None,
            for_host: false,
            doctest: false,
            custom_build: false,
            harness: true,
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
            dest: None,
            .. Profile::default()
        }
    }

    pub fn default_example() -> Profile {
        Profile {
            test: false,
            .. Profile::default_test()
        }
    }

    pub fn default_bench() -> Profile {
        Profile {
            env: "bench".to_string(),
            opt_level: 3,
            test: true,
            dest: Some("release".to_string()),
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
            dest: None,
            doc: true,
            .. Profile::default()
        }
    }

    pub fn codegen_units(&self) -> Option<u32> { self.codegen_units }
    pub fn debug(&self) -> bool { self.debug }
    pub fn env(&self) -> &str { &self.env }
    pub fn is_compile(&self) -> bool { self.env == "compile" }
    pub fn is_custom_build(&self) -> bool { self.custom_build }
    pub fn is_doc(&self) -> bool { self.doc }
    pub fn is_doctest(&self) -> bool { self.doctest }
    pub fn is_for_host(&self) -> bool { self.for_host }
    pub fn is_test(&self) -> bool { self.test }
    pub fn lto(&self) -> bool { self.lto }
    pub fn opt_level(&self) -> u32 { self.opt_level }
    pub fn rpath(&self) -> bool { self.rpath }
    pub fn uses_test_harness(&self) -> bool { self.harness }

    pub fn dest(&self) -> Option<&str> {
        self.dest.as_ref().map(|d| d.as_slice())
    }

    pub fn set_opt_level(mut self, level: u32) -> Profile {
        self.opt_level = level;
        self
    }

    pub fn set_lto(mut self, lto: bool) -> Profile {
        self.lto = lto;
        self
    }

    pub fn set_codegen_units(mut self, units: Option<u32>) -> Profile {
        self.codegen_units = units;
        self
    }

    pub fn set_debug(mut self, debug: bool) -> Profile {
        self.debug = debug;
        self
    }

    pub fn set_rpath(mut self, rpath: bool) -> Profile {
        self.rpath = rpath;
        self
    }

    pub fn set_test(mut self, test: bool) -> Profile {
        self.test = test;
        self
    }

    pub fn set_doctest(mut self, doctest: bool) -> Profile {
        self.doctest = doctest;
        self
    }

    pub fn set_doc(mut self, doc: bool) -> Profile {
        self.doc = doc;
        self
    }

    /// Sets whether the `Target` must be compiled for the host instead of the
    /// target platform.
    pub fn set_for_host(mut self, for_host: bool) -> Profile {
        self.for_host = for_host;
        self
    }

    pub fn set_harness(mut self, harness: bool) -> Profile {
        self.harness = harness;
        self
    }

    /// Sets whether the `Target` is a custom build script.
    pub fn set_custom_build(mut self, custom_build: bool) -> Profile {
        self.custom_build = custom_build;
        self
    }
}

impl hash::Hash for Profile {
    fn hash<H: hash::Hasher>(&self, into: &mut H) {
        // Be sure to match all fields explicitly, but ignore those not relevant
        // to the actual hash of a profile.
        let Profile {
            opt_level,
            lto,
            codegen_units,
            debug,
            rpath,
            for_host,
            ref dest,
            harness,

            // test flags are separated by file, not by profile hash, and
            // env/doc also don't matter for the actual contents of the output
            // file, just where the output file is located.
            doc: _,
            env: _,
            test: _,
            doctest: _,

            custom_build: _,
        } = *self;
        (opt_level, lto, codegen_units, debug,
         rpath, for_host, dest, harness).hash(into)
    }
}

/// Informations about a binary, a library, an example, etc. that is part of the
/// package.
#[derive(Clone, Hash, PartialEq, Eq, Debug)]
pub struct Target {
    kind: TargetKind,
    name: String,
    src_path: PathBuf,
    profile: Profile,
    metadata: Option<Metadata>,
}

#[derive(RustcEncodable)]
pub struct SerializedTarget {
    kind: Vec<&'static str>,
    name: String,
    src_path: String,
    profile: Profile,
    metadata: Option<Metadata>
}

impl Encodable for Target {
    fn encode<S: Encoder>(&self, s: &mut S) -> Result<(), S::Error> {
        let kind = match self.kind {
            TargetKind::Lib(ref kinds) => {
                kinds.iter().map(|k| k.crate_type()).collect()
            }
            TargetKind::Bin => vec!("bin"),
            TargetKind::Example => vec!["example"],
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

impl Manifest {
    pub fn new(summary: Summary, targets: Vec<Target>,
               target_dir: PathBuf, doc_dir: PathBuf,
               exclude: Vec<String>,
               include: Vec<String>,
               links: Option<String>,
               metadata: ManifestMetadata) -> Manifest {
        Manifest {
            summary: summary,
            targets: targets,
            target_dir: target_dir,
            doc_dir: doc_dir,
            warnings: Vec::new(),
            exclude: exclude,
            include: include,
            links: links,
            metadata: metadata,
        }
    }

    pub fn dependencies(&self) -> &[Dependency] { self.summary.dependencies() }
    pub fn doc_dir(&self) -> &Path { &self.doc_dir }
    pub fn exclude(&self) -> &[String] { &self.exclude }
    pub fn include(&self) -> &[String] { &self.include }
    pub fn metadata(&self) -> &ManifestMetadata { &self.metadata }
    pub fn name(&self) -> &str { self.package_id().name() }
    pub fn package_id(&self) -> &PackageId { self.summary.package_id() }
    pub fn summary(&self) -> &Summary { &self.summary }
    pub fn target_dir(&self) -> &Path { &self.target_dir }
    pub fn targets(&self) -> &[Target] { &self.targets }
    pub fn version(&self) -> &Version { self.package_id().version() }
    pub fn warnings(&self) -> &[String] { &self.warnings }
    pub fn links(&self) -> Option<&str> {
        self.links.as_ref().map(|s| s.as_slice())
    }

    pub fn add_warning(&mut self, s: String) {
        self.warnings.push(s)
    }

    pub fn set_summary(&mut self, summary: Summary) {
        self.summary = summary;
    }

    pub fn set_target_dir(&mut self, target_dir: PathBuf) {
        self.target_dir = target_dir;
    }
}

impl Target {
    pub fn file_stem(&self) -> String {
        match self.metadata {
            Some(ref metadata) => format!("{}{}", self.name,
                                          metadata.extra_filename),
            None => self.name.clone()
        }
    }

    pub fn lib_target(name: &str, crate_targets: Vec<LibKind>,
                      src_path: &Path, profile: &Profile,
                      metadata: Metadata) -> Target {
        Target {
            kind: TargetKind::Lib(crate_targets),
            name: name.to_string(),
            src_path: src_path.to_path_buf(),
            profile: profile.clone(),
            metadata: Some(metadata)
        }
    }

    pub fn bin_target(name: &str, src_path: &Path, profile: &Profile,
                      metadata: Option<Metadata>) -> Target {
        Target {
            kind: TargetKind::Bin,
            name: name.to_string(),
            src_path: src_path.to_path_buf(),
            profile: profile.clone(),
            metadata: metadata,
        }
    }

    /// Builds a `Target` corresponding to the `build = "build.rs"` entry.
    pub fn custom_build_target(name: &str, src_path: &Path, profile: &Profile,
                               metadata: Option<Metadata>) -> Target {
        Target {
            kind: TargetKind::Bin,
            name: name.to_string(),
            src_path: src_path.to_path_buf(),
            profile: profile.clone(),
            metadata: metadata,
        }
    }

    pub fn example_target(name: &str, src_path: &Path, profile: &Profile) -> Target {
        Target {
            kind: TargetKind::Example,
            name: name.to_string(),
            src_path: src_path.to_path_buf(),
            profile: profile.clone(),
            metadata: None,
        }
    }

    pub fn test_target(name: &str, src_path: &Path,
                       profile: &Profile, metadata: Metadata) -> Target {
        Target {
            kind: TargetKind::Bin,
            name: name.to_string(),
            src_path: src_path.to_path_buf(),
            profile: profile.clone(),
            metadata: Some(metadata),
        }
    }

    pub fn bench_target(name: &str, src_path: &Path,
                        profile: &Profile, metadata: Metadata) -> Target {
        Target {
            kind: TargetKind::Bin,
            name: name.to_string(),
            src_path: src_path.to_path_buf(),
            profile: profile.clone(),
            metadata: Some(metadata),
        }
    }

    pub fn name(&self) -> &str { &self.name }
    pub fn src_path(&self) -> &Path { &self.src_path }
    pub fn profile(&self) -> &Profile { &self.profile }
    pub fn metadata(&self) -> Option<&Metadata> { self.metadata.as_ref() }

    pub fn is_lib(&self) -> bool {
        match self.kind {
            TargetKind::Lib(_) => true,
            _ => false
        }
    }

    pub fn is_dylib(&self) -> bool {
        match self.kind {
            TargetKind::Lib(ref kinds) => kinds.iter().any(|&k| k == LibKind::Dylib),
            _ => false
        }
    }

    pub fn is_rlib(&self) -> bool {
        match self.kind {
            TargetKind::Lib(ref kinds) =>
                kinds.iter().any(|&k| k == LibKind::Rlib || k == LibKind::Lib),
            _ => false
        }
    }

    pub fn is_staticlib(&self) -> bool {
        match self.kind {
            TargetKind::Lib(ref kinds) => kinds.iter().any(|&k| k == LibKind::StaticLib),
            _ => false
        }
    }

    /// Returns true for binary, bench, and tests.
    pub fn is_bin(&self) -> bool {
        match self.kind {
            TargetKind::Bin => true,
            _ => false
        }
    }

    /// Returns true for exampels
    pub fn is_example(&self) -> bool {
        match self.kind {
            TargetKind::Example => true,
            _ => false
        }
    }

    /// Returns the arguments suitable for `--crate-type` to pass to rustc.
    pub fn rustc_crate_types(&self) -> Vec<&'static str> {
        match self.kind {
            TargetKind::Lib(ref kinds) => {
                kinds.iter().map(|kind| kind.crate_type()).collect()
            },
            TargetKind::Example |
            TargetKind::Bin => vec!("bin"),
        }
    }
}
