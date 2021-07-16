use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use anyhow::Context as _;
use semver::Version;
use serde::ser;
use serde::Serialize;
use url::Url;

use crate::core::compiler::{CompileKind, CrateType};
use crate::core::resolver::ResolveBehavior;
use crate::core::{Dependency, PackageId, PackageIdSpec, SourceId, Summary};
use crate::core::{Edition, Feature, Features, WorkspaceConfig};
use crate::util::errors::*;
use crate::util::interning::InternedString;
use crate::util::toml::{TomlManifest, TomlProfiles};
use crate::util::{short_hash, Config, Filesystem};

pub enum EitherManifest {
    Real(Manifest),
    Virtual(VirtualManifest),
}

/// Contains all the information about a package, as loaded from a `Cargo.toml`.
///
/// This is deserialized using the [`TomlManifest`] type.
#[derive(Clone, Debug)]
pub struct Manifest {
    summary: Summary,
    targets: Vec<Target>,
    default_kind: Option<CompileKind>,
    forced_kind: Option<CompileKind>,
    links: Option<String>,
    warnings: Warnings,
    exclude: Vec<String>,
    include: Vec<String>,
    metadata: ManifestMetadata,
    custom_metadata: Option<toml::Value>,
    profiles: Option<TomlProfiles>,
    publish: Option<Vec<String>>,
    replace: Vec<(PackageIdSpec, Dependency)>,
    patch: HashMap<Url, Vec<Dependency>>,
    workspace: WorkspaceConfig,
    original: Rc<TomlManifest>,
    unstable_features: Features,
    edition: Edition,
    rust_version: Option<String>,
    im_a_teapot: Option<bool>,
    default_run: Option<String>,
    metabuild: Option<Vec<String>>,
    resolve_behavior: Option<ResolveBehavior>,
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
    profiles: Option<TomlProfiles>,
    warnings: Warnings,
    features: Features,
    resolve_behavior: Option<ResolveBehavior>,
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

#[derive(Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum TargetKind {
    Lib(Vec<CrateType>),
    Bin,
    Test,
    Bench,
    ExampleLib(Vec<CrateType>),
    ExampleBin,
    CustomBuild,
}

impl ser::Serialize for TargetKind {
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        use self::TargetKind::*;
        match self {
            Lib(kinds) => s.collect_seq(kinds.iter().map(|t| t.to_string())),
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

    /// Returns whether production of this artifact requires the object files
    /// from dependencies to be available.
    ///
    /// This only returns `false` when all we're producing is an rlib, otherwise
    /// it will return `true`.
    pub fn requires_upstream_objects(&self) -> bool {
        match self {
            TargetKind::Lib(kinds) | TargetKind::ExampleLib(kinds) => {
                kinds.iter().any(|k| k.requires_upstream_objects())
            }
            _ => true,
        }
    }

    /// Returns the arguments suitable for `--crate-type` to pass to rustc.
    pub fn rustc_crate_types(&self) -> Vec<CrateType> {
        match self {
            TargetKind::Lib(kinds) | TargetKind::ExampleLib(kinds) => kinds.clone(),
            TargetKind::CustomBuild
            | TargetKind::Bench
            | TargetKind::Test
            | TargetKind::ExampleBin
            | TargetKind::Bin => vec![CrateType::Bin],
        }
    }
}

/// Information about a binary, a library, an example, etc. that is part of the
/// package.
#[derive(Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct Target {
    inner: Arc<TargetInner>,
}

#[derive(Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
struct TargetInner {
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
        matches!(self, TargetSourcePath::Path(_))
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
    crate_types: Vec<CrateType>,
    name: &'a str,
    src_path: Option<&'a PathBuf>,
    edition: &'a str,
    #[serde(rename = "required-features", skip_serializing_if = "Option::is_none")]
    required_features: Option<Vec<&'a str>>,
    /// Whether docs should be built for the target via `cargo doc`
    /// See https://doc.rust-lang.org/cargo/commands/cargo-doc.html#target-selection
    doc: bool,
    doctest: bool,
    /// Whether tests should be run for the target (`test` field in `Cargo.toml`)
    test: bool,
}

impl ser::Serialize for Target {
    fn serialize<S: ser::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let src_path = match self.src_path() {
            TargetSourcePath::Path(p) => Some(p),
            // Unfortunately getting the correct path would require access to
            // target_dir, which is not available here.
            TargetSourcePath::Metabuild => None,
        };
        SerializedTarget {
            kind: self.kind(),
            crate_types: self.rustc_crate_types(),
            name: self.name(),
            src_path,
            edition: &self.edition().to_string(),
            required_features: self
                .required_features()
                .map(|rf| rf.iter().map(|s| s.as_str()).collect()),
            doc: self.documented(),
            doctest: self.doctested() && self.doctestable(),
            test: self.tested(),
        }
        .serialize(s)
    }
}

impl fmt::Debug for Target {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}

compact_debug! {
    impl fmt::Debug for TargetInner {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            let (default, default_name) = {
                match &self.kind {
                    TargetKind::Lib(kinds) => {
                        (
                            Target::lib_target(
                                &self.name,
                                kinds.clone(),
                                self.src_path.path().unwrap().to_path_buf(),
                                self.edition,
                            ).inner,
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
                                    ).inner,
                                    format!("custom_build_target({:?}, {:?}, {:?})",
                                            self.name, path, self.edition),
                                )
                            }
                            TargetSourcePath::Metabuild => {
                                (
                                    Target::metabuild_target(&self.name).inner,
                                    format!("metabuild_target({:?})", self.name),
                                )
                            }
                        }
                    }
                    _ => (
                        Target::new(self.src_path.clone(), self.edition).inner,
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
        default_kind: Option<CompileKind>,
        forced_kind: Option<CompileKind>,
        targets: Vec<Target>,
        exclude: Vec<String>,
        include: Vec<String>,
        links: Option<String>,
        metadata: ManifestMetadata,
        custom_metadata: Option<toml::Value>,
        profiles: Option<TomlProfiles>,
        publish: Option<Vec<String>>,
        replace: Vec<(PackageIdSpec, Dependency)>,
        patch: HashMap<Url, Vec<Dependency>>,
        workspace: WorkspaceConfig,
        unstable_features: Features,
        edition: Edition,
        rust_version: Option<String>,
        im_a_teapot: Option<bool>,
        default_run: Option<String>,
        original: Rc<TomlManifest>,
        metabuild: Option<Vec<String>>,
        resolve_behavior: Option<ResolveBehavior>,
    ) -> Manifest {
        Manifest {
            summary,
            default_kind,
            forced_kind,
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
            unstable_features,
            edition,
            rust_version,
            original,
            im_a_teapot,
            default_run,
            metabuild,
            resolve_behavior,
        }
    }

    pub fn dependencies(&self) -> &[Dependency] {
        self.summary.dependencies()
    }
    pub fn default_kind(&self) -> Option<CompileKind> {
        self.default_kind
    }
    pub fn forced_kind(&self) -> Option<CompileKind> {
        self.forced_kind
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
    // It is used by cargo-c, please do not remove it
    pub fn targets_mut(&mut self) -> &mut [Target] {
        &mut self.targets
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
    pub fn profiles(&self) -> Option<&TomlProfiles> {
        self.profiles.as_ref()
    }
    pub fn publish(&self) -> &Option<Vec<String>> {
        &self.publish
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
        self.links.as_deref()
    }

    pub fn workspace_config(&self) -> &WorkspaceConfig {
        &self.workspace
    }

    /// Unstable, nightly features that are enabled in this manifest.
    pub fn unstable_features(&self) -> &Features {
        &self.unstable_features
    }

    /// The style of resolver behavior to use, declared with the `resolver` field.
    ///
    /// Returns `None` if it is not specified.
    pub fn resolve_behavior(&self) -> Option<ResolveBehavior> {
        self.resolve_behavior
    }

    pub fn map_source(self, to_replace: SourceId, replace_with: SourceId) -> Manifest {
        Manifest {
            summary: self.summary.map_source(to_replace, replace_with),
            ..self
        }
    }

    pub fn feature_gate(&self) -> CargoResult<()> {
        if self.im_a_teapot.is_some() {
            self.unstable_features
                .require(Feature::test_dummy_unstable())
                .with_context(|| {
                    "the `im-a-teapot` manifest key is unstable and may \
                     not work properly in England"
                })?;
        }

        if self.default_kind.is_some() || self.forced_kind.is_some() {
            self.unstable_features
                .require(Feature::per_package_target())
                .with_context(|| {
                    "the `package.default-target` and `package.forced-target` \
                     manifest keys are unstable and may not work properly"
                })?;
        }

        Ok(())
    }

    // Just a helper function to test out `-Z` flags on Cargo
    pub fn print_teapot(&self, config: &Config) {
        if let Some(teapot) = self.im_a_teapot {
            if config.cli_unstable().print_im_a_teapot {
                crate::drop_println!(config, "im-a-teapot = {}", teapot);
            }
        }
    }

    pub fn edition(&self) -> Edition {
        self.edition
    }

    pub fn rust_version(&self) -> Option<&str> {
        self.rust_version.as_deref()
    }

    pub fn custom_metadata(&self) -> Option<&toml::Value> {
        self.custom_metadata.as_ref()
    }

    pub fn default_run(&self) -> Option<&str> {
        self.default_run.as_deref()
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
        profiles: Option<TomlProfiles>,
        features: Features,
        resolve_behavior: Option<ResolveBehavior>,
    ) -> VirtualManifest {
        VirtualManifest {
            replace,
            patch,
            workspace,
            profiles,
            warnings: Warnings::new(),
            features,
            resolve_behavior,
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

    pub fn profiles(&self) -> Option<&TomlProfiles> {
        self.profiles.as_ref()
    }

    pub fn warnings_mut(&mut self) -> &mut Warnings {
        &mut self.warnings
    }

    pub fn warnings(&self) -> &Warnings {
        &self.warnings
    }

    pub fn unstable_features(&self) -> &Features {
        &self.features
    }

    /// The style of resolver behavior to use, declared with the `resolver` field.
    ///
    /// Returns `None` if it is not specified.
    pub fn resolve_behavior(&self) -> Option<ResolveBehavior> {
        self.resolve_behavior
    }
}

impl Target {
    fn new(src_path: TargetSourcePath, edition: Edition) -> Target {
        Target {
            inner: Arc::new(TargetInner {
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
            }),
        }
    }

    fn with_path(src_path: PathBuf, edition: Edition) -> Target {
        Target::new(TargetSourcePath::from(src_path), edition)
    }

    pub fn lib_target(
        name: &str,
        crate_targets: Vec<CrateType>,
        src_path: PathBuf,
        edition: Edition,
    ) -> Target {
        let mut target = Target::with_path(src_path, edition);
        target
            .set_kind(TargetKind::Lib(crate_targets))
            .set_name(name)
            .set_doctest(true)
            .set_doc(true);
        target
    }

    pub fn bin_target(
        name: &str,
        src_path: PathBuf,
        required_features: Option<Vec<String>>,
        edition: Edition,
    ) -> Target {
        let mut target = Target::with_path(src_path, edition);
        target
            .set_kind(TargetKind::Bin)
            .set_name(name)
            .set_required_features(required_features)
            .set_doc(true);
        target
    }

    /// Builds a `Target` corresponding to the `build = "build.rs"` entry.
    pub fn custom_build_target(name: &str, src_path: PathBuf, edition: Edition) -> Target {
        let mut target = Target::with_path(src_path, edition);
        target
            .set_kind(TargetKind::CustomBuild)
            .set_name(name)
            .set_for_host(true)
            .set_benched(false)
            .set_tested(false);
        target
    }

    pub fn metabuild_target(name: &str) -> Target {
        let mut target = Target::new(TargetSourcePath::Metabuild, Edition::Edition2018);
        target
            .set_kind(TargetKind::CustomBuild)
            .set_name(name)
            .set_for_host(true)
            .set_benched(false)
            .set_tested(false);
        target
    }

    pub fn example_target(
        name: &str,
        crate_targets: Vec<CrateType>,
        src_path: PathBuf,
        required_features: Option<Vec<String>>,
        edition: Edition,
    ) -> Target {
        let kind = if crate_targets.is_empty() || crate_targets.iter().all(|t| *t == CrateType::Bin)
        {
            TargetKind::ExampleBin
        } else {
            TargetKind::ExampleLib(crate_targets)
        };
        let mut target = Target::with_path(src_path, edition);
        target
            .set_kind(kind)
            .set_name(name)
            .set_required_features(required_features)
            .set_tested(false)
            .set_benched(false);
        target
    }

    pub fn test_target(
        name: &str,
        src_path: PathBuf,
        required_features: Option<Vec<String>>,
        edition: Edition,
    ) -> Target {
        let mut target = Target::with_path(src_path, edition);
        target
            .set_kind(TargetKind::Test)
            .set_name(name)
            .set_required_features(required_features)
            .set_benched(false);
        target
    }

    pub fn bench_target(
        name: &str,
        src_path: PathBuf,
        required_features: Option<Vec<String>>,
        edition: Edition,
    ) -> Target {
        let mut target = Target::with_path(src_path, edition);
        target
            .set_kind(TargetKind::Bench)
            .set_name(name)
            .set_required_features(required_features)
            .set_tested(false);
        target
    }

    pub fn name(&self) -> &str {
        &self.inner.name
    }
    pub fn crate_name(&self) -> String {
        self.name().replace("-", "_")
    }
    pub fn src_path(&self) -> &TargetSourcePath {
        &self.inner.src_path
    }
    pub fn set_src_path(&mut self, src_path: TargetSourcePath) {
        Arc::make_mut(&mut self.inner).src_path = src_path;
    }
    pub fn required_features(&self) -> Option<&Vec<String>> {
        self.inner.required_features.as_ref()
    }
    pub fn kind(&self) -> &TargetKind {
        &self.inner.kind
    }
    pub fn tested(&self) -> bool {
        self.inner.tested
    }
    pub fn harness(&self) -> bool {
        self.inner.harness
    }
    pub fn documented(&self) -> bool {
        self.inner.doc
    }
    // A plugin, proc-macro, or build-script.
    pub fn for_host(&self) -> bool {
        self.inner.for_host
    }
    pub fn proc_macro(&self) -> bool {
        self.inner.proc_macro
    }
    pub fn edition(&self) -> Edition {
        self.inner.edition
    }
    pub fn benched(&self) -> bool {
        self.inner.benched
    }
    pub fn doctested(&self) -> bool {
        self.inner.doctest
    }

    pub fn doctestable(&self) -> bool {
        match self.kind() {
            TargetKind::Lib(ref kinds) => kinds.iter().any(|k| {
                *k == CrateType::Rlib || *k == CrateType::Lib || *k == CrateType::ProcMacro
            }),
            _ => false,
        }
    }

    pub fn is_lib(&self) -> bool {
        matches!(self.kind(), TargetKind::Lib(_))
    }

    pub fn is_dylib(&self) -> bool {
        match self.kind() {
            TargetKind::Lib(libs) => libs.iter().any(|l| *l == CrateType::Dylib),
            _ => false,
        }
    }

    pub fn is_cdylib(&self) -> bool {
        match self.kind() {
            TargetKind::Lib(libs) => libs.iter().any(|l| *l == CrateType::Cdylib),
            _ => false,
        }
    }

    /// Returns whether this target produces an artifact which can be linked
    /// into a Rust crate.
    ///
    /// This only returns true for certain kinds of libraries.
    pub fn is_linkable(&self) -> bool {
        match self.kind() {
            TargetKind::Lib(kinds) => kinds.iter().any(|k| k.is_linkable()),
            _ => false,
        }
    }

    pub fn is_bin(&self) -> bool {
        *self.kind() == TargetKind::Bin
    }

    pub fn is_example(&self) -> bool {
        matches!(
            self.kind(),
            TargetKind::ExampleBin | TargetKind::ExampleLib(..)
        )
    }

    /// Returns `true` if it is a binary or executable example.
    /// NOTE: Tests are `false`!
    pub fn is_executable(&self) -> bool {
        self.is_bin() || self.is_exe_example()
    }

    /// Returns `true` if it is an executable example.
    pub fn is_exe_example(&self) -> bool {
        // Needed for --all-examples in contexts where only runnable examples make sense
        matches!(self.kind(), TargetKind::ExampleBin)
    }

    pub fn is_test(&self) -> bool {
        *self.kind() == TargetKind::Test
    }
    pub fn is_bench(&self) -> bool {
        *self.kind() == TargetKind::Bench
    }
    pub fn is_custom_build(&self) -> bool {
        *self.kind() == TargetKind::CustomBuild
    }

    /// Returns the arguments suitable for `--crate-type` to pass to rustc.
    pub fn rustc_crate_types(&self) -> Vec<CrateType> {
        self.kind().rustc_crate_types()
    }

    pub fn set_tested(&mut self, tested: bool) -> &mut Target {
        Arc::make_mut(&mut self.inner).tested = tested;
        self
    }
    pub fn set_benched(&mut self, benched: bool) -> &mut Target {
        Arc::make_mut(&mut self.inner).benched = benched;
        self
    }
    pub fn set_doctest(&mut self, doctest: bool) -> &mut Target {
        Arc::make_mut(&mut self.inner).doctest = doctest;
        self
    }
    pub fn set_for_host(&mut self, for_host: bool) -> &mut Target {
        Arc::make_mut(&mut self.inner).for_host = for_host;
        self
    }
    pub fn set_proc_macro(&mut self, proc_macro: bool) -> &mut Target {
        Arc::make_mut(&mut self.inner).proc_macro = proc_macro;
        self
    }
    pub fn set_edition(&mut self, edition: Edition) -> &mut Target {
        Arc::make_mut(&mut self.inner).edition = edition;
        self
    }
    pub fn set_harness(&mut self, harness: bool) -> &mut Target {
        Arc::make_mut(&mut self.inner).harness = harness;
        self
    }
    pub fn set_doc(&mut self, doc: bool) -> &mut Target {
        Arc::make_mut(&mut self.inner).doc = doc;
        self
    }
    pub fn set_kind(&mut self, kind: TargetKind) -> &mut Target {
        Arc::make_mut(&mut self.inner).kind = kind;
        self
    }
    pub fn set_name(&mut self, name: &str) -> &mut Target {
        Arc::make_mut(&mut self.inner).name = name.to_string();
        self
    }
    pub fn set_required_features(&mut self, required_features: Option<Vec<String>>) -> &mut Target {
        Arc::make_mut(&mut self.inner).required_features = required_features;
        self
    }

    pub fn description_named(&self) -> String {
        match self.kind() {
            TargetKind::Lib(..) => "lib".to_string(),
            TargetKind::Bin => format!("bin \"{}\"", self.name()),
            TargetKind::Test => format!("test \"{}\"", self.name()),
            TargetKind::Bench => format!("bench \"{}\"", self.name()),
            TargetKind::ExampleLib(..) | TargetKind::ExampleBin => {
                format!("example \"{}\"", self.name())
            }
            TargetKind::CustomBuild => "build script".to_string(),
        }
    }
}

impl fmt::Display for Target {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind() {
            TargetKind::Lib(..) => write!(f, "Target(lib)"),
            TargetKind::Bin => write!(f, "Target(bin: {})", self.name()),
            TargetKind::Test => write!(f, "Target(test: {})", self.name()),
            TargetKind::Bench => write!(f, "Target(bench: {})", self.name()),
            TargetKind::ExampleBin | TargetKind::ExampleLib(..) => {
                write!(f, "Target(example: {})", self.name())
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
