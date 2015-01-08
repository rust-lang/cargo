use std::hash;
use std::fmt::{self, Show, Formatter};

use semver::Version;
use rustc_serialize::{Encoder,Encodable};

use core::{Dependency, PackageId, Summary};
use core::package_id::Metadata;
use core::dependency::SerializedDependency;
use util::{CargoResult, human};

/// Contains all the informations about a package, as loaded from a Cargo.toml.
#[derive(PartialEq,Clone)]
pub struct Manifest {
    summary: Summary,
    targets: Vec<Target>,
    target_dir: Path,
    doc_dir: Path,
    build: Vec<String>,         // TODO: deprecated, remove
    links: Option<String>,
    warnings: Vec<String>,
    exclude: Vec<String>,
    metadata: ManifestMetadata,
}

impl Show for Manifest {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "Manifest({}, targets={}, target_dir={}, \
                   build={})",
               self.summary, self.targets, self.target_dir.display(),
               self.build)
    }
}

/// General metadata about a package which is just blindly uploaded to the
/// registry.
///
/// Note that many of these fields can contain invalid values such as the
/// homepage, repository, documentation, or license. These fields are not
/// validated by cargo itself, but rather it is up to the registry when uploaded
/// to validate these fields. Cargo will itself accept any valid TOML
/// specification for these values.
#[derive(PartialEq, Clone)]
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
    build: Option<Vec<String>>,     // TODO: deprecated, remove
}

impl<E, S: Encoder<E>> Encodable<S, E> for Manifest {
    fn encode(&self, s: &mut S) -> Result<(), E> {
        SerializedManifest {
            name: self.summary.get_name().to_string(),
            version: self.summary.get_version().to_string(),
            dependencies: self.summary.get_dependencies().iter().map(|d| {
                SerializedDependency::from_dependency(d)
            }).collect(),
            targets: self.targets.clone(),
            target_dir: self.target_dir.display().to_string(),
            doc_dir: self.doc_dir.display().to_string(),
            // TODO: deprecated, remove
            build: if self.build.len() == 0 { None } else { Some(self.build.clone()) },
        }.encode(s)
    }
}

#[derive(Show, Clone, PartialEq, Hash, RustcEncodable, Copy)]
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

    pub fn from_strs<S: Str>(strings: Vec<S>) -> CargoResult<Vec<LibKind>> {
        strings.iter().map(|s| LibKind::from_str(s.as_slice())).collect()
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

#[derive(Show, Clone, Hash, PartialEq, RustcEncodable)]
pub enum TargetKind {
    Lib(Vec<LibKind>),
    Bin,
    Example,
}

#[derive(RustcEncodable, RustcDecodable, Clone, PartialEq, Show)]
pub struct Profile {
    env: String, // compile, test, dev, bench, etc.
    opt_level: uint,
    lto: bool,
    codegen_units: Option<uint>,    // None = use rustc default
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

    pub fn default_example_release() -> Profile {
        Profile {
            test: false,
            .. Profile::default_bench()
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

    pub fn is_compile(&self) -> bool {
        self.env.as_slice() == "compile"
    }

    pub fn is_doc(&self) -> bool {
        self.doc
    }

    pub fn is_test(&self) -> bool {
        self.test
    }

    pub fn uses_test_harness(&self) -> bool {
        self.harness
    }

    pub fn is_doctest(&self) -> bool {
        self.doctest
    }

    pub fn is_custom_build(&self) -> bool {
        self.custom_build
    }

    /// Returns true if the target must be built for the host instead of the target.
    pub fn is_for_host(&self) -> bool {
        self.for_host
    }

    pub fn get_opt_level(&self) -> uint {
        self.opt_level
    }

    pub fn get_lto(&self) -> bool {
        self.lto
    }

    pub fn get_codegen_units(&self) -> Option<uint> {
        self.codegen_units
    }

    pub fn get_debug(&self) -> bool {
        self.debug
    }

    pub fn get_rpath(&self) -> bool {
        self.rpath
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

    pub fn lto(mut self, lto: bool) -> Profile {
        self.lto = lto;
        self
    }

    pub fn codegen_units(mut self, units: Option<uint>) -> Profile {
        self.codegen_units = units;
        self
    }

    pub fn debug(mut self, debug: bool) -> Profile {
        self.debug = debug;
        self
    }

    pub fn rpath(mut self, rpath: bool) -> Profile {
        self.rpath = rpath;
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

    /// Sets whether the `Target` must be compiled for the host instead of the target platform.
    pub fn for_host(mut self, for_host: bool) -> Profile {
        self.for_host = for_host;
        self
    }

    pub fn harness(mut self, harness: bool) -> Profile {
        self.harness = harness;
        self
    }

    /// Sets whether the `Target` is a custom build script.
    pub fn custom_build(mut self, custom_build: bool) -> Profile {
        self.custom_build = custom_build;
        self
    }
}

impl<H: hash::Writer> hash::Hash<H> for Profile {
    fn hash(&self, into: &mut H) {
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

/// Informations about a binary, a library, an example, etc. that is part of the package.
#[derive(Clone, Hash, PartialEq)]
pub struct Target {
    kind: TargetKind,
    name: String,
    src_path: Path,
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

impl<E, S: Encoder<E>> Encodable<S, E> for Target {
    fn encode(&self, s: &mut S) -> Result<(), E> {
        let kind = match self.kind {
            TargetKind::Lib(ref kinds) => kinds.iter().map(|k| k.crate_type()).collect(),
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

impl Show for Target {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{}(name={}, path={}, profile={})", self.kind, self.name,
               self.src_path.display(), self.profile)
    }
}


impl Manifest {
    pub fn new(summary: Summary, targets: Vec<Target>,
               target_dir: Path, doc_dir: Path,
               build: Vec<String>, exclude: Vec<String>, links: Option<String>,
               metadata: ManifestMetadata) -> Manifest {
        Manifest {
            summary: summary,
            targets: targets,
            target_dir: target_dir,
            doc_dir: doc_dir,
            build: build,     // TODO: deprecated, remove
            warnings: Vec::new(),
            exclude: exclude,
            links: links,
            metadata: metadata,
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

    pub fn get_build(&self) -> &[String] {
        self.build.as_slice()
    }

    pub fn get_links(&self) -> Option<&str> {
        self.links.as_ref().map(|s| s.as_slice())
    }

    pub fn add_warning(&mut self, s: String) {
        self.warnings.push(s)
    }

    pub fn get_warnings(&self) -> &[String] {
        self.warnings.as_slice()
    }

    pub fn get_exclude(&self) -> &[String] {
        self.exclude.as_slice()
    }

    pub fn get_metadata(&self) -> &ManifestMetadata { &self.metadata }

    pub fn set_summary(&mut self, summary: Summary) {
        self.summary = summary;
    }

    pub fn set_target_dir(&mut self, target_dir: Path) {
        self.target_dir = target_dir;
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
            kind: TargetKind::Lib(crate_targets),
            name: name.to_string(),
            src_path: src_path.clone(),
            profile: profile.clone(),
            metadata: Some(metadata)
        }
    }

    pub fn bin_target(name: &str, src_path: &Path, profile: &Profile,
                      metadata: Option<Metadata>) -> Target {
        Target {
            kind: TargetKind::Bin,
            name: name.to_string(),
            src_path: src_path.clone(),
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
            src_path: src_path.clone(),
            profile: profile.clone(),
            metadata: metadata,
        }
    }

    pub fn example_target(name: &str, src_path: &Path, profile: &Profile) -> Target {
        Target {
            kind: TargetKind::Example,
            name: name.to_string(),
            src_path: src_path.clone(),
            profile: profile.clone(),
            metadata: None,
        }
    }

    pub fn test_target(name: &str, src_path: &Path,
                       profile: &Profile, metadata: Metadata) -> Target {
        Target {
            kind: TargetKind::Bin,
            name: name.to_string(),
            src_path: src_path.clone(),
            profile: profile.clone(),
            metadata: Some(metadata),
        }
    }

    pub fn bench_target(name: &str, src_path: &Path,
                        profile: &Profile, metadata: Metadata) -> Target {
        Target {
            kind: TargetKind::Bin,
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

    pub fn get_profile(&self) -> &Profile {
        &self.profile
    }

    pub fn get_metadata(&self) -> Option<&Metadata> {
        self.metadata.as_ref()
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
