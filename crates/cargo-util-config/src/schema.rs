//! Cargo configuration schemas.
//!
//! This module contains types that define the schema for various configuration
//! sections found in Cargo configuration.
//!
//! These types are mostly used by [`ConfigReader::get`](super::ConfigReader::get)
//! to deserialize configuration values from TOML files, environment variables,
//! and CLI arguments.
//!
//! Schema types here should only contain data and simple accessor methods.
//! Avoid depending on Cargo's `GlobalContext` directly.

use cargo_util::collections::HashMap;
use std::borrow::Cow;
use std::ffi::OsStr;
use std::str::FromStr;
use std::{fmt, hash};

use cargo_credential::Secret;
use cargo_util_schemas::manifest::ProfileName;
use serde::Deserialize;
use serde::Serialize;
use serde::de;
use serde::de::Error;
use serde_untagged::UntaggedEnumVisitor;

use std::path::Path;

use anyhow::Result;

use super::CRATES_IO_REGISTRY;
use super::OptValue;
use super::Value;

pub use cargo_util_schemas::manifest::TomlProfiles;

/// Cargo's `.cargo/config.toml` file
#[cfg(feature = "unstable-schema")]
#[derive(schemars::JsonSchema)]
#[schemars(rename_all = "kebab-case")]
pub struct Config {
    pub paths: Option<Vec<ConfigRelativePath>>,
    pub target_applies_to_host: Option<bool>,

    pub build: Option<CargoBuildConfig>,
    pub cache: Option<CargoCacheConfig>,
    pub cargo_new: Option<CargoNewConfig>,
    pub credential_alias: Option<HashMap<String, PathAndArgs>>,
    pub doc: Option<DocConfig>,
    pub env: Option<EnvConfig>,
    pub future_incompat_report: Option<CargoFutureIncompatConfig>,
    pub http: Option<CargoHttpConfig>,
    pub net: Option<CargoNetConfig>,
    pub patch: Option<HashMap<String, HashMap<String, PatchDependency>>>,
    pub path_bases: Option<HashMap<String, ConfigRelativePath>>,
    pub profile: Option<TomlProfiles>,
    pub registries: Option<HashMap<String, RegistryConfig>>,
    pub registry: Option<GlobalRegistryConfig>,
    pub resolver: Option<CargoResolverConfig>,
    pub source: Option<HashMap<String, SourceConfigDef>>,
    pub term: Option<TermConfig>,
    #[schemars(with = "Option<cargo_util_schemas::schema::TomlValueWrapper>")]
    pub unstable: Option<toml::Value>,
}

/// Cargo's `credentials.toml` file
#[cfg(feature = "unstable-schema")]
#[derive(schemars::JsonSchema)]
pub struct Credentials {
    pub registry: Option<RegistryCredential>,
    pub registries: Option<HashMap<String, RegistryCredential>>,
}

pub type PatchDependency = cargo_util_schemas::manifest::TomlDependency<ConfigRelativePath>;

#[cfg_attr(feature = "unstable-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub struct CargoCacheConfig {
    /// How often to automatically clean unused cache data.
    pub auto_clean_frequency: Option<String>,
    /// Settings for cleaning the global cache.
    pub global_clean: Option<GlobalCleanConfig>,
}

/// Cache cleaning settings from the `cache.global-clean` config table.
///
/// NOTE: Not all of these options may get stabilized. Some of them are very
/// low-level details, and may not be something typical users need.
///
/// If any of these options are `None`, the built-in default is used.
#[cfg_attr(feature = "unstable-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub struct GlobalCleanConfig {
    /// Anything older than this duration will be deleted in the source cache.
    pub max_src_age: Option<String>,
    /// Anything older than this duration will be deleted in the compressed crate cache.
    pub max_crate_age: Option<String>,
    /// Any index older than this duration will be deleted from the index cache.
    pub max_index_age: Option<String>,
    /// Any git checkout older than this duration will be deleted from the checkout cache.
    pub max_git_co_age: Option<String>,
    /// Any git clone older than this duration will be deleted from the git cache.
    pub max_git_db_age: Option<String>,
}

#[cfg_attr(feature = "unstable-schema", derive(schemars::JsonSchema))]
#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct CargoNewConfig {
    #[deprecated = "cargo-new no longer supports adding the authors field"]
    #[expect(dead_code, reason = "deprecated")]
    name: Option<String>,

    #[deprecated = "cargo-new no longer supports adding the authors field"]
    #[expect(dead_code, reason = "deprecated")]
    email: Option<String>,

    #[serde(rename = "vcs")]
    pub version_control: Option<VersionControl>,
}

#[cfg_attr(feature = "unstable-schema", derive(schemars::JsonSchema))]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum VersionControl {
    Git,
    Hg,
    Pijul,
    Fossil,
    NoVcs,
}

impl VersionControl {
    pub const VALUES: &[Self] = &[Self::Git, Self::Hg, Self::Pijul, Self::Fossil, Self::NoVcs];

    pub fn as_str(&self) -> &'static str {
        match self {
            VersionControl::Git => "git",
            VersionControl::Hg => "hg",
            VersionControl::Pijul => "pijul",
            VersionControl::Fossil => "fossil",
            VersionControl::NoVcs => "none",
        }
    }
}

impl FromStr for VersionControl {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, anyhow::Error> {
        match s {
            "git" => Ok(VersionControl::Git),
            "hg" => Ok(VersionControl::Hg),
            "pijul" => Ok(VersionControl::Pijul),
            "fossil" => Ok(VersionControl::Fossil),
            "none" => Ok(VersionControl::NoVcs),
            other => anyhow::bail!("unknown vcs specification: `{}`", other),
        }
    }
}

impl<'de> de::Deserialize<'de> for VersionControl {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        FromStr::from_str(&s).map_err(de::Error::custom)
    }
}

/// Definition of a source in a config file.
#[cfg_attr(feature = "unstable-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct SourceConfigDef {
    /// Indicates this source should be replaced with another of the given name.
    pub replace_with: OptValue<String>,
    /// A directory source.
    pub directory: Option<ConfigRelativePath>,
    /// A registry source. Value is a URL.
    pub registry: OptValue<String>,
    /// A local registry source.
    pub local_registry: Option<ConfigRelativePath>,
    /// A git source. Value is a URL.
    pub git: OptValue<String>,
    /// The git branch.
    pub branch: OptValue<String>,
    /// The git tag.
    pub tag: OptValue<String>,
    /// The git revision.
    pub rev: OptValue<String>,
}

#[cfg(feature = "unstable-schema")]
#[derive(schemars::JsonSchema)]
#[schemars(rename_all = "kebab-case")]
pub struct DocConfig {
    pub browser: Option<PathAndArgs>,
    pub extern_map: Option<RustdocExternMap>,
}

/// A map of registry names to URLs where documentations are hosted.
/// This is for unstable feature [`-Zrustdoc-map`][1].
///
/// [1]: https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#rustdoc-map
#[cfg_attr(feature = "unstable-schema", derive(schemars::JsonSchema))]
#[derive(serde::Deserialize, Debug)]
#[serde(default)]
pub struct RustdocExternMap {
    #[serde(deserialize_with = "default_crates_io_to_docs_rs")]
    /// * Key is the registry name in the configuration `[registries.<name>]`.
    /// * Value is the URL where the documentation is hosted.
    pub registries: HashMap<String, String>,
    pub std: Option<RustdocExternMode>,
}

impl Default for RustdocExternMap {
    fn default() -> Self {
        Self {
            registries: HashMap::from_iter([(CRATES_IO_REGISTRY.into(), DOCS_RS_URL.into())]),
            std: None,
        }
    }
}

const DOCS_RS_URL: &'static str = "https://docs.rs/";

fn default_crates_io_to_docs_rs<'de, D: serde::Deserializer<'de>>(
    de: D,
) -> Result<HashMap<String, String>, D::Error> {
    let mut registries = HashMap::deserialize(de)?;
    if !registries.contains_key(CRATES_IO_REGISTRY) {
        registries.insert(CRATES_IO_REGISTRY.into(), DOCS_RS_URL.into());
    }
    Ok(registries)
}

impl hash::Hash for RustdocExternMap {
    fn hash<H: hash::Hasher>(&self, into: &mut H) {
        self.std.hash(into);
        for (key, value) in &self.registries {
            key.hash(into);
            value.hash(into);
        }
    }
}

/// Mode used for `std`. This is for unstable feature [`-Zrustdoc-map`][1].
///
/// [1]: https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#rustdoc-map
#[cfg_attr(feature = "unstable-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Hash)]
pub enum RustdocExternMode {
    /// Use a local `file://` URL.
    Local,
    /// Use a remote URL to <https://doc.rust-lang.org/> (default).
    Remote,
    /// An arbitrary URL.
    Url(String),
}

impl From<String> for RustdocExternMode {
    fn from(s: String) -> RustdocExternMode {
        match s.as_ref() {
            "local" => RustdocExternMode::Local,
            "remote" => RustdocExternMode::Remote,
            _ => RustdocExternMode::Url(s),
        }
    }
}

impl fmt::Display for RustdocExternMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RustdocExternMode::Local => "local".fmt(f),
            RustdocExternMode::Remote => "remote".fmt(f),
            RustdocExternMode::Url(s) => s.fmt(f),
        }
    }
}

impl<'de> serde::de::Deserialize<'de> for RustdocExternMode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(s.into())
    }
}

/// The `[http]` table.
///
/// Example configuration:
///
/// ```toml
/// [http]
/// proxy = "host:port"
/// timeout = 30
/// cainfo = "/path/to/ca-bundle.crt"
/// check-revoke = true
/// multiplexing = true
/// ssl-version = "tlsv1.3"
/// ```
#[cfg_attr(feature = "unstable-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub struct CargoHttpConfig {
    pub proxy: Option<String>,
    pub low_speed_limit: Option<u32>,
    pub timeout: Option<u64>,
    pub cainfo: Option<ConfigRelativePath>,
    pub proxy_cainfo: Option<ConfigRelativePath>,
    pub check_revoke: Option<bool>,
    pub user_agent: Option<String>,
    pub debug: Option<bool>,
    pub multiplexing: Option<bool>,
    pub ssl_version: Option<SslVersionConfig>,
}

/// The `[future-incompat-report]` stable
///
/// Example configuration:
///
/// ```toml
/// [future-incompat-report]
/// frequency = "always"
/// ```
#[cfg_attr(feature = "unstable-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub struct CargoFutureIncompatConfig {
    frequency: Option<CargoFutureIncompatFrequencyConfig>,
}

#[cfg_attr(feature = "unstable-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum CargoFutureIncompatFrequencyConfig {
    #[default]
    Always,
    Never,
}

impl CargoFutureIncompatConfig {
    pub fn should_display_message(&self) -> bool {
        use CargoFutureIncompatFrequencyConfig::*;

        let frequency = self.frequency.as_ref().unwrap_or(&Always);
        match frequency {
            Always => true,
            Never => false,
        }
    }
}

/// Configuration for `ssl-version` in `http` section
/// There are two ways to configure:
///
/// ```text
/// [http]
/// ssl-version = "tlsv1.3"
/// ```
///
/// ```text
/// [http]
/// ssl-version.min = "tlsv1.2"
/// ssl-version.max = "tlsv1.3"
/// ```
#[cfg_attr(feature = "unstable-schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, PartialEq)]
pub enum SslVersionConfig {
    Single(String),
    Range(SslVersionConfigRange),
}

impl<'de> Deserialize<'de> for SslVersionConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = SslVersionConfig;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a string or map")
            }

            fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
                Ok(SslVersionConfig::Single(value.to_owned()))
            }

            fn visit_string<E: de::Error>(self, value: String) -> Result<Self::Value, E> {
                Ok(SslVersionConfig::Single(value))
            }

            fn visit_map<M: de::MapAccess<'de>>(self, map: M) -> Result<Self::Value, M::Error> {
                SslVersionConfigRange::deserialize(de::value::MapAccessDeserializer::new(map))
                    .map(SslVersionConfig::Range)
            }
        }

        deserializer.deserialize_any(Visitor)
    }
}

#[cfg_attr(feature = "unstable-schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub struct SslVersionConfigRange {
    pub min: Option<String>,
    pub max: Option<String>,
}

/// The `[net]` table.
///
/// Example configuration:
///
/// ```toml
/// [net]
/// retry = 2
/// offline = false
/// git-fetch-with-cli = true
/// ```
#[cfg_attr(feature = "unstable-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct CargoNetConfig {
    pub retry: Option<u32>,
    pub offline: Option<bool>,
    pub git_fetch_with_cli: Option<bool>,
    pub ssh: Option<CargoSshConfig>,
}

#[cfg_attr(feature = "unstable-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct CargoSshConfig {
    pub known_hosts: Option<Vec<Value<String>>>,
}

/// Configuration for `jobs` in `build` section. There are two
/// ways to configure: An integer or a simple string expression.
///
/// ```toml
/// [build]
/// jobs = 1
/// ```
///
/// ```toml
/// [build]
/// jobs = "default" # Currently only support "default".
/// ```
#[cfg_attr(feature = "unstable-schema", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "unstable-schema", schemars(untagged))]
#[derive(Debug, Clone)]
pub enum JobsConfig {
    Integer(i32),
    String(String),
}

impl<'de> Deserialize<'de> for JobsConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        UntaggedEnumVisitor::new()
            .i32(|int| Ok(JobsConfig::Integer(int)))
            .string(|string| Ok(JobsConfig::String(string.to_owned())))
            .deserialize(deserializer)
    }
}

/// The `[build]` table.
///
/// Example configuration:
///
/// ```toml
/// [build]
/// jobs = 4
/// target = "x86_64-unknown-linux-gnu"
/// target-dir = "target"
/// rustflags = ["-C", "link-arg=-fuse-ld=lld"]
/// incremental = true
/// ```
#[cfg_attr(feature = "unstable-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct CargoBuildConfig {
    // deprecated, but preserved for compatibility
    pub pipelining: Option<bool>,
    pub dep_info_basedir: Option<ConfigRelativePath>,
    pub target_dir: Option<ConfigRelativePath>,
    pub build_dir: Option<ConfigRelativePath>,
    pub incremental: Option<bool>,
    pub target: Option<BuildTargetConfig>,
    pub profile: Option<ProfileName>,
    pub jobs: Option<JobsConfig>,
    pub rustflags: Option<StringList>,
    pub rustdocflags: Option<StringList>,
    pub rustc_wrapper: Option<ConfigRelativePath>,
    pub rustc_workspace_wrapper: Option<ConfigRelativePath>,
    pub rustc: Option<ConfigRelativePath>,
    pub rustdoc: Option<ConfigRelativePath>,
    pub artifact_dir: Option<ConfigRelativePath>,
    pub warnings: Option<WarningHandling>,
    /// Unstable feature `-Zsbom`.
    pub sbom: Option<bool>,
    /// Unstable feature `-Zbuild-analysis`.
    pub analysis: Option<CargoBuildAnalysis>,
    /// Unstable feature `-Zchecksum-freshness`.
    pub fingerprint: Option<FingerprintMethod>,
    pub build_std: Option<BuildStdConfig>,
}

/// Metrics collection for build analysis.
#[cfg_attr(feature = "unstable-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub struct CargoBuildAnalysis {
    pub enabled: bool,
}

/// Whether warnings should warn, be allowed, or cause an error.
#[cfg_attr(feature = "unstable-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum WarningHandling {
    #[default]
    /// Output warnings.
    Warn,
    /// Allow warnings (do not output them).
    Allow,
    /// Error if  warnings are emitted.
    Deny,
}

#[cfg_attr(feature = "unstable-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Deserialize, Serialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum FingerprintMethod {
    #[default]
    Mtime,
    Content,
}

impl FingerprintMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Mtime => "mtime",
            Self::Content => "content",
        }
    }
}

impl std::fmt::Display for FingerprintMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.as_str().fmt(f)
    }
}

/// Configuration for `build.build-std`.
#[cfg_attr(feature = "unstable-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct BuildStdConfig {
    pub when: Option<BuildStdWhen>,
    pub crates: Option<BuildStdCrates>,
}

/// When to build the standard library.
#[cfg_attr(feature = "unstable-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum BuildStdWhen {
    Never,
    Always,
}

/// Which standard library crates to build.
/// Valid values are "core", "alloc", "std", or an array of crate names
#[cfg_attr(feature = "unstable-schema", derive(schemars::JsonSchema))]
#[derive(Debug, PartialEq, Eq)]
pub enum BuildStdCrates {
    Core,
    Alloc,
    Std,
    // Custom list of crates - permanently unstable
    List(Vec<String>),
}

impl<'de> Deserialize<'de> for BuildStdCrates {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        UntaggedEnumVisitor::new()
            .string(|s| match s {
                "core" => Ok(Self::Core),
                "alloc" => Ok(Self::Alloc),
                "std" => Ok(Self::Std),
                _ => Err(serde::de::Error::unknown_variant(
                    s,
                    &["core", "alloc", "std"],
                )),
            })
            .seq(|seq| seq.deserialize().map(Self::List))
            .deserialize(deserializer)
    }
}

/// Configuration for `build.target`.
///
/// Accepts in the following forms:
///
/// ```toml
/// target = "a"
/// target = ["a"]
/// target = ["a", "b"]
/// ```
#[cfg_attr(feature = "unstable-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Deserialize)]
#[serde(transparent)]
pub struct BuildTargetConfig {
    inner: Value<BuildTargetConfigInner>,
}

#[cfg_attr(feature = "unstable-schema", derive(schemars::JsonSchema))]
#[derive(Debug)]
enum BuildTargetConfigInner {
    One(String),
    Many(Vec<String>),
}

impl<'de> Deserialize<'de> for BuildTargetConfigInner {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        UntaggedEnumVisitor::new()
            .string(|one| Ok(BuildTargetConfigInner::One(one.to_owned())))
            .seq(|many| many.deserialize().map(BuildTargetConfigInner::Many))
            .deserialize(deserializer)
    }
}

impl BuildTargetConfig {
    /// Gets values of `build.target` as a list of strings.
    pub fn values(&self, cwd: &Path) -> Result<Vec<String>> {
        let map = |s: &String| {
            if s.ends_with(".json") {
                // Path to a target specification file (in JSON).
                // <https://doc.rust-lang.org/rustc/targets/custom.html>
                self.inner
                    .definition
                    .root(cwd)
                    .join(s)
                    .to_str()
                    .expect("must be utf-8 in toml")
                    .to_string()
            } else {
                // A string. Probably a target tuple.
                s.to_string()
            }
        };
        let values = match &self.inner.val {
            BuildTargetConfigInner::One(s) => vec![map(s)],
            BuildTargetConfigInner::Many(v) => v.iter().map(map).collect(),
        };
        Ok(values)
    }
}

/// The `[resolver]` table.
///
/// Example configuration:
///
/// ```toml
/// [resolver]
/// incompatible-rust-versions = "fallback"
/// incompatible-publish-age = "deny"
/// feature-unification = "workspace"
/// lockfile-path = "my/Cargo.lock"
/// ```
#[cfg_attr(feature = "unstable-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct CargoResolverConfig {
    pub incompatible_rust_versions: Option<IncompatibleRustVersions>,
    pub incompatible_publish_age: Option<IncompatiblePublishAge>,
    pub feature_unification: Option<FeatureUnification>,
    pub lockfile_path: Option<ConfigRelativePath>,
}

#[cfg_attr(feature = "unstable-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum IncompatibleRustVersions {
    Allow,
    Fallback,
}

#[cfg_attr(feature = "unstable-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum IncompatiblePublishAge {
    Allow,
    Deny,
}

#[cfg_attr(feature = "unstable-schema", derive(schemars::JsonSchema))]
#[derive(Copy, Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FeatureUnification {
    Package,
    Selected,
    Workspace,
}

/// The `[term]` table.
///
/// Example configuration:
///
/// ```toml
/// [term]
/// verbose = false
/// quiet = false
/// color = "auto"
/// progress.when = "auto"
/// ```
#[cfg_attr(feature = "unstable-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub struct TermConfig {
    pub verbose: Option<bool>,
    pub quiet: Option<bool>,
    pub color: Option<String>,
    pub hyperlinks: Option<bool>,
    pub unicode: Option<bool>,
    pub progress: Option<ProgressConfig>,
}

/// The `term.progress` configuration.
///
/// Example configuration:
///
/// ```toml
/// [term]
/// progress.when = "never" # or "auto"
/// ```
///
/// ```toml
/// # `when = "always"` requires a `width` field
/// [term]
/// progress = { when = "always", width = 80 }
/// ```
#[cfg_attr(feature = "unstable-schema", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "unstable-schema", schemars(rename_all = "kebab-case"))]
#[derive(Debug, Default)]
pub struct ProgressConfig {
    pub when: ProgressWhen,
    pub width: Option<usize>,
    /// Communicate progress status with a terminal
    pub term_integration: Option<bool>,
}

#[cfg_attr(feature = "unstable-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProgressWhen {
    #[default]
    Auto,
    Never,
    Always,
}

// We need this custom deserialization for validadting the rule of
// `when = "always"` requiring a `width` field.
impl<'de> Deserialize<'de> for ProgressConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(rename_all = "kebab-case")]
        struct ProgressConfigInner {
            #[serde(default)]
            when: ProgressWhen,
            width: Option<usize>,
            term_integration: Option<bool>,
        }

        let pc = ProgressConfigInner::deserialize(deserializer)?;
        if let ProgressConfigInner {
            when: ProgressWhen::Always,
            width: None,
            ..
        } = pc
        {
            return Err(serde::de::Error::custom(
                "\"always\" progress requires a `width` key",
            ));
        }
        Ok(ProgressConfig {
            when: pc.when,
            width: pc.width,
            term_integration: pc.term_integration,
        })
    }
}

#[cfg_attr(feature = "unstable-schema", derive(schemars::JsonSchema))]
#[derive(Debug)]
enum EnvConfigValueInner {
    Simple(String),
    WithOptions {
        value: ConfigRelativePath,
        force: bool,
        relative: bool,
    },
}

impl<'de> Deserialize<'de> for EnvConfigValueInner {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct WithOptions {
            value: ConfigRelativePath,
            #[serde(default)]
            force: bool,
            #[serde(default)]
            relative: bool,
        }

        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = EnvConfigValueInner;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a string or map")
            }

            fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
                Ok(EnvConfigValueInner::Simple(value.to_owned()))
            }

            fn visit_string<E: de::Error>(self, value: String) -> Result<Self::Value, E> {
                Ok(EnvConfigValueInner::Simple(value))
            }

            fn visit_map<M: de::MapAccess<'de>>(self, map: M) -> Result<Self::Value, M::Error> {
                let options = WithOptions::deserialize(de::value::MapAccessDeserializer::new(map))?;
                Ok(EnvConfigValueInner::WithOptions {
                    value: options.value,
                    force: options.force,
                    relative: options.relative,
                })
            }
        }

        deserializer.deserialize_any(Visitor)
    }
}

/// Configuration value for environment variables in `[env]` section.
///
/// Supports two formats: simple string and with options.
///
/// ```toml
/// [env]
/// FOO = "value"
/// ```
///
/// ```toml
/// [env]
/// BAR = { value = "relative/path", relative = true }
/// BAZ = { value = "override", force = true }
/// ```
#[cfg_attr(feature = "unstable-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Deserialize)]
#[serde(transparent)]
pub struct EnvConfigValue {
    inner: EnvConfigValueInner,
}

impl EnvConfigValue {
    /// Whether this value should override existing environment variables.
    pub fn is_force(&self) -> bool {
        match self.inner {
            EnvConfigValueInner::Simple(_) => false,
            EnvConfigValueInner::WithOptions { force, .. } => force,
        }
    }

    /// Resolves the environment variable value.
    ///
    /// If `relative = true`,
    /// the value is interpreted as a [`ConfigRelativePath`]-like path.
    pub fn resolve<'a>(&'a self, cwd: &Path) -> Cow<'a, OsStr> {
        match self.inner {
            EnvConfigValueInner::Simple(ref s) => Cow::Borrowed(OsStr::new(s.as_str())),
            EnvConfigValueInner::WithOptions {
                ref value,
                relative,
                ..
            } => {
                if relative {
                    let p = value.value().definition.root(cwd).join(value.raw_value());
                    Cow::Owned(p.into_os_string())
                } else {
                    Cow::Borrowed(OsStr::new(value.raw_value()))
                }
            }
        }
    }
}

pub type EnvConfig = HashMap<String, EnvConfigValue>;

/// Credentials for a registry.
#[cfg_attr(feature = "unstable-schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct RegistryCredential {
    #[cfg_attr(feature = "unstable-schema", schemars(with = "Option<String>"))]
    pub token: OptValue<Secret<String>>,
}

/// `[registries.NAME]` tables.
///
/// The values here should be kept in sync with `GlobalRegistryConfig`
#[cfg_attr(feature = "unstable-schema", derive(schemars::JsonSchema))]
#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "kebab-case")]
pub struct RegistryConfig {
    pub index: Option<String>,
    #[deprecated = "see `RegistryCredential::token` instead"]
    #[cfg_attr(feature = "unstable-schema", schemars(with = "Option<String>"))]
    pub token: OptValue<Secret<String>>,
    pub credential_provider: Option<PathAndArgs>,
    /// Minimum publish age threshold for RFC 3923
    pub min_publish_age: Option<String>,
    #[serde(rename = "protocol")]
    _protocol: Option<String>,
}

/// The `[registry]` table, which has more keys than the `[registries.NAME]` tables.
///
/// Note: nesting `RegistryConfig` inside this struct and using `serde(flatten)` *should* work
/// but fails with "invalid type: sequence, expected a value" when attempting to deserialize.
#[cfg_attr(feature = "unstable-schema", derive(schemars::JsonSchema))]
#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct GlobalRegistryConfig {
    pub index: Option<String>,
    #[deprecated = "see `RegistryCredential::token` instead"]
    #[cfg_attr(feature = "unstable-schema", schemars(with = "Option<String>"))]
    pub token: OptValue<Secret<String>>,
    pub credential_provider: Option<PathAndArgs>,
    /// Global default Minimum publish age threshold for RFC 3923
    pub global_min_publish_age: Option<String>,
    #[serde(rename = "default")]
    _default: Option<String>,
    #[serde(rename = "global-credential-providers")]
    _global_credential_providers: Option<Vec<String>>,
}

impl GlobalRegistryConfig {
    pub fn to_registry_config(self) -> RegistryConfig {
        RegistryConfig {
            index: self.index,
            #[expect(deprecated)]
            token: self.token,
            credential_provider: self.credential_provider,
            // `min-publish-age` is per-registry config only.
            min_publish_age: None,
            _protocol: None,
        }
    }
}

/// Use with the `get` API to fetch a string that will be converted to a
/// `PathBuf`. Relative paths are converted to absolute paths based on the
/// location of the config file.
#[cfg_attr(feature = "unstable-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Deserialize, PartialEq, Clone)]
#[serde(transparent)]
pub struct ConfigRelativePath(Value<String>);

impl ConfigRelativePath {
    pub fn new(path: Value<String>) -> ConfigRelativePath {
        ConfigRelativePath(path)
    }

    /// Returns the underlying value.
    pub fn value(&self) -> &Value<String> {
        &self.0
    }

    /// Returns the raw underlying configuration value for this key.
    pub fn raw_value(&self) -> &str {
        &self.0.val
    }
}

/// A config type that is a program to run.
///
/// This supports a list of strings like `['/path/to/program', 'somearg']`
/// or a space separated string like `'/path/to/program somearg'`.
///
/// This expects the first value to be the path to the program to run.
/// Subsequent values are strings of arguments to pass to the program.
///
/// Typically you should use `ConfigRelativePathResolveExt::resolve_program` on the path
/// to get the actual program.
///
/// **Note**: Any usage of this type in config needs to be listed in
/// the `NON_MERGEABLE_LISTS` check to prevent list merging
/// from multiple config files.
#[cfg_attr(feature = "unstable-schema", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "unstable-schema", schemars(with = "StringList"))]
#[derive(Debug, Clone, PartialEq)]
pub struct PathAndArgs {
    pub path: ConfigRelativePath,
    pub args: Vec<String>,
}

impl<'de> serde::Deserialize<'de> for PathAndArgs {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let vsl = Value::<StringList>::deserialize(deserializer)?;
        let mut strings = vsl.val.0;
        if strings.is_empty() {
            return Err(D::Error::invalid_length(0, &"at least one element"));
        }
        let first = strings.remove(0);
        let crp = Value {
            val: first,
            definition: vsl.definition,
        };
        Ok(PathAndArgs {
            path: ConfigRelativePath(crp),
            args: strings,
        })
    }
}

impl PathAndArgs {
    /// Construct a `PathAndArgs` from a string. The string will be split on ascii whitespace,
    /// with the first item being treated as a `ConfigRelativePath` to the executable, and subsequent
    /// items as arguments.
    pub fn from_whitespace_separated_string(p: &Value<String>) -> PathAndArgs {
        let mut iter = p.val.split_ascii_whitespace().map(str::to_string);
        let val = iter.next().unwrap_or_default();
        let args = iter.collect();
        let crp = Value {
            val,
            definition: p.definition.clone(),
        };
        PathAndArgs {
            path: ConfigRelativePath(crp),
            args,
        }
    }
}

/// A type to deserialize a list of strings from a toml file.
///
/// Supports deserializing either a whitespace-separated list of arguments in a
/// single string or a string list itself. For example these deserialize to
/// equivalent values:
///
/// ```toml
/// a = 'a b c'
/// b = ['a', 'b', 'c']
/// ```
#[cfg_attr(feature = "unstable-schema", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "unstable-schema", schemars(extend("type" = ["array", "string"])))]
#[derive(Debug, Deserialize, Clone)]
pub struct StringList(Vec<String>);

impl StringList {
    pub fn as_slice(&self) -> &[String] {
        &self.0
    }
}

#[cfg(feature = "unstable-schema")]
#[test]
fn dump_config_schema() {
    let schema = schemars::schema_for!(Config);
    let dump = serde_json::to_string_pretty(&schema).unwrap();
    snapbox::assert_data_eq!(dump, snapbox::file!("../config.schema.json").raw());
}

#[cfg(feature = "unstable-schema")]
#[test]
fn dump_credentials_schema() {
    let schema = schemars::schema_for!(Credentials);
    let dump = serde_json::to_string_pretty(&schema).unwrap();
    snapbox::assert_data_eq!(dump, snapbox::file!("../credentials.schema.json").raw());
}
