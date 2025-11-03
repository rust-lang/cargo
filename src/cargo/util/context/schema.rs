//! Cargo configuration schemas.
//!
//! This module contains types that define the schema for various configuration
//! sections found in Cargo configuration.
//!
//! These types are mostly used by [`GlobalContext::get`](super::GlobalContext::get)
//! to deserialize configuration values from TOML files, environment variables,
//! and CLI arguments.
//!
//! Schema types here should only contain data and simple accessor methods.
//! Avoid depending on [`GlobalContext`](super::GlobalContext) directly.

use std::borrow::Cow;
use std::collections::HashMap;
use std::ffi::OsStr;

use serde::Deserialize;
use serde_untagged::UntaggedEnumVisitor;

use std::path::Path;

use crate::CargoResult;

use super::StringList;
use super::Value;
use super::path::ConfigRelativePath;

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
#[derive(Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub struct CargoFutureIncompatConfig {
    frequency: Option<CargoFutureIncompatFrequencyConfig>,
}

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
        UntaggedEnumVisitor::new()
            .string(|single| Ok(SslVersionConfig::Single(single.to_owned())))
            .map(|map| map.deserialize().map(SslVersionConfig::Range))
            .deserialize(deserializer)
    }
}

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
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct CargoNetConfig {
    pub retry: Option<u32>,
    pub offline: Option<bool>,
    pub git_fetch_with_cli: Option<bool>,
    pub ssh: Option<CargoSshConfig>,
}

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
    pub jobs: Option<JobsConfig>,
    pub rustflags: Option<StringList>,
    pub rustdocflags: Option<StringList>,
    pub rustc_wrapper: Option<ConfigRelativePath>,
    pub rustc_workspace_wrapper: Option<ConfigRelativePath>,
    pub rustc: Option<ConfigRelativePath>,
    pub rustdoc: Option<ConfigRelativePath>,
    // deprecated alias for artifact-dir
    pub out_dir: Option<ConfigRelativePath>,
    pub artifact_dir: Option<ConfigRelativePath>,
    pub warnings: Option<WarningHandling>,
    /// Unstable feature `-Zsbom`.
    pub sbom: Option<bool>,
    /// Unstable feature `-Zbuild-analysis`.
    pub analysis: Option<CargoBuildAnalysis>,
}

/// Metrics collection for build analysis.
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub struct CargoBuildAnalysis {
    pub enabled: bool,
}

/// Whether warnings should warn, be allowed, or cause an error.
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

/// Configuration for `build.target`.
///
/// Accepts in the following forms:
///
/// ```toml
/// target = "a"
/// target = ["a"]
/// target = ["a", "b"]
/// ```
#[derive(Debug, Deserialize)]
#[serde(transparent)]
pub struct BuildTargetConfig {
    inner: Value<BuildTargetConfigInner>,
}

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
    pub fn values(&self, cwd: &Path) -> CargoResult<Vec<String>> {
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
                // A string. Probably a target triple.
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
/// feature-unification = "workspace"
/// ```
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct CargoResolverConfig {
    pub incompatible_rust_versions: Option<IncompatibleRustVersions>,
    pub feature_unification: Option<FeatureUnification>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum IncompatibleRustVersions {
    Allow,
    Fallback,
}

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
#[derive(Debug, Default)]
pub struct ProgressConfig {
    pub when: ProgressWhen,
    pub width: Option<usize>,
    /// Communicate progress status with a terminal
    pub term_integration: Option<bool>,
}

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

#[derive(Debug)]
enum EnvConfigValueInner {
    Simple(String),
    WithOptions {
        value: String,
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
            value: String,
            #[serde(default)]
            force: bool,
            #[serde(default)]
            relative: bool,
        }

        UntaggedEnumVisitor::new()
            .string(|simple| Ok(EnvConfigValueInner::Simple(simple.to_owned())))
            .map(|map| {
                let with_options: WithOptions = map.deserialize()?;
                Ok(EnvConfigValueInner::WithOptions {
                    value: with_options.value,
                    force: with_options.force,
                    relative: with_options.relative,
                })
            })
            .deserialize(deserializer)
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
#[derive(Debug, Deserialize)]
#[serde(transparent)]
pub struct EnvConfigValue {
    inner: Value<EnvConfigValueInner>,
}

impl EnvConfigValue {
    /// Whether this value should override existing environment variables.
    pub fn is_force(&self) -> bool {
        match self.inner.val {
            EnvConfigValueInner::Simple(_) => false,
            EnvConfigValueInner::WithOptions { force, .. } => force,
        }
    }

    /// Resolves the environment variable value.
    ///
    /// If `relative = true`,
    /// the value is interpreted as a [`ConfigRelativePath`]-like path.
    pub fn resolve<'a>(&'a self, cwd: &Path) -> Cow<'a, OsStr> {
        match self.inner.val {
            EnvConfigValueInner::Simple(ref s) => Cow::Borrowed(OsStr::new(s.as_str())),
            EnvConfigValueInner::WithOptions {
                ref value,
                relative,
                ..
            } => {
                if relative {
                    let p = self.inner.definition.root(cwd).join(&value);
                    Cow::Owned(p.into_os_string())
                } else {
                    Cow::Borrowed(OsStr::new(value.as_str()))
                }
            }
        }
    }
}

pub type EnvConfig = HashMap<String, EnvConfigValue>;
