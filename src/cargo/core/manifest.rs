use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use semver::Version;
use serde::ser;
use serde::Serialize;
use url::Url;

use crate::core::interning::InternedString;
use crate::core::profiles::Profiles;
use crate::core::{Dependency, PackageId, PackageIdSpec, SourceId, Summary};
use crate::core::{Edition, Feature, Features, WorkspaceConfig};
use crate::util::errors::*;
use crate::util::toml::TomlManifest;
use crate::util::{short_hash, Config, Filesystem};

pub enum EitherManifest {
    Real(Manifest),
    Virtual(VirtualManifest),
}

/// Contains all the information about a package, as loaded from a `Cargo.toml`.
#[derive(Clone, Debug)]
pub struct Manifest {
    summary: Summary,
    targets: Vec<Target>,
    links: Option<String>,
    warnings: Warnings,
    exclude: Vec<String>,
    include: Vec<String>,
    metadata: ManifestMetadata,
    custom_metadata: Option<toml::Value>,
    profiles: Profiles,
    publish: Option<Vec<String>>,
    publish_lockfile: bool,
    replace: Vec<(PackageIdSpec, Dependency)>,
    patch: HashMap<Url, Vec<Dependency>>,
    workspace: WorkspaceConfig,
    original: Rc<TomlManifest>,
    features: Features,
    edition: Edition,
    im_a_teapot: Option<bool>,
    default_run: Option<String>,
    metabuild: Option<Vec<String>>,
}

/// When parsing `Cargo.toml`, some warnings should silenced
/// if the manifest comes from a dependency. `ManifestWarning`
/// allows this delayed emission of warnings.
#[derive(Clone, Debug)]
pub struct DelayedWarning {
    pub message: String,
    pub is_critical: bool,
}

#[derive(Clone, Debug)]
pub struct Warnings(Vec<DelayedWarning>);

#[derive(Clone, Debug)]
pub struct VirtualManifest {
    replace: Vec<(PackageIdSpec, Dependency)>,
    patch: HashMap<Url, Vec<Dependency>>,
    workspace: WorkspaceConfig,
    profiles: Profiles,
    warnings: Warnings,
    features: Features,
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
    pub description: Option<String>,   // Not in Markdown
    pub readme: Option<String>,        // File, not contents
    pub homepage: Option<String>,      // URL
    pub repository: Option<String>,    // URL
    pub documentation: Option<String>, // URL
    pub badges: BTreeMap<String, BTreeMap<String, String>>,
    pub links: Option<String>,
}

#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum LibKind {
    Lib,
    Rlib,
    Dylib,
    ProcMacro,
    Other(String),
}

impl LibKind {
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
            LibKind::Lib | LibKind::Rlib | LibKind::Dylib | LibKind::ProcMacro => true,
            LibKind::Other(..) => false,
        }
    }

    pub fn requires_upstream_objects(&self) -> bool {
        match *self {
            // "lib" == "rlib" and is a compilation that doesn't actually
            // require upstream object files to exist, only upstream metadata
            // files. As a result, it doesn't require upstream artifacts
            LibKind::Lib | LibKind::Rlib => false,

            // Everything else, however, is some form of "linkable output" or
            // something that requires upstream object files.
            _ => true,
        }
    }
}

impl fmt::Debug for LibKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.crate_type().fmt(f)
    }
}

impl<'a> From<&'a String> for LibKind {
    fn from(string: &'a String) -> Self {
        match string.as_ref() {
            "lib" => LibKind::Lib,
            "rlib" => LibKind::Rlib,
            "dylib" => LibKind::Dylib,
            "proc-macro" => LibKind::ProcMacro,
            s => LibKind::Other(s.to_string()),
        }
    }
}

#[derive(Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
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
    where
        S: ser::Serializer,
    {
        use self::TargetKind::*;
        match *self {
            Lib(ref kinds) => s.collect_seq(kinds.iter().map(LibKind::crate_type)),
            Bin => ["bin"].serialize(s),
            ExampleBin | ExampleLib(_) => ["example"].serialize(s),
            Test => ["test"].serialize(s),
            CustomBuild => ["custom-build"].serialize(s),
            Bench => ["bench"].serialize(s),
        }
    }
}

impl fmt::Debug for TargetKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use self::TargetKind::*;
        match *self {
            Lib(ref kinds) => kinds.fmt(f),
            Bin => "bin".fmt(f),
            ExampleBin | ExampleLib(_) => "example".fmt(f),
            Test => "test".fmt(f),
            CustomBuild => "custom-build".fmt(f),
            Bench => "bench".fmt(f),
        }
    }
}

impl TargetKind {
    pub fn description(&self) -> &'static str {
        match self {
            TargetKind::Lib(..) => "lib",
            TargetKind::Bin => "bin",
            TargetKind::Test => "integration-test",
            TargetKind::ExampleBin | TargetKind::ExampleLib(..) => "example",
            TargetKind::Bench => "bench",
            TargetKind::CustomBuild => "build-script",
        }
    }
}

/// Information about a binary, a library, an example, etc. that is part of the
/// package.
#[derive(Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct Target {
    kind: TargetKind,
    name: String,
    // Note that the `src_path` here is excluded from the `Hash` implementation
    // as it's absolute currently and is otherwise a little too brittle for
    // causing rebuilds. Instead the hash for the path that we send to the
    // compiler is handled elsewhere.
    src_path: TargetSourcePath,
    required_features: Option<Vec<String>>,
    tested: bool,
    benched: bool,
    doc: bool,
    doctest: bool,
    harness: bool, // whether to use the test harness (--test)
    for_host: bool,
    proc_macro: bool,
    edition: Edition,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum TargetSourcePath {
    Path(PathBuf),
    Metabuild,
}

impl TargetSourcePath {
    pub fn path(&self) -> Option<&Path> {
        match self {
            TargetSourcePath::Path(path) => Some(path.as_ref()),
            TargetSourcePath::Metabuild => None,
        }
    }

    pub fn is_path(&self) -> bool {
        match self {
            TargetSourcePath::Path(_) => true,
            _ => false,
        }
    }
}

impl Hash for TargetSourcePath {
    fn hash<H: Hasher>(&self, _: &mut H) {
        // ...
    }
}

impl fmt::Debug for TargetSourcePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TargetSourcePath::Path(path) => path.fmt(f),
            TargetSourcePath::Metabuild => "metabuild".fmt(f),
        }
    }
}

impl From<PathBuf> for TargetSourcePath {
    fn from(path: PathBuf) -> Self {
        assert!(path.is_absolute(), "`{}` is not absolute", path.display());
        TargetSourcePath::Path(path)
    }
}

#[derive(Serialize)]
struct SerializedTarget<'a> {
    /// Is this a `--bin bin`, `--lib`, `--example ex`?
    /// Serialized as a list of strings for historical reasons.
    kind: &'a TargetKind,
    /// Corresponds to `--crate-type` compiler attribute.
    /// See https://doc.rust-lang.org/reference/linkage.html
    crate_types: Vec<&'a str>,
    name: &'a str,
    src_path: Option<&'a PathBuf>,
    edition: &'a str,
    #[serde(rename = "required-features", skip_serializing_if = "Option::is_none")]
    required_features: Option<Vec<&'a str>>,
}

impl ser::Serialize for Target {
    fn serialize<S: ser::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let src_path = match &self.src_path {
            TargetSourcePath::Path(p) => Some(p),
            // Unfortunately getting the correct path would require access to
            // target_dir, which is not available here.
            TargetSourcePath::Metabuild => None,
        };
        SerializedTarget {
            kind: &self.kind,
            crate_types: self.rustc_crate_types(),
            name: &self.name,
            src_path,
            edition: &self.edition.to_string(),
            required_features: self
                .required_features
                .as_ref()
                .map(|rf| rf.iter().map(|s| &**s).collect()),
        }
        .serialize(s)
    }
}

compact_debug! {
    impl fmt::Debug for Target {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            let (default, default_name) = {
                match &self.kind {
                    TargetKind::Lib(kinds) => {
                        (
                            Target::lib_target(
                                &self.name,
                                kinds.clone(),
                                self.src_path().path().unwrap().to_path_buf(),
                                self.edition,
                            ),
                            format!("lib_target({:?}, {:?}, {:?}, {:?})",
                                    self.name, kinds, self.src_path, self.edition),
                        )
                    }
                    TargetKind::CustomBuild => {
                        match self.src_path {
                            TargetSourcePath::Path(ref path) => {
                                (
                                    Target::custom_build_target(
                                        &self.name,
                                        path.to_path_buf(),
                                        self.edition,
                                    ),
                                    format!("custom_build_target({:?}, {:?}, {:?})",
                                            self.name, path, self.edition),
                                )
                            }
                            TargetSourcePath::Metabuild => {
                                (
                                    Target::metabuild_target(&self.name),
                                    format!("metabuild_target({:?})", self.name),
                                )
                            }
                        }
                    }
                    _ => (
                        Target::new(self.src_path.clone(), self.edition),
                        format!("with_path({:?}, {:?})", self.src_path, self.edition),
                    ),
                }
            };
            [debug_the_fields(
                kind
                name
                src_path
                required_features
                tested
                benched
                doc
                doctest
                harness
                for_host
                proc_macro
                edition
            )]
        }
    }
}

impl Manifest {
    pub fn new(
        summary: Summary,
        targets: Vec<Target>,
        exclude: Vec<String>,
        include: Vec<String>,
        links: Option<String>,
        metadata: ManifestMetadata,
        custom_metadata: Option<toml::Value>,
        profiles: Profiles,
        publish: Option<Vec<String>>,
        publish_lockfile: bool,
        replace: Vec<(PackageIdSpec, Dependency)>,
        patch: HashMap<Url, Vec<Dependency>>,
        workspace: WorkspaceConfig,
        features: Features,
        edition: Edition,
        im_a_teapot: Option<bool>,
        default_run: Option<String>,
        original: Rc<TomlManifest>,
        metabuild: Option<Vec<String>>,
    ) -> Manifest {
        Manifest {
            summary,
            targets,
            warnings: Warnings::new(),
            exclude,
            include,
            links,
            metadata,
            custom_metadata,
            profiles,
            publish,
            replace,
            patch,
            workspace,
            features,
            edition,
            original,
            im_a_teapot,
            default_run,
            publish_lockfile,
            metabuild,
        }
    }

    pub fn dependencies(&self) -> &[Dependency] {
        self.summary.dependencies()
    }
    pub fn exclude(&self) -> &[String] {
        &self.exclude
    }
    pub fn include(&self) -> &[String] {
        &self.include
    }
    pub fn metadata(&self) -> &ManifestMetadata {
        &self.metadata
    }
    pub fn name(&self) -> InternedString {
        self.package_id().name()
    }
    pub fn package_id(&self) -> PackageId {
        self.summary.package_id()
    }
    pub fn summary(&self) -> &Summary {
        &self.summary
    }
    pub fn summary_mut(&mut self) -> &mut Summary {
        &mut self.summary
    }
    pub fn targets(&self) -> &[Target] {
        &self.targets
    }
    pub fn version(&self) -> &Version {
        self.package_id().version()
    }
    pub fn warnings_mut(&mut self) -> &mut Warnings {
        &mut self.warnings
    }
    pub fn warnings(&self) -> &Warnings {
        &self.warnings
    }
    pub fn profiles(&self) -> &Profiles {
        &self.profiles
    }
    pub fn publish(&self) -> &Option<Vec<String>> {
        &self.publish
    }
    pub fn publish_lockfile(&self) -> bool {
        self.publish_lockfile
    }
    pub fn replace(&self) -> &[(PackageIdSpec, Dependency)] {
        &self.replace
    }
    pub fn original(&self) -> &TomlManifest {
        &self.original
    }
    pub fn patch(&self) -> &HashMap<Url, Vec<Dependency>> {
        &self.patch
    }
    pub fn links(&self) -> Option<&str> {
        self.links.as_ref().map(|s| &s[..])
    }

    pub fn workspace_config(&self) -> &WorkspaceConfig {
        &self.workspace
    }

    pub fn features(&self) -> &Features {
        &self.features
    }

    pub fn map_source(self, to_replace: SourceId, replace_with: SourceId) -> Manifest {
        Manifest {
            summary: self.summary.map_source(to_replace, replace_with),
            ..self
        }
    }

    pub fn feature_gate(&self) -> CargoResult<()> {
        if self.im_a_teapot.is_some() {
            self.features
                .require(Feature::test_dummy_unstable())
                .chain_err(|| {
                    failure::format_err!(
                        "the `im-a-teapot` manifest key is unstable and may \
                         not work properly in England"
                    )
                })?;
        }

        if self.default_run.is_some() {
            self.features
                .require(Feature::default_run())
                .chain_err(|| failure::format_err!("the `default-run` manifest key is unstable"))?;
        }

        Ok(())
    }

    // Just a helper function to test out `-Z` flags on Cargo
    pub fn print_teapot(&self, config: &Config) {
        if let Some(teapot) = self.im_a_teapot {
            if config.cli_unstable().print_im_a_teapot {
                println!("im-a-teapot = {}", teapot);
            }
        }
    }

    pub fn edition(&self) -> Edition {
        self.edition
    }

    pub fn custom_metadata(&self) -> Option<&toml::Value> {
        self.custom_metadata.as_ref()
    }

    pub fn default_run(&self) -> Option<&str> {
        self.default_run.as_ref().map(|s| &s[..])
    }

    pub fn metabuild(&self) -> Option<&Vec<String>> {
        self.metabuild.as_ref()
    }

    pub fn metabuild_path(&self, target_dir: Filesystem) -> PathBuf {
        let hash = short_hash(&self.package_id());
        target_dir
            .into_path_unlocked()
            .join(".metabuild")
            .join(format!("metabuild-{}-{}.rs", self.name(), hash))
    }
}

impl VirtualManifest {
    pub fn new(
        replace: Vec<(PackageIdSpec, Dependency)>,
        patch: HashMap<Url, Vec<Dependency>>,
        workspace: WorkspaceConfig,
        profiles: Profiles,
        features: Features,
    ) -> VirtualManifest {
        VirtualManifest {
            replace,
            patch,
            workspace,
            profiles,
            warnings: Warnings::new(),
            features,
        }
    }

    pub fn replace(&self) -> &[(PackageIdSpec, Dependency)] {
        &self.replace
    }

    pub fn patch(&self) -> &HashMap<Url, Vec<Dependency>> {
        &self.patch
    }

    pub fn workspace_config(&self) -> &WorkspaceConfig {
        &self.workspace
    }

    pub fn profiles(&self) -> &Profiles {
        &self.profiles
    }

    pub fn warnings_mut(&mut self) -> &mut Warnings {
        &mut self.warnings
    }

    pub fn warnings(&self) -> &Warnings {
        &self.warnings
    }

    pub fn features(&self) -> &Features {
        &self.features
    }
}

impl Target {
    fn new(src_path: TargetSourcePath, edition: Edition) -> Target {
        Target {
            kind: TargetKind::Bin,
            name: String::new(),
            src_path,
            required_features: None,
            doc: false,
            doctest: false,
            harness: true,
            for_host: false,
            proc_macro: false,
            edition,
            tested: true,
            benched: true,
        }
    }

    fn with_path(src_path: PathBuf, edition: Edition) -> Target {
        Target::new(TargetSourcePath::from(src_path), edition)
    }

    pub fn lib_target(
        name: &str,
        crate_targets: Vec<LibKind>,
        src_path: PathBuf,
        edition: Edition,
    ) -> Target {
        Target {
            kind: TargetKind::Lib(crate_targets),
            name: name.to_string(),
            doctest: true,
            doc: true,
            ..Target::with_path(src_path, edition)
        }
    }

    pub fn bin_target(
        name: &str,
        src_path: PathBuf,
        required_features: Option<Vec<String>>,
        edition: Edition,
    ) -> Target {
        Target {
            kind: TargetKind::Bin,
            name: name.to_string(),
            required_features,
            doc: true,
            ..Target::with_path(src_path, edition)
        }
    }

    /// Builds a `Target` corresponding to the `build = "build.rs"` entry.
    pub fn custom_build_target(name: &str, src_path: PathBuf, edition: Edition) -> Target {
        Target {
            kind: TargetKind::CustomBuild,
            name: name.to_string(),
            for_host: true,
            benched: false,
            tested: false,
            ..Target::with_path(src_path, edition)
        }
    }

    pub fn metabuild_target(name: &str) -> Target {
        Target {
            kind: TargetKind::CustomBuild,
            name: name.to_string(),
            for_host: true,
            benched: false,
            tested: false,
            ..Target::new(TargetSourcePath::Metabuild, Edition::Edition2018)
        }
    }

    pub fn example_target(
        name: &str,
        crate_targets: Vec<LibKind>,
        src_path: PathBuf,
        required_features: Option<Vec<String>>,
        edition: Edition,
    ) -> Target {
        let kind = if crate_targets.is_empty()
            || crate_targets
                .iter()
                .all(|t| *t == LibKind::Other("bin".into()))
        {
            TargetKind::ExampleBin
        } else {
            TargetKind::ExampleLib(crate_targets)
        };

        Target {
            kind,
            name: name.to_string(),
            required_features,
            tested: false,
            benched: false,
            ..Target::with_path(src_path, edition)
        }
    }

    pub fn test_target(
        name: &str,
        src_path: PathBuf,
        required_features: Option<Vec<String>>,
        edition: Edition,
    ) -> Target {
        Target {
            kind: TargetKind::Test,
            name: name.to_string(),
            required_features,
            benched: false,
            ..Target::with_path(src_path, edition)
        }
    }

    pub fn bench_target(
        name: &str,
        src_path: PathBuf,
        required_features: Option<Vec<String>>,
        edition: Edition,
    ) -> Target {
        Target {
            kind: TargetKind::Bench,
            name: name.to_string(),
            required_features,
            tested: false,
            ..Target::with_path(src_path, edition)
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn crate_name(&self) -> String {
        self.name.replace("-", "_")
    }
    pub fn src_path(&self) -> &TargetSourcePath {
        &self.src_path
    }
    pub fn set_src_path(&mut self, src_path: TargetSourcePath) {
        self.src_path = src_path;
    }
    pub fn required_features(&self) -> Option<&Vec<String>> {
        self.required_features.as_ref()
    }
    pub fn kind(&self) -> &TargetKind {
        &self.kind
    }
    pub fn tested(&self) -> bool {
        self.tested
    }
    pub fn harness(&self) -> bool {
        self.harness
    }
    pub fn documented(&self) -> bool {
        self.doc
    }
    pub fn for_host(&self) -> bool {
        self.for_host
    }
    pub fn proc_macro(&self) -> bool {
        self.proc_macro
    }
    pub fn edition(&self) -> Edition {
        self.edition
    }
    pub fn benched(&self) -> bool {
        self.benched
    }
    pub fn doctested(&self) -> bool {
        self.doctest
    }

    pub fn doctestable(&self) -> bool {
        match self.kind {
            TargetKind::Lib(ref kinds) => kinds
                .iter()
                .any(|k| *k == LibKind::Rlib || *k == LibKind::Lib || *k == LibKind::ProcMacro),
            _ => false,
        }
    }

    pub fn allows_underscores(&self) -> bool {
        self.is_bin() || self.is_example() || self.is_custom_build()
    }

    pub fn is_lib(&self) -> bool {
        match self.kind {
            TargetKind::Lib(_) => true,
            _ => false,
        }
    }

    pub fn is_dylib(&self) -> bool {
        match self.kind {
            TargetKind::Lib(ref libs) => libs.iter().any(|l| *l == LibKind::Dylib),
            _ => false,
        }
    }

    pub fn is_cdylib(&self) -> bool {
        let libs = match self.kind {
            TargetKind::Lib(ref libs) => libs,
            _ => return false,
        };
        libs.iter().any(|l| match *l {
            LibKind::Other(ref s) => s == "cdylib",
            _ => false,
        })
    }

    /// Returns whether this target produces an artifact which can be linked
    /// into a Rust crate.
    ///
    /// This only returns true for certain kinds of libraries.
    pub fn linkable(&self) -> bool {
        match self.kind {
            TargetKind::Lib(ref kinds) => kinds.iter().any(|k| k.linkable()),
            _ => false,
        }
    }

    /// Returns whether production of this artifact requires the object files
    /// from dependencies to be available.
    ///
    /// This only returns `false` when all we're producing is an rlib, otherwise
    /// it will return `true`.
    pub fn requires_upstream_objects(&self) -> bool {
        match &self.kind {
            TargetKind::Lib(kinds) | TargetKind::ExampleLib(kinds) => {
                kinds.iter().any(|k| k.requires_upstream_objects())
            }
            _ => true,
        }
    }

    pub fn is_bin(&self) -> bool {
        self.kind == TargetKind::Bin
    }

    pub fn is_example(&self) -> bool {
        match self.kind {
            TargetKind::ExampleBin | TargetKind::ExampleLib(..) => true,
            _ => false,
        }
    }

    /// Returns `true` if it is a binary or executable example.
    /// NOTE: Tests are `false`!
    pub fn is_executable(&self) -> bool {
        self.is_bin() || self.is_exe_example()
    }

    /// Returns `true` if it is an executable example.
    pub fn is_exe_example(&self) -> bool {
        // Needed for --all-examples in contexts where only runnable examples make sense
        match self.kind {
            TargetKind::ExampleBin => true,
            _ => false,
        }
    }

    pub fn is_test(&self) -> bool {
        self.kind == TargetKind::Test
    }
    pub fn is_bench(&self) -> bool {
        self.kind == TargetKind::Bench
    }
    pub fn is_custom_build(&self) -> bool {
        self.kind == TargetKind::CustomBuild
    }

    /// Returns the arguments suitable for `--crate-type` to pass to rustc.
    pub fn rustc_crate_types(&self) -> Vec<&str> {
        match self.kind {
            TargetKind::Lib(ref kinds) | TargetKind::ExampleLib(ref kinds) => {
                kinds.iter().map(LibKind::crate_type).collect()
            }
            TargetKind::CustomBuild
            | TargetKind::Bench
            | TargetKind::Test
            | TargetKind::ExampleBin
            | TargetKind::Bin => vec!["bin"],
        }
    }

    pub fn can_lto(&self) -> bool {
        match self.kind {
            TargetKind::Lib(ref v) => {
                !v.contains(&LibKind::Rlib)
                    && !v.contains(&LibKind::Dylib)
                    && !v.contains(&LibKind::Lib)
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
    pub fn set_proc_macro(&mut self, proc_macro: bool) -> &mut Target {
        self.proc_macro = proc_macro;
        self
    }
    pub fn set_edition(&mut self, edition: Edition) -> &mut Target {
        self.edition = edition;
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
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            TargetKind::Lib(..) => write!(f, "Target(lib)"),
            TargetKind::Bin => write!(f, "Target(bin: {})", self.name),
            TargetKind::Test => write!(f, "Target(test: {})", self.name),
            TargetKind::Bench => write!(f, "Target(bench: {})", self.name),
            TargetKind::ExampleBin | TargetKind::ExampleLib(..) => {
                write!(f, "Target(example: {})", self.name)
            }
            TargetKind::CustomBuild => write!(f, "Target(script)"),
        }
    }
}

impl Warnings {
    fn new() -> Warnings {
        Warnings(Vec::new())
    }

    pub fn add_warning(&mut self, s: String) {
        self.0.push(DelayedWarning {
            message: s,
            is_critical: false,
        })
    }

    pub fn add_critical_warning(&mut self, s: String) {
        self.0.push(DelayedWarning {
            message: s,
            is_critical: true,
        })
    }

    pub fn warnings(&self) -> &[DelayedWarning] {
        &self.0
    }
}
