use std::collections::HashMap;
use std::fmt;
use std::path::{PathBuf, Path};

use semver::Version;
use serde::ser;

use core::{Dependency, PackageId, Summary, SourceId, PackageIdSpec};
use core::WorkspaceConfig;

pub enum EitherManifest {
    Real(Manifest),
    Virtual(VirtualManifest),
}

/// Contains all the information about a package, as loaded from a Cargo.toml.
#[derive(Clone, Debug)]
pub struct Manifest {
    summary: Summary,
    targets: Vec<Target>,
    links: Option<String>,
    warnings: Vec<String>,
    exclude: Vec<String>,
    include: Vec<String>,
    metadata: ManifestMetadata,
    profiles: Profiles,
    publish: bool,
    replace: Vec<(PackageIdSpec, Dependency)>,
    workspace: WorkspaceConfig,
}

#[derive(Clone, Debug)]
pub struct VirtualManifest {
    replace: Vec<(PackageIdSpec, Dependency)>,
    workspace: WorkspaceConfig,
    profiles: Profiles,
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
    pub categories: Vec<String>,
    pub license: Option<String>,
    pub license_file: Option<String>,
    pub description: Option<String>,    // not markdown
    pub readme: Option<String>,         // file, not contents
    pub homepage: Option<String>,       // url
    pub repository: Option<String>,     // url
    pub documentation: Option<String>,  // url
    pub badges: HashMap<String, HashMap<String, String>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum LibKind {
    Lib,
    Rlib,
    Dylib,
    ProcMacro,
    Other(String),
}

impl LibKind {
    pub fn from_str(string: &str) -> LibKind {
        match string {
            "lib" => LibKind::Lib,
            "rlib" => LibKind::Rlib,
            "dylib" => LibKind::Dylib,
            "proc-macro" => LibKind::ProcMacro,
            s => LibKind::Other(s.to_string()),
        }
    }

    /// Returns the argument suitable for `--crate-type` to pass to rustc.
    pub fn crate_type(&self) -> &str {
        match *self {
            LibKind::Lib => "lib",
            LibKind::Rlib => "rlib",
            LibKind::Dylib => "dylib",
            LibKind::ProcMacro => "proc-macro",
            LibKind::Other(ref s) => s,
        }
    }

    pub fn linkable(&self) -> bool {
        match *self {
            LibKind::Lib |
            LibKind::Rlib |
            LibKind::Dylib |
            LibKind::ProcMacro => true,
            LibKind::Other(..) => false,
        }
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum TargetKind {
    Lib(Vec<LibKind>),
    Bin,
    Test,
    Bench,
    ExampleLib(Vec<LibKind>),
    ExampleBin,
    CustomBuild,
}

impl ser::Serialize for TargetKind {
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
        where S: ser::Serializer,
    {
        use self::TargetKind::*;
        match *self {
            Lib(ref kinds) => kinds.iter().map(LibKind::crate_type).collect(),
            Bin => vec!["bin"],
            ExampleBin | ExampleLib(_) => vec!["example"],
            Test => vec!["test"],
            CustomBuild => vec!["custom-build"],
            Bench => vec!["bench"]
        }.serialize(s)
    }
}


// Note that most of the fields here are skipped when serializing because we
// don't want to export them just yet (becomes a public API of Cargo). Others
// though are definitely needed!
#[derive(Clone, PartialEq, Eq, Debug, Hash, Serialize)]
pub struct Profile {
    pub opt_level: String,
    #[serde(skip_serializing)]
    pub lto: bool,
    #[serde(skip_serializing)]
    pub codegen_units: Option<u32>,    // None = use rustc default
    #[serde(skip_serializing)]
    pub rustc_args: Option<Vec<String>>,
    #[serde(skip_serializing)]
    pub rustdoc_args: Option<Vec<String>>,
    pub debuginfo: Option<u32>,
    pub debug_assertions: bool,
    pub overflow_checks: bool,
    #[serde(skip_serializing)]
    pub rpath: bool,
    pub test: bool,
    #[serde(skip_serializing)]
    pub doc: bool,
    #[serde(skip_serializing)]
    pub run_custom_build: bool,
    #[serde(skip_serializing)]
    pub check: bool,
    #[serde(skip_serializing)]
    pub panic: Option<String>,
}

#[derive(Default, Clone, Debug, PartialEq, Eq)]
pub struct Profiles {
    pub release: Profile,
    pub dev: Profile,
    pub test: Profile,
    pub test_deps: Profile,
    pub bench: Profile,
    pub bench_deps: Profile,
    pub doc: Profile,
    pub custom_build: Profile,
    pub check: Profile,
    pub doctest: Profile,
}

/// Information about a binary, a library, an example, etc. that is part of the
/// package.
#[derive(Clone, Hash, PartialEq, Eq, Debug)]
pub struct Target {
    kind: TargetKind,
    name: String,
    src_path: PathBuf,
    required_features: Option<Vec<String>>,
    tested: bool,
    benched: bool,
    doc: bool,
    doctest: bool,
    harness: bool, // whether to use the test harness (--test)
    for_host: bool,
}

#[derive(Serialize)]
struct SerializedTarget<'a> {
    /// Is this a `--bin bin`, `--lib`, `--example ex`?
    /// Serialized as a list of strings for historical reasons.
    kind: &'a TargetKind,
    /// Corresponds to `--crate-type` compiler attribute.
    /// See https://doc.rust-lang.org/reference.html#linkage
    crate_types: Vec<&'a str>,
    name: &'a str,
    src_path: &'a PathBuf,
}

impl ser::Serialize for Target {
    fn serialize<S: ser::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        SerializedTarget {
            kind: &self.kind,
            crate_types: self.rustc_crate_types(),
            name: &self.name,
            src_path: &self.src_path,
        }.serialize(s)
    }
}

impl Manifest {
    pub fn new(summary: Summary,
               targets: Vec<Target>,
               exclude: Vec<String>,
               include: Vec<String>,
               links: Option<String>,
               metadata: ManifestMetadata,
               profiles: Profiles,
               publish: bool,
               replace: Vec<(PackageIdSpec, Dependency)>,
               workspace: WorkspaceConfig) -> Manifest {
        Manifest {
            summary: summary,
            targets: targets,
            warnings: Vec::new(),
            exclude: exclude,
            include: include,
            links: links,
            metadata: metadata,
            profiles: profiles,
            publish: publish,
            replace: replace,
            workspace: workspace,
        }
    }

    pub fn dependencies(&self) -> &[Dependency] { self.summary.dependencies() }
    pub fn exclude(&self) -> &[String] { &self.exclude }
    pub fn include(&self) -> &[String] { &self.include }
    pub fn metadata(&self) -> &ManifestMetadata { &self.metadata }
    pub fn name(&self) -> &str { self.package_id().name() }
    pub fn package_id(&self) -> &PackageId { self.summary.package_id() }
    pub fn summary(&self) -> &Summary { &self.summary }
    pub fn targets(&self) -> &[Target] { &self.targets }
    pub fn version(&self) -> &Version { self.package_id().version() }
    pub fn warnings(&self) -> &[String] { &self.warnings }
    pub fn profiles(&self) -> &Profiles { &self.profiles }
    pub fn publish(&self) -> bool { self.publish }
    pub fn replace(&self) -> &[(PackageIdSpec, Dependency)] { &self.replace }
    pub fn links(&self) -> Option<&str> {
        self.links.as_ref().map(|s| &s[..])
    }

    pub fn workspace_config(&self) -> &WorkspaceConfig {
        &self.workspace
    }

    pub fn add_warning(&mut self, s: String) {
        self.warnings.push(s)
    }

    pub fn set_summary(&mut self, summary: Summary) {
        self.summary = summary;
    }

    pub fn map_source(self, to_replace: &SourceId, replace_with: &SourceId)
                      -> Manifest {
        Manifest {
            summary: self.summary.map_source(to_replace, replace_with),
            ..self
        }
    }
}

impl VirtualManifest {
    pub fn new(replace: Vec<(PackageIdSpec, Dependency)>,
               workspace: WorkspaceConfig,
               profiles: Profiles) -> VirtualManifest {
        VirtualManifest {
            replace: replace,
            workspace: workspace,
            profiles: profiles,
        }
    }

    pub fn replace(&self) -> &[(PackageIdSpec, Dependency)] {
        &self.replace
    }

    pub fn workspace_config(&self) -> &WorkspaceConfig {
        &self.workspace
    }

    pub fn profiles(&self) -> &Profiles {
        &self.profiles
    }
}

impl Target {
    fn with_path(src_path: PathBuf) -> Target {
        assert!(src_path.is_absolute());
        Target {
            kind: TargetKind::Bin,
            name: String::new(),
            src_path: src_path,
            required_features: None,
            doc: false,
            doctest: false,
            harness: true,
            for_host: false,
            tested: true,
            benched: true,
        }
    }

    pub fn lib_target(name: &str,
                      crate_targets: Vec<LibKind>,
                      src_path: PathBuf) -> Target {
        Target {
            kind: TargetKind::Lib(crate_targets),
            name: name.to_string(),
            doctest: true,
            doc: true,
            ..Target::with_path(src_path)
        }
    }

    pub fn bin_target(name: &str, src_path: PathBuf,
                      required_features: Option<Vec<String>>) -> Target {
        Target {
            kind: TargetKind::Bin,
            name: name.to_string(),
            required_features: required_features,
            doc: true,
            ..Target::with_path(src_path)
        }
    }

    /// Builds a `Target` corresponding to the `build = "build.rs"` entry.
    pub fn custom_build_target(name: &str, src_path: PathBuf) -> Target {
        Target {
            kind: TargetKind::CustomBuild,
            name: name.to_string(),
            for_host: true,
            benched: false,
            tested: false,
            ..Target::with_path(src_path)
        }
    }

    pub fn example_target(name: &str,
                          crate_targets: Vec<LibKind>,
                          src_path: PathBuf,
                          required_features: Option<Vec<String>>) -> Target {
        let kind = if crate_targets.is_empty() {
            TargetKind::ExampleBin
        } else {
            TargetKind::ExampleLib(crate_targets)
        };

        Target {
            kind: kind,
            name: name.to_string(),
            required_features: required_features,
            benched: false,
            ..Target::with_path(src_path)
        }
    }

    pub fn test_target(name: &str, src_path: PathBuf,
                       required_features: Option<Vec<String>>) -> Target {
        Target {
            kind: TargetKind::Test,
            name: name.to_string(),
            required_features: required_features,
            benched: false,
            ..Target::with_path(src_path)
        }
    }

    pub fn bench_target(name: &str, src_path: PathBuf,
                        required_features: Option<Vec<String>>) -> Target {
        Target {
            kind: TargetKind::Bench,
            name: name.to_string(),
            required_features: required_features,
            tested: false,
            ..Target::with_path(src_path)
        }
    }

    pub fn name(&self) -> &str { &self.name }
    pub fn crate_name(&self) -> String { self.name.replace("-", "_") }
    pub fn src_path(&self) -> &Path { &self.src_path }
    pub fn required_features(&self) -> Option<&Vec<String>> { self.required_features.as_ref() }
    pub fn kind(&self) -> &TargetKind { &self.kind }
    pub fn tested(&self) -> bool { self.tested }
    pub fn harness(&self) -> bool { self.harness }
    pub fn documented(&self) -> bool { self.doc }
    pub fn for_host(&self) -> bool { self.for_host }
    pub fn benched(&self) -> bool { self.benched }

    pub fn doctested(&self) -> bool {
        self.doctest && match self.kind {
            TargetKind::Lib(ref kinds) => {
                kinds.iter().find(|k| {
                  *k == &LibKind::Rlib ||
                  *k == &LibKind::Lib ||
                  *k == &LibKind::ProcMacro
                }).is_some()
            }
            _ => false,
        }
    }

    pub fn allows_underscores(&self) -> bool {
        self.is_bin() || self.is_example() || self.is_custom_build()
    }

    pub fn is_lib(&self) -> bool {
        match self.kind {
            TargetKind::Lib(_) => true,
            _ => false
        }
    }

    pub fn is_dylib(&self) -> bool {
        match self.kind {
            TargetKind::Lib(ref libs) => libs.iter().any(|l| *l == LibKind::Dylib),
            _ => false
        }
    }

    pub fn linkable(&self) -> bool {
        match self.kind {
            TargetKind::Lib(ref kinds) => {
                kinds.iter().any(|k| k.linkable())
            }
            _ => false
        }
    }

    pub fn is_bin(&self) -> bool { self.kind == TargetKind::Bin }

    pub fn is_example(&self) -> bool {
        match self.kind {
            TargetKind::ExampleBin |
            TargetKind::ExampleLib(..) => true,
            _ => false
        }
    }

    pub fn is_test(&self) -> bool { self.kind == TargetKind::Test }
    pub fn is_bench(&self) -> bool { self.kind == TargetKind::Bench }
    pub fn is_custom_build(&self) -> bool { self.kind == TargetKind::CustomBuild }

    /// Returns the arguments suitable for `--crate-type` to pass to rustc.
    pub fn rustc_crate_types(&self) -> Vec<&str> {
        match self.kind {
            TargetKind::Lib(ref kinds) |
            TargetKind::ExampleLib(ref kinds) => {
                kinds.iter().map(LibKind::crate_type).collect()
            }
            TargetKind::CustomBuild |
            TargetKind::Bench |
            TargetKind::Test |
            TargetKind::ExampleBin |
            TargetKind::Bin => vec!["bin"],
        }
    }

    pub fn can_lto(&self) -> bool {
        match self.kind {
            TargetKind::Lib(ref v) => {
                !v.contains(&LibKind::Rlib) &&
                    !v.contains(&LibKind::Dylib) &&
                    !v.contains(&LibKind::Lib)
            }
            _ => true,
        }
    }

    pub fn set_tested(&mut self, tested: bool) -> &mut Target {
        self.tested = tested;
        self
    }
    pub fn set_benched(&mut self, benched: bool) -> &mut Target {
        self.benched = benched;
        self
    }
    pub fn set_doctest(&mut self, doctest: bool) -> &mut Target {
        self.doctest = doctest;
        self
    }
    pub fn set_for_host(&mut self, for_host: bool) -> &mut Target {
        self.for_host = for_host;
        self
    }
    pub fn set_harness(&mut self, harness: bool) -> &mut Target {
        self.harness = harness;
        self
    }
    pub fn set_doc(&mut self, doc: bool) -> &mut Target {
        self.doc = doc;
        self
    }
}

impl fmt::Display for Target {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.kind {
            TargetKind::Lib(..) => write!(f, "Target(lib)"),
            TargetKind::Bin => write!(f, "Target(bin: {})", self.name),
            TargetKind::Test => write!(f, "Target(test: {})", self.name),
            TargetKind::Bench => write!(f, "Target(bench: {})", self.name),
            TargetKind::ExampleBin |
            TargetKind::ExampleLib(..) => write!(f, "Target(example: {})", self.name),
            TargetKind::CustomBuild => write!(f, "Target(script)"),
        }
    }
}

impl Profile {
    pub fn default_dev() -> Profile {
        Profile {
            debuginfo: Some(2),
            debug_assertions: true,
            overflow_checks: true,
            ..Profile::default()
        }
    }

    pub fn default_release() -> Profile {
        Profile {
            opt_level: "3".to_string(),
            debuginfo: None,
            ..Profile::default()
        }
    }

    pub fn default_test() -> Profile {
        Profile {
            test: true,
            ..Profile::default_dev()
        }
    }

    pub fn default_bench() -> Profile {
        Profile {
            test: true,
            ..Profile::default_release()
        }
    }

    pub fn default_doc() -> Profile {
        Profile {
            doc: true,
            ..Profile::default_dev()
        }
    }

    pub fn default_custom_build() -> Profile {
        Profile {
            run_custom_build: true,
            ..Profile::default_dev()
        }
    }

    pub fn default_check() -> Profile {
        Profile {
            check: true,
            ..Profile::default_dev()
        }
    }

    pub fn default_doctest() -> Profile {
        Profile {
            doc: true,
            test: true,
            ..Profile::default_dev()
        }
    }
}

impl Default for Profile {
    fn default() -> Profile {
        Profile {
            opt_level: "0".to_string(),
            lto: false,
            codegen_units: None,
            rustc_args: None,
            rustdoc_args: None,
            debuginfo: None,
            debug_assertions: false,
            overflow_checks: false,
            rpath: false,
            test: false,
            doc: false,
            run_custom_build: false,
            check: false,
            panic: None,
        }
    }
}

impl fmt::Display for Profile {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.test {
            write!(f, "Profile(test)")
        } else if self.doc {
            write!(f, "Profile(doc)")
        } else if self.run_custom_build {
            write!(f, "Profile(run)")
        } else if self.check {
            write!(f, "Profile(check)")
        } else {
            write!(f, "Profile(build)")
        }

    }
}
