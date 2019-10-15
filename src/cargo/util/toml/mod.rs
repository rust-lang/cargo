use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::str;

use cargo_platform::Platform;
use failure::bail;
use log::{debug, trace};
use semver::{self, VersionReq};
use serde::de;
use serde::ser;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::core::dependency::Kind;
use crate::core::manifest::{LibKind, ManifestMetadata, TargetSourcePath, Warnings};
use crate::core::profiles::Profiles;
use crate::core::{Dependency, InternedString, Manifest, PackageId, Summary, Target};
use crate::core::{Edition, EitherManifest, Feature, Features, VirtualManifest};
use crate::core::{GitReference, PackageIdSpec, SourceId, WorkspaceConfig, WorkspaceRootConfig};
use crate::sources::{CRATES_IO_INDEX, CRATES_IO_REGISTRY};
use crate::util::errors::{CargoResult, CargoResultExt, ManifestError};
use crate::util::{self, paths, validate_package_name, Config, IntoUrl};

mod targets;
use self::targets::targets;

pub fn read_manifest(
    path: &Path,
    source_id: SourceId,
    config: &Config,
) -> Result<(EitherManifest, Vec<PathBuf>), ManifestError> {
    trace!(
        "read_manifest; path={}; source-id={}",
        path.display(),
        source_id
    );
    let contents = paths::read(path).map_err(|err| ManifestError::new(err, path.into()))?;

    do_read_manifest(&contents, path, source_id, config)
        .chain_err(|| format!("failed to parse manifest at `{}`", path.display()))
        .map_err(|err| ManifestError::new(err, path.into()))
}

fn do_read_manifest(
    contents: &str,
    manifest_file: &Path,
    source_id: SourceId,
    config: &Config,
) -> CargoResult<(EitherManifest, Vec<PathBuf>)> {
    let package_root = manifest_file.parent().unwrap();

    let toml = {
        let pretty_filename = manifest_file
            .strip_prefix(config.cwd())
            .unwrap_or(manifest_file);
        parse(contents, pretty_filename, config)?
    };

    let mut unused = BTreeSet::new();
    let manifest: TomlManifest = serde_ignored::deserialize(toml, |path| {
        let mut key = String::new();
        stringify(&mut key, &path);
        unused.insert(key);
    })?;
    let add_unused = |warnings: &mut Warnings| {
        for key in unused {
            warnings.add_warning(format!("unused manifest key: {}", key));
            if key == "profiles.debug" {
                warnings.add_warning("use `[profile.dev]` to configure debug builds".to_string());
            }
        }
    };

    let manifest = Rc::new(manifest);
    return if manifest.project.is_some() || manifest.package.is_some() {
        let (mut manifest, paths) =
            TomlManifest::to_real_manifest(&manifest, source_id, package_root, config)?;
        add_unused(manifest.warnings_mut());
        if !manifest.targets().iter().any(|t| !t.is_custom_build()) {
            bail!(
                "no targets specified in the manifest\n  \
                 either src/lib.rs, src/main.rs, a [lib] section, or \
                 [[bin]] section must be present"
            )
        }
        Ok((EitherManifest::Real(manifest), paths))
    } else {
        let (mut m, paths) =
            TomlManifest::to_virtual_manifest(&manifest, source_id, package_root, config)?;
        add_unused(m.warnings_mut());
        Ok((EitherManifest::Virtual(m), paths))
    };

    fn stringify(dst: &mut String, path: &serde_ignored::Path<'_>) {
        use serde_ignored::Path;

        match *path {
            Path::Root => {}
            Path::Seq { parent, index } => {
                stringify(dst, parent);
                if !dst.is_empty() {
                    dst.push('.');
                }
                dst.push_str(&index.to_string());
            }
            Path::Map { parent, ref key } => {
                stringify(dst, parent);
                if !dst.is_empty() {
                    dst.push('.');
                }
                dst.push_str(key);
            }
            Path::Some { parent }
            | Path::NewtypeVariant { parent }
            | Path::NewtypeStruct { parent } => stringify(dst, parent),
        }
    }
}

pub fn parse(toml: &str, file: &Path, config: &Config) -> CargoResult<toml::Value> {
    let first_error = match toml.parse() {
        Ok(ret) => return Ok(ret),
        Err(e) => e,
    };

    let mut second_parser = toml::de::Deserializer::new(toml);
    second_parser.set_require_newline_after_table(false);
    if let Ok(ret) = toml::Value::deserialize(&mut second_parser) {
        let msg = format!(
            "\
TOML file found which contains invalid syntax and will soon not parse
at `{}`.

The TOML spec requires newlines after table definitions (e.g., `[a] b = 1` is
invalid), but this file has a table header which does not have a newline after
it. A newline needs to be added and this warning will soon become a hard error
in the future.",
            file.display()
        );
        config.shell().warn(&msg)?;
        return Ok(ret);
    }

    let mut third_parser = toml::de::Deserializer::new(toml);
    third_parser.set_allow_duplicate_after_longer_table(true);
    if let Ok(ret) = toml::Value::deserialize(&mut third_parser) {
        let msg = format!(
            "\
TOML file found which contains invalid syntax and will soon not parse
at `{}`.

The TOML spec requires that each table header is defined at most once, but
historical versions of Cargo have erroneously accepted this file. The table
definitions will need to be merged together with one table header to proceed,
and this will become a hard error in the future.",
            file.display()
        );
        config.shell().warn(&msg)?;
        return Ok(ret);
    }

    let first_error = failure::Error::from(first_error);
    Err(first_error.context("could not parse input as TOML").into())
}

type TomlLibTarget = TomlTarget;
type TomlBinTarget = TomlTarget;
type TomlExampleTarget = TomlTarget;
type TomlTestTarget = TomlTarget;
type TomlBenchTarget = TomlTarget;

#[derive(Clone, Debug, Serialize)]
#[serde(untagged)]
pub enum TomlDependency {
    Simple(String),
    Detailed(DetailedTomlDependency),
}

impl<'de> de::Deserialize<'de> for TomlDependency {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        struct TomlDependencyVisitor;

        impl<'de> de::Visitor<'de> for TomlDependencyVisitor {
            type Value = TomlDependency;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(
                    "a version string like \"0.9.8\" or a \
                     detailed dependency like { version = \"0.9.8\" }",
                )
            }

            fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(TomlDependency::Simple(s.to_owned()))
            }

            fn visit_map<V>(self, map: V) -> Result<Self::Value, V::Error>
            where
                V: de::MapAccess<'de>,
            {
                let mvd = de::value::MapAccessDeserializer::new(map);
                DetailedTomlDependency::deserialize(mvd).map(TomlDependency::Detailed)
            }
        }

        deserializer.deserialize_any(TomlDependencyVisitor)
    }
}

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
#[serde(rename_all = "kebab-case")]
pub struct DetailedTomlDependency {
    version: Option<String>,
    registry: Option<String>,
    /// The URL of the `registry` field.
    /// This is an internal implementation detail. When Cargo creates a
    /// package, it replaces `registry` with `registry-index` so that the
    /// manifest contains the correct URL. All users won't have the same
    /// registry names configured, so Cargo can't rely on just the name for
    /// crates published by other users.
    registry_index: Option<String>,
    path: Option<String>,
    git: Option<String>,
    branch: Option<String>,
    tag: Option<String>,
    rev: Option<String>,
    features: Option<Vec<String>>,
    optional: Option<bool>,
    default_features: Option<bool>,
    #[serde(rename = "default_features")]
    default_features2: Option<bool>,
    package: Option<String>,
    public: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct TomlManifest {
    cargo_features: Option<Vec<String>>,
    package: Option<Box<TomlProject>>,
    project: Option<Box<TomlProject>>,
    profile: Option<TomlProfiles>,
    lib: Option<TomlLibTarget>,
    bin: Option<Vec<TomlBinTarget>>,
    example: Option<Vec<TomlExampleTarget>>,
    test: Option<Vec<TomlTestTarget>>,
    bench: Option<Vec<TomlTestTarget>>,
    dependencies: Option<BTreeMap<String, TomlDependency>>,
    dev_dependencies: Option<BTreeMap<String, TomlDependency>>,
    #[serde(rename = "dev_dependencies")]
    dev_dependencies2: Option<BTreeMap<String, TomlDependency>>,
    build_dependencies: Option<BTreeMap<String, TomlDependency>>,
    #[serde(rename = "build_dependencies")]
    build_dependencies2: Option<BTreeMap<String, TomlDependency>>,
    features: Option<BTreeMap<String, Vec<String>>>,
    target: Option<BTreeMap<String, TomlPlatform>>,
    replace: Option<BTreeMap<String, TomlDependency>>,
    patch: Option<BTreeMap<String, BTreeMap<String, TomlDependency>>>,
    workspace: Option<TomlWorkspace>,
    badges: Option<BTreeMap<String, BTreeMap<String, String>>>,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
pub struct TomlProfiles(BTreeMap<String, TomlProfile>);

impl TomlProfiles {
    pub fn get_all(&self) -> &BTreeMap<String, TomlProfile> {
        &self.0
    }

    pub fn get(&self, name: &'static str) -> Option<&TomlProfile> {
        self.0.get(&String::from(name))
    }

    pub fn validate(&self, features: &Features, warnings: &mut Vec<String>) -> CargoResult<()> {
        for (name, profile) in &self.0 {
            if name == "debug" {
                warnings.push("use `[profile.dev]` to configure debug builds".to_string());
            }

            profile.validate(name, features, warnings)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TomlOptLevel(pub String);

impl<'de> de::Deserialize<'de> for TomlOptLevel {
    fn deserialize<D>(d: D) -> Result<TomlOptLevel, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = TomlOptLevel;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("an optimization level")
            }

            fn visit_i64<E>(self, value: i64) -> Result<TomlOptLevel, E>
            where
                E: de::Error,
            {
                Ok(TomlOptLevel(value.to_string()))
            }

            fn visit_str<E>(self, value: &str) -> Result<TomlOptLevel, E>
            where
                E: de::Error,
            {
                if value == "s" || value == "z" {
                    Ok(TomlOptLevel(value.to_string()))
                } else {
                    Err(E::custom(format!(
                        "must be an integer, `z`, or `s`, \
                         but found: {}",
                        value
                    )))
                }
            }
        }

        d.deserialize_any(Visitor)
    }
}

impl ser::Serialize for TomlOptLevel {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        match self.0.parse::<u32>() {
            Ok(n) => n.serialize(serializer),
            Err(_) => self.0.serialize(serializer),
        }
    }
}

#[derive(Clone, Debug, Serialize, Eq, PartialEq)]
#[serde(untagged)]
pub enum U32OrBool {
    U32(u32),
    Bool(bool),
}

impl<'de> de::Deserialize<'de> for U32OrBool {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = U32OrBool;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a boolean or an integer")
            }

            fn visit_bool<E>(self, b: bool) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(U32OrBool::Bool(b))
            }

            fn visit_i64<E>(self, u: i64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(U32OrBool::U32(u as u32))
            }

            fn visit_u64<E>(self, u: u64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(U32OrBool::U32(u as u32))
            }
        }

        deserializer.deserialize_any(Visitor)
    }
}

#[derive(Deserialize, Serialize, Clone, Debug, Default, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub struct TomlProfile {
    pub opt_level: Option<TomlOptLevel>,
    pub lto: Option<StringOrBool>,
    pub codegen_units: Option<u32>,
    pub debug: Option<U32OrBool>,
    pub debug_assertions: Option<bool>,
    pub rpath: Option<bool>,
    pub panic: Option<String>,
    pub overflow_checks: Option<bool>,
    pub incremental: Option<bool>,
    // `overrides` has been renamed to `package`, this should be removed when
    // stabilized.
    pub overrides: Option<BTreeMap<ProfilePackageSpec, TomlProfile>>,
    pub package: Option<BTreeMap<ProfilePackageSpec, TomlProfile>>,
    pub build_override: Option<Box<TomlProfile>>,
    pub dir_name: Option<String>,
    pub inherits: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Ord, PartialOrd, Hash)]
pub enum ProfilePackageSpec {
    Spec(PackageIdSpec),
    All,
}

impl ser::Serialize for ProfilePackageSpec {
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        match *self {
            ProfilePackageSpec::Spec(ref spec) => spec.serialize(s),
            ProfilePackageSpec::All => "*".serialize(s),
        }
    }
}

impl<'de> de::Deserialize<'de> for ProfilePackageSpec {
    fn deserialize<D>(d: D) -> Result<ProfilePackageSpec, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        let string = String::deserialize(d)?;
        if string == "*" {
            Ok(ProfilePackageSpec::All)
        } else {
            PackageIdSpec::parse(&string)
                .map_err(de::Error::custom)
                .map(ProfilePackageSpec::Spec)
        }
    }
}

impl TomlProfile {
    pub fn validate(
        &self,
        name: &str,
        features: &Features,
        warnings: &mut Vec<String>,
    ) -> CargoResult<()> {
        if let Some(ref profile) = self.build_override {
            features.require(Feature::profile_overrides())?;
            profile.validate_override("build-override")?;
        }
        if let Some(ref override_map) = self.overrides {
            warnings.push(
                "profile key `overrides` has been renamed to `package`, \
                 please update the manifest to the new key name"
                    .to_string(),
            );
            features.require(Feature::profile_overrides())?;
            for profile in override_map.values() {
                profile.validate_override("package")?;
            }
        }
        if let Some(ref packages) = self.package {
            features.require(Feature::profile_overrides())?;
            for profile in packages.values() {
                profile.validate_override("package")?;
            }
        }

        // Feature gate definition of named profiles
        match name {
            "dev" | "release" | "bench" | "test" | "doc" => {}
            _ => {
                features.require(Feature::named_profiles())?;
            }
        }

        // Profile name validation
        Self::validate_name(name, "profile name")?;

        // Feature gate on uses of keys related to named profiles
        if self.inherits.is_some() {
            features.require(Feature::named_profiles())?;
        }

        if self.dir_name.is_some() {
            features.require(Feature::named_profiles())?;
        }

        // `dir-name` validation
        match &self.dir_name {
            None => {}
            Some(dir_name) => {
                Self::validate_name(dir_name, "dir-name")?;
            }
        }

        // `inherits` validation
        match &self.inherits {
            None => {}
            Some(inherits) => {
                Self::validate_name(inherits, "inherits")?;
            }
        }

        match name {
            "doc" => {
                warnings.push("profile `doc` is deprecated and has no effect".to_string());
            }
            "test" | "bench" => {
                if self.panic.is_some() {
                    warnings.push(format!("`panic` setting is ignored for `{}` profile", name))
                }
            }
            _ => {}
        }

        if let Some(panic) = &self.panic {
            if panic != "unwind" && panic != "abort" {
                bail!(
                    "`panic` setting of `{}` is not a valid setting,\
                     must be `unwind` or `abort`",
                    panic
                );
            }
        }
        Ok(())
    }

    /// Validate dir-names and profile names according to RFC 2678.
    pub fn validate_name(name: &str, what: &str) -> CargoResult<()> {
        if let Some(ch) = name
            .chars()
            .find(|ch| !ch.is_alphanumeric() && *ch != '_' && *ch != '-')
        {
            failure::bail!("Invalid character `{}` in {}: `{}`", ch, what, name);
        }

        match name {
            "package" | "build" => {
                failure::bail!("Invalid {}: `{}`", what, name);
            }
            "debug" if what == "profile" => {
                if what == "profile name" {
                    // Allowed, but will emit warnings
                } else {
                    failure::bail!("Invalid {}: `{}`", what, name);
                }
            }
            "doc" if what == "dir-name" => {
                failure::bail!("Invalid {}: `{}`", what, name);
            }
            _ => {}
        }

        Ok(())
    }

    fn validate_override(&self, which: &str) -> CargoResult<()> {
        if self.overrides.is_some() || self.package.is_some() {
            bail!("package-specific profiles cannot be nested");
        }
        if self.build_override.is_some() {
            bail!("build-override profiles cannot be nested");
        }
        if self.panic.is_some() {
            bail!("`panic` may not be specified in a `{}` profile", which)
        }
        if self.lto.is_some() {
            bail!("`lto` may not be specified in a `{}` profile", which)
        }
        if self.rpath.is_some() {
            bail!("`rpath` may not be specified in a `{}` profile", which)
        }
        Ok(())
    }

    pub fn merge(&mut self, profile: &TomlProfile) {
        if let Some(v) = &profile.opt_level {
            self.opt_level = Some(v.clone());
        }

        if let Some(v) = &profile.lto {
            self.lto = Some(v.clone());
        }

        if let Some(v) = profile.codegen_units {
            self.codegen_units = Some(v);
        }

        if let Some(v) = &profile.debug {
            self.debug = Some(v.clone());
        }

        if let Some(v) = profile.debug_assertions {
            self.debug_assertions = Some(v);
        }

        if let Some(v) = profile.rpath {
            self.rpath = Some(v);
        }

        if let Some(v) = &profile.panic {
            self.panic = Some(v.clone());
        }

        if let Some(v) = profile.overflow_checks {
            self.overflow_checks = Some(v);
        }

        if let Some(v) = profile.incremental {
            self.incremental = Some(v);
        }

        if let Some(v) = &profile.overrides {
            self.overrides = Some(v.clone());
        }

        if let Some(v) = &profile.package {
            self.package = Some(v.clone());
        }

        if let Some(v) = &profile.build_override {
            self.build_override = Some(v.clone());
        }

        if let Some(v) = &profile.inherits {
            self.inherits = Some(v.clone());
        }

        if let Some(v) = &profile.dir_name {
            self.dir_name = Some(v.clone());
        }
    }
}

#[derive(Clone, Debug, Serialize, Eq, PartialEq)]
pub struct StringOrVec(Vec<String>);

impl<'de> de::Deserialize<'de> for StringOrVec {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = StringOrVec;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("string or list of strings")
            }

            fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(StringOrVec(vec![s.to_string()]))
            }

            fn visit_seq<V>(self, v: V) -> Result<Self::Value, V::Error>
            where
                V: de::SeqAccess<'de>,
            {
                let seq = de::value::SeqAccessDeserializer::new(v);
                Vec::deserialize(seq).map(StringOrVec)
            }
        }

        deserializer.deserialize_any(Visitor)
    }
}

#[derive(Clone, Debug, Serialize, Eq, PartialEq)]
#[serde(untagged)]
pub enum StringOrBool {
    String(String),
    Bool(bool),
}

impl<'de> de::Deserialize<'de> for StringOrBool {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = StringOrBool;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a boolean or a string")
            }

            fn visit_bool<E>(self, b: bool) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(StringOrBool::Bool(b))
            }

            fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(StringOrBool::String(s.to_string()))
            }
        }

        deserializer.deserialize_any(Visitor)
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(untagged)]
pub enum VecStringOrBool {
    VecString(Vec<String>),
    Bool(bool),
}

impl<'de> de::Deserialize<'de> for VecStringOrBool {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = VecStringOrBool;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a boolean or vector of strings")
            }

            fn visit_seq<V>(self, v: V) -> Result<Self::Value, V::Error>
            where
                V: de::SeqAccess<'de>,
            {
                let seq = de::value::SeqAccessDeserializer::new(v);
                Vec::deserialize(seq).map(VecStringOrBool::VecString)
            }

            fn visit_bool<E>(self, b: bool) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(VecStringOrBool::Bool(b))
            }
        }

        deserializer.deserialize_any(Visitor)
    }
}

/// Represents the `package`/`project` sections of a `Cargo.toml`.
///
/// Note that the order of the fields matters, since this is the order they
/// are serialized to a TOML file. For example, you cannot have values after
/// the field `metadata`, since it is a table and values cannot appear after
/// tables.
#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct TomlProject {
    edition: Option<String>,
    name: InternedString,
    version: semver::Version,
    authors: Option<Vec<String>>,
    build: Option<StringOrBool>,
    metabuild: Option<StringOrVec>,
    links: Option<String>,
    exclude: Option<Vec<String>>,
    include: Option<Vec<String>>,
    publish: Option<VecStringOrBool>,
    #[serde(rename = "publish-lockfile")]
    publish_lockfile: Option<bool>,
    workspace: Option<String>,
    #[serde(rename = "im-a-teapot")]
    im_a_teapot: Option<bool>,
    autobins: Option<bool>,
    autoexamples: Option<bool>,
    autotests: Option<bool>,
    autobenches: Option<bool>,
    #[serde(rename = "namespaced-features")]
    namespaced_features: Option<bool>,
    #[serde(rename = "default-run")]
    default_run: Option<String>,

    // Package metadata.
    description: Option<String>,
    homepage: Option<String>,
    documentation: Option<String>,
    readme: Option<String>,
    keywords: Option<Vec<String>>,
    categories: Option<Vec<String>>,
    license: Option<String>,
    #[serde(rename = "license-file")]
    license_file: Option<String>,
    repository: Option<String>,
    metadata: Option<toml::Value>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TomlWorkspace {
    members: Option<Vec<String>>,
    #[serde(rename = "default-members")]
    default_members: Option<Vec<String>>,
    exclude: Option<Vec<String>>,
}

impl TomlProject {
    pub fn to_package_id(&self, source_id: SourceId) -> CargoResult<PackageId> {
        PackageId::new(self.name, self.version.clone(), source_id)
    }
}

struct Context<'a, 'b> {
    pkgid: Option<PackageId>,
    deps: &'a mut Vec<Dependency>,
    source_id: SourceId,
    nested_paths: &'a mut Vec<PathBuf>,
    config: &'b Config,
    warnings: &'a mut Vec<String>,
    platform: Option<Platform>,
    root: &'a Path,
    features: &'a Features,
}

impl TomlManifest {
    pub fn prepare_for_publish(&self, config: &Config) -> CargoResult<TomlManifest> {
        let mut package = self
            .package
            .as_ref()
            .or_else(|| self.project.as_ref())
            .unwrap()
            .clone();
        package.workspace = None;
        let all = |_d: &TomlDependency| true;
        return Ok(TomlManifest {
            package: Some(package),
            project: None,
            profile: self.profile.clone(),
            lib: self.lib.clone(),
            bin: self.bin.clone(),
            example: self.example.clone(),
            test: self.test.clone(),
            bench: self.bench.clone(),
            dependencies: map_deps(config, self.dependencies.as_ref(), all)?,
            dev_dependencies: map_deps(
                config,
                self.dev_dependencies
                    .as_ref()
                    .or_else(|| self.dev_dependencies2.as_ref()),
                TomlDependency::is_version_specified,
            )?,
            dev_dependencies2: None,
            build_dependencies: map_deps(
                config,
                self.build_dependencies
                    .as_ref()
                    .or_else(|| self.build_dependencies2.as_ref()),
                all,
            )?,
            build_dependencies2: None,
            features: self.features.clone(),
            target: match self.target.as_ref().map(|target_map| {
                target_map
                    .iter()
                    .map(|(k, v)| {
                        Ok((
                            k.clone(),
                            TomlPlatform {
                                dependencies: map_deps(config, v.dependencies.as_ref(), all)?,
                                dev_dependencies: map_deps(
                                    config,
                                    v.dev_dependencies
                                        .as_ref()
                                        .or_else(|| v.dev_dependencies2.as_ref()),
                                    TomlDependency::is_version_specified,
                                )?,
                                dev_dependencies2: None,
                                build_dependencies: map_deps(
                                    config,
                                    v.build_dependencies
                                        .as_ref()
                                        .or_else(|| v.build_dependencies2.as_ref()),
                                    all,
                                )?,
                                build_dependencies2: None,
                            },
                        ))
                    })
                    .collect()
            }) {
                Some(Ok(v)) => Some(v),
                Some(Err(e)) => return Err(e),
                None => None,
            },
            replace: None,
            patch: None,
            workspace: None,
            badges: self.badges.clone(),
            cargo_features: self.cargo_features.clone(),
        });

        fn map_deps(
            config: &Config,
            deps: Option<&BTreeMap<String, TomlDependency>>,
            filter: impl Fn(&TomlDependency) -> bool,
        ) -> CargoResult<Option<BTreeMap<String, TomlDependency>>> {
            let deps = match deps {
                Some(deps) => deps,
                None => return Ok(None),
            };
            let deps = deps
                .iter()
                .filter(|(_k, v)| filter(v))
                .map(|(k, v)| Ok((k.clone(), map_dependency(config, v)?)))
                .collect::<CargoResult<BTreeMap<_, _>>>()?;
            Ok(Some(deps))
        }

        fn map_dependency(config: &Config, dep: &TomlDependency) -> CargoResult<TomlDependency> {
            match dep {
                TomlDependency::Detailed(d) => {
                    let mut d = d.clone();
                    // Path dependencies become crates.io deps.
                    d.path.take();
                    // Same with git dependencies.
                    d.git.take();
                    d.branch.take();
                    d.tag.take();
                    d.rev.take();
                    // registry specifications are elaborated to the index URL
                    if let Some(registry) = d.registry.take() {
                        let src = SourceId::alt_registry(config, &registry)?;
                        d.registry_index = Some(src.url().to_string());
                    }
                    Ok(TomlDependency::Detailed(d))
                }
                TomlDependency::Simple(s) => Ok(TomlDependency::Detailed(DetailedTomlDependency {
                    version: Some(s.clone()),
                    ..Default::default()
                })),
            }
        }
    }

    pub fn to_real_manifest(
        me: &Rc<TomlManifest>,
        source_id: SourceId,
        package_root: &Path,
        config: &Config,
    ) -> CargoResult<(Manifest, Vec<PathBuf>)> {
        let mut nested_paths = vec![];
        let mut warnings = vec![];
        let mut errors = vec![];

        // Parse features first so they will be available when parsing other parts of the TOML.
        let empty = Vec::new();
        let cargo_features = me.cargo_features.as_ref().unwrap_or(&empty);
        let features = Features::new(cargo_features, &mut warnings)?;

        let project = me.project.as_ref().or_else(|| me.package.as_ref());
        let project = project.ok_or_else(|| failure::format_err!("no `package` section found"))?;

        let package_name = project.name.trim();
        if package_name.is_empty() {
            bail!("package name cannot be an empty string")
        }

        validate_package_name(package_name, "package name", "")?;

        let pkgid = project.to_package_id(source_id)?;

        let edition = if let Some(ref edition) = project.edition {
            features
                .require(Feature::edition())
                .chain_err(|| "editions are unstable")?;
            edition
                .parse()
                .chain_err(|| "failed to parse the `edition` key")?
        } else {
            Edition::Edition2015
        };

        if project.metabuild.is_some() {
            features.require(Feature::metabuild())?;
        }

        // If we have no lib at all, use the inferred lib, if available.
        // If we have a lib with a path, we're done.
        // If we have a lib with no path, use the inferred lib or else the package name.
        let targets = targets(
            &features,
            me,
            package_name,
            package_root,
            edition,
            &project.build,
            &project.metabuild,
            &mut warnings,
            &mut errors,
        )?;

        if targets.is_empty() {
            debug!("manifest has no build targets");
        }

        if let Err(e) = unique_build_targets(&targets, package_root) {
            warnings.push(format!(
                "file found to be present in multiple \
                 build targets: {}",
                e
            ));
        }

        if let Some(links) = &project.links {
            if !targets.iter().any(|t| t.is_custom_build()) {
                bail!(
                    "package `{}` specifies that it links to `{}` but does not \
                     have a custom build script",
                    pkgid,
                    links
                )
            }
        }

        let mut deps = Vec::new();
        let replace;
        let patch;

        {
            let mut cx = Context {
                pkgid: Some(pkgid),
                deps: &mut deps,
                source_id,
                nested_paths: &mut nested_paths,
                config,
                warnings: &mut warnings,
                features: &features,
                platform: None,
                root: package_root,
            };

            fn process_dependencies(
                cx: &mut Context<'_, '_>,
                new_deps: Option<&BTreeMap<String, TomlDependency>>,
                kind: Option<Kind>,
            ) -> CargoResult<()> {
                let dependencies = match new_deps {
                    Some(dependencies) => dependencies,
                    None => return Ok(()),
                };
                for (n, v) in dependencies.iter() {
                    let dep = v.to_dependency(n, cx, kind)?;
                    cx.deps.push(dep);
                }

                Ok(())
            }

            // Collect the dependencies.
            process_dependencies(&mut cx, me.dependencies.as_ref(), None)?;
            let dev_deps = me
                .dev_dependencies
                .as_ref()
                .or_else(|| me.dev_dependencies2.as_ref());
            process_dependencies(&mut cx, dev_deps, Some(Kind::Development))?;
            let build_deps = me
                .build_dependencies
                .as_ref()
                .or_else(|| me.build_dependencies2.as_ref());
            process_dependencies(&mut cx, build_deps, Some(Kind::Build))?;

            for (name, platform) in me.target.iter().flatten() {
                cx.platform = Some(name.parse()?);
                process_dependencies(&mut cx, platform.dependencies.as_ref(), None)?;
                let build_deps = platform
                    .build_dependencies
                    .as_ref()
                    .or_else(|| platform.build_dependencies2.as_ref());
                process_dependencies(&mut cx, build_deps, Some(Kind::Build))?;
                let dev_deps = platform
                    .dev_dependencies
                    .as_ref()
                    .or_else(|| platform.dev_dependencies2.as_ref());
                process_dependencies(&mut cx, dev_deps, Some(Kind::Development))?;
            }

            replace = me.replace(&mut cx)?;
            patch = me.patch(&mut cx)?;
        }

        {
            let mut names_sources = BTreeMap::new();
            for dep in &deps {
                let name = dep.name_in_toml();
                let prev = names_sources.insert(name.to_string(), dep.source_id());
                if prev.is_some() && prev != Some(dep.source_id()) {
                    bail!(
                        "Dependency '{}' has different source paths depending on the build \
                         target. Each dependency must have a single canonical source path \
                         irrespective of build target.",
                        name
                    );
                }
            }
        }

        let exclude = project.exclude.clone().unwrap_or_default();
        let include = project.include.clone().unwrap_or_default();
        if project.namespaced_features.is_some() {
            features.require(Feature::namespaced_features())?;
        }

        let summary = Summary::new(
            pkgid,
            deps,
            &me.features
                .as_ref()
                .map(|x| {
                    x.iter()
                        .map(|(k, v)| (k.as_str(), v.iter().collect()))
                        .collect()
                })
                .unwrap_or_else(BTreeMap::new),
            project.links.as_ref().map(|x| x.as_str()),
            project.namespaced_features.unwrap_or(false),
        )?;
        let metadata = ManifestMetadata {
            description: project.description.clone(),
            homepage: project.homepage.clone(),
            documentation: project.documentation.clone(),
            readme: project.readme.clone(),
            authors: project.authors.clone().unwrap_or_default(),
            license: project.license.clone(),
            license_file: project.license_file.clone(),
            repository: project.repository.clone(),
            keywords: project.keywords.clone().unwrap_or_default(),
            categories: project.categories.clone().unwrap_or_default(),
            badges: me.badges.clone().unwrap_or_default(),
            links: project.links.clone(),
        };

        let workspace_config = match (me.workspace.as_ref(), project.workspace.as_ref()) {
            (Some(config), None) => WorkspaceConfig::Root(WorkspaceRootConfig::new(
                package_root,
                &config.members,
                &config.default_members,
                &config.exclude,
            )),
            (None, root) => WorkspaceConfig::Member {
                root: root.cloned(),
            },
            (Some(..), Some(..)) => bail!(
                "cannot configure both `package.workspace` and \
                 `[workspace]`, only one can be specified"
            ),
        };
        let profiles = Profiles::new(me.profile.as_ref(), config, &features, &mut warnings)?;
        let publish = match project.publish {
            Some(VecStringOrBool::VecString(ref vecstring)) => Some(vecstring.clone()),
            Some(VecStringOrBool::Bool(false)) => Some(vec![]),
            None | Some(VecStringOrBool::Bool(true)) => None,
        };

        let publish_lockfile = match project.publish_lockfile {
            Some(b) => {
                features.require(Feature::publish_lockfile())?;
                warnings.push(
                    "The `publish-lockfile` feature is deprecated and currently \
                     has no effect. It may be removed in a future version."
                        .to_string(),
                );
                b
            }
            None => features.is_enabled(Feature::publish_lockfile()),
        };

        if summary.features().contains_key("default-features") {
            warnings.push(
                "`default-features = [\"..\"]` was found in [features]. \
                 Did you mean to use `default = [\"..\"]`?"
                    .to_string(),
            )
        }

        if let Some(run) = &project.default_run {
            if !targets
                .iter()
                .filter(|t| t.is_bin())
                .any(|t| t.name() == run)
            {
                let suggestion =
                    util::closest_msg(run, targets.iter().filter(|t| t.is_bin()), |t| t.name());
                bail!("default-run target `{}` not found{}", run, suggestion);
            }
        }

        let custom_metadata = project.metadata.clone();
        let mut manifest = Manifest::new(
            summary,
            targets,
            exclude,
            include,
            project.links.clone(),
            metadata,
            custom_metadata,
            profiles,
            publish,
            publish_lockfile,
            replace,
            patch,
            workspace_config,
            features,
            edition,
            project.im_a_teapot,
            project.default_run.clone(),
            Rc::clone(me),
            project.metabuild.clone().map(|sov| sov.0),
        );
        if project.license_file.is_some() && project.license.is_some() {
            manifest.warnings_mut().add_warning(
                "only one of `license` or \
                 `license-file` is necessary"
                    .to_string(),
            );
        }
        for warning in warnings {
            manifest.warnings_mut().add_warning(warning);
        }
        for error in errors {
            manifest.warnings_mut().add_critical_warning(error);
        }

        manifest.feature_gate()?;

        Ok((manifest, nested_paths))
    }

    fn to_virtual_manifest(
        me: &Rc<TomlManifest>,
        source_id: SourceId,
        root: &Path,
        config: &Config,
    ) -> CargoResult<(VirtualManifest, Vec<PathBuf>)> {
        if me.project.is_some() {
            bail!("virtual manifests do not define [project]");
        }
        if me.package.is_some() {
            bail!("virtual manifests do not define [package]");
        }
        if me.lib.is_some() {
            bail!("virtual manifests do not specify [lib]");
        }
        if me.bin.is_some() {
            bail!("virtual manifests do not specify [[bin]]");
        }
        if me.example.is_some() {
            bail!("virtual manifests do not specify [[example]]");
        }
        if me.test.is_some() {
            bail!("virtual manifests do not specify [[test]]");
        }
        if me.bench.is_some() {
            bail!("virtual manifests do not specify [[bench]]");
        }
        if me.dependencies.is_some() {
            bail!("virtual manifests do not specify [dependencies]");
        }
        if me.dev_dependencies.is_some() || me.dev_dependencies2.is_some() {
            bail!("virtual manifests do not specify [dev-dependencies]");
        }
        if me.build_dependencies.is_some() || me.build_dependencies2.is_some() {
            bail!("virtual manifests do not specify [build-dependencies]");
        }
        if me.features.is_some() {
            bail!("virtual manifests do not specify [features]");
        }
        if me.target.is_some() {
            bail!("virtual manifests do not specify [target]");
        }
        if me.badges.is_some() {
            bail!("virtual manifests do not specify [badges]");
        }

        let mut nested_paths = Vec::new();
        let mut warnings = Vec::new();
        let mut deps = Vec::new();
        let empty = Vec::new();
        let cargo_features = me.cargo_features.as_ref().unwrap_or(&empty);
        let features = Features::new(cargo_features, &mut warnings)?;

        let (replace, patch) = {
            let mut cx = Context {
                pkgid: None,
                deps: &mut deps,
                source_id,
                nested_paths: &mut nested_paths,
                config,
                warnings: &mut warnings,
                platform: None,
                features: &features,
                root,
            };
            (me.replace(&mut cx)?, me.patch(&mut cx)?)
        };
        let profiles = Profiles::new(me.profile.as_ref(), config, &features, &mut warnings)?;
        let workspace_config = match me.workspace {
            Some(ref config) => WorkspaceConfig::Root(WorkspaceRootConfig::new(
                root,
                &config.members,
                &config.default_members,
                &config.exclude,
            )),
            None => {
                bail!("virtual manifests must be configured with [workspace]");
            }
        };
        Ok((
            VirtualManifest::new(replace, patch, workspace_config, profiles, features),
            nested_paths,
        ))
    }

    fn replace(&self, cx: &mut Context<'_, '_>) -> CargoResult<Vec<(PackageIdSpec, Dependency)>> {
        if self.patch.is_some() && self.replace.is_some() {
            bail!("cannot specify both [replace] and [patch]");
        }
        let mut replace = Vec::new();
        for (spec, replacement) in self.replace.iter().flatten() {
            let mut spec = PackageIdSpec::parse(spec).chain_err(|| {
                format!(
                    "replacements must specify a valid semver \
                     version to replace, but `{}` does not",
                    spec
                )
            })?;
            if spec.url().is_none() {
                spec.set_url(CRATES_IO_INDEX.parse().unwrap());
            }

            if replacement.is_version_specified() {
                bail!(
                    "replacements cannot specify a version \
                     requirement, but found one for `{}`",
                    spec
                );
            }

            let mut dep = replacement.to_dependency(spec.name().as_str(), cx, None)?;
            {
                let version = spec.version().ok_or_else(|| {
                    failure::format_err!(
                        "replacements must specify a version \
                         to replace, but `{}` does not",
                        spec
                    )
                })?;
                dep.set_version_req(VersionReq::exact(version));
            }
            replace.push((spec, dep));
        }
        Ok(replace)
    }

    fn patch(&self, cx: &mut Context<'_, '_>) -> CargoResult<HashMap<Url, Vec<Dependency>>> {
        let mut patch = HashMap::new();
        for (url, deps) in self.patch.iter().flatten() {
            let url = match &url[..] {
                CRATES_IO_REGISTRY => CRATES_IO_INDEX.parse().unwrap(),
                _ => cx
                    .config
                    .get_registry_index(url)
                    .or_else(|_| url.into_url())
                    .chain_err(|| {
                        format!("[patch] entry `{}` should be a URL or registry name", url)
                    })?,
            };
            patch.insert(
                url,
                deps.iter()
                    .map(|(name, dep)| dep.to_dependency(name, cx, None))
                    .collect::<CargoResult<Vec<_>>>()?,
            );
        }
        Ok(patch)
    }

    fn maybe_custom_build(
        &self,
        build: &Option<StringOrBool>,
        package_root: &Path,
    ) -> Option<PathBuf> {
        let build_rs = package_root.join("build.rs");
        match *build {
            // Explicitly no build script.
            Some(StringOrBool::Bool(false)) => None,
            Some(StringOrBool::Bool(true)) => Some(build_rs),
            Some(StringOrBool::String(ref s)) => Some(PathBuf::from(s)),
            None => {
                match fs::metadata(&build_rs) {
                    // If there is a `build.rs` file next to the `Cargo.toml`, assume it is
                    // a build script.
                    Ok(ref e) if e.is_file() => Some(build_rs),
                    Ok(_) | Err(_) => None,
                }
            }
        }
    }

    pub fn has_profiles(&self) -> bool {
        self.profile.is_some()
    }
}

/// Checks a list of build targets, and ensures the target names are unique within a vector.
/// If not, the name of the offending build target is returned.
fn unique_build_targets(targets: &[Target], package_root: &Path) -> Result<(), String> {
    let mut seen = HashSet::new();
    for target in targets {
        if let TargetSourcePath::Path(path) = target.src_path() {
            let full = package_root.join(path);
            if !seen.insert(full.clone()) {
                return Err(full.display().to_string());
            }
        }
    }
    Ok(())
}

impl TomlDependency {
    fn to_dependency(
        &self,
        name: &str,
        cx: &mut Context<'_, '_>,
        kind: Option<Kind>,
    ) -> CargoResult<Dependency> {
        match *self {
            TomlDependency::Simple(ref version) => DetailedTomlDependency {
                version: Some(version.clone()),
                ..Default::default()
            }
            .to_dependency(name, cx, kind),
            TomlDependency::Detailed(ref details) => details.to_dependency(name, cx, kind),
        }
    }

    fn is_version_specified(&self) -> bool {
        match self {
            TomlDependency::Detailed(d) => d.version.is_some(),
            TomlDependency::Simple(..) => true,
        }
    }
}

impl DetailedTomlDependency {
    fn to_dependency(
        &self,
        name_in_toml: &str,
        cx: &mut Context<'_, '_>,
        kind: Option<Kind>,
    ) -> CargoResult<Dependency> {
        if self.version.is_none() && self.path.is_none() && self.git.is_none() {
            let msg = format!(
                "dependency ({}) specified without \
                 providing a local path, Git repository, or \
                 version to use. This will be considered an \
                 error in future versions",
                name_in_toml
            );
            cx.warnings.push(msg);
        }

        if let Some(version) = &self.version {
            if version.contains('+') {
                cx.warnings.push(format!(
                    "version requirement `{}` for dependency `{}` \
                     includes semver metadata which will be ignored, removing the \
                     metadata is recommended to avoid confusion",
                    version, name_in_toml
                ));
            }
        }

        if self.git.is_none() {
            let git_only_keys = [
                (&self.branch, "branch"),
                (&self.tag, "tag"),
                (&self.rev, "rev"),
            ];

            for &(key, key_name) in &git_only_keys {
                if key.is_some() {
                    let msg = format!(
                        "key `{}` is ignored for dependency ({}). \
                         This will be considered an error in future versions",
                        key_name, name_in_toml
                    );
                    cx.warnings.push(msg)
                }
            }
        }

        let new_source_id = match (
            self.git.as_ref(),
            self.path.as_ref(),
            self.registry.as_ref(),
            self.registry_index.as_ref(),
        ) {
            (Some(_), _, Some(_), _) | (Some(_), _, _, Some(_)) => bail!(
                "dependency ({}) specification is ambiguous. \
                 Only one of `git` or `registry` is allowed.",
                name_in_toml
            ),
            (_, _, Some(_), Some(_)) => bail!(
                "dependency ({}) specification is ambiguous. \
                 Only one of `registry` or `registry-index` is allowed.",
                name_in_toml
            ),
            (Some(git), maybe_path, _, _) => {
                if maybe_path.is_some() {
                    let msg = format!(
                        "dependency ({}) specification is ambiguous. \
                         Only one of `git` or `path` is allowed. \
                         This will be considered an error in future versions",
                        name_in_toml
                    );
                    cx.warnings.push(msg)
                }

                let n_details = [&self.branch, &self.tag, &self.rev]
                    .iter()
                    .filter(|d| d.is_some())
                    .count();

                if n_details > 1 {
                    let msg = format!(
                        "dependency ({}) specification is ambiguous. \
                         Only one of `branch`, `tag` or `rev` is allowed. \
                         This will be considered an error in future versions",
                        name_in_toml
                    );
                    cx.warnings.push(msg)
                }

                let reference = self
                    .branch
                    .clone()
                    .map(GitReference::Branch)
                    .or_else(|| self.tag.clone().map(GitReference::Tag))
                    .or_else(|| self.rev.clone().map(GitReference::Rev))
                    .unwrap_or_else(|| GitReference::Branch("master".to_string()));
                let loc = git.into_url()?;
                SourceId::for_git(&loc, reference)?
            }
            (None, Some(path), _, _) => {
                cx.nested_paths.push(PathBuf::from(path));
                // If the source ID for the package we're parsing is a path
                // source, then we normalize the path here to get rid of
                // components like `..`.
                //
                // The purpose of this is to get a canonical ID for the package
                // that we're depending on to ensure that builds of this package
                // always end up hashing to the same value no matter where it's
                // built from.
                if cx.source_id.is_path() {
                    let path = cx.root.join(path);
                    let path = util::normalize_path(&path);
                    SourceId::for_path(&path)?
                } else {
                    cx.source_id
                }
            }
            (None, None, Some(registry), None) => SourceId::alt_registry(cx.config, registry)?,
            (None, None, None, Some(registry_index)) => {
                let url = registry_index.into_url()?;
                SourceId::for_registry(&url)?
            }
            (None, None, None, None) => SourceId::crates_io(cx.config)?,
        };

        let (pkg_name, explicit_name_in_toml) = match self.package {
            Some(ref s) => (&s[..], Some(name_in_toml)),
            None => (name_in_toml, None),
        };

        let version = self.version.as_ref().map(|v| &v[..]);
        let mut dep = match cx.pkgid {
            Some(id) => Dependency::parse(pkg_name, version, new_source_id, id, cx.config)?,
            None => Dependency::parse_no_deprecated(pkg_name, version, new_source_id)?,
        };
        dep.set_features(self.features.iter().flatten())
            .set_default_features(
                self.default_features
                    .or(self.default_features2)
                    .unwrap_or(true),
            )
            .set_optional(self.optional.unwrap_or(false))
            .set_platform(cx.platform.clone());
        if let Some(registry) = &self.registry {
            let registry_id = SourceId::alt_registry(cx.config, registry)?;
            dep.set_registry_id(registry_id);
        }
        if let Some(registry_index) = &self.registry_index {
            let url = registry_index.into_url()?;
            let registry_id = SourceId::for_registry(&url)?;
            dep.set_registry_id(registry_id);
        }

        if let Some(kind) = kind {
            dep.set_kind(kind);
        }
        if let Some(name_in_toml) = explicit_name_in_toml {
            cx.features.require(Feature::rename_dependency())?;
            dep.set_explicit_name_in_toml(name_in_toml);
        }

        if let Some(p) = self.public {
            cx.features.require(Feature::public_dependency())?;

            if dep.kind() != Kind::Normal {
                bail!("'public' specifier can only be used on regular dependencies, not {:?} dependencies", dep.kind());
            }

            dep.set_public(p);
        }
        Ok(dep)
    }
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
struct TomlTarget {
    name: Option<String>,

    // The intention was to only accept `crate-type` here but historical
    // versions of Cargo also accepted `crate_type`, so look for both.
    #[serde(rename = "crate-type")]
    crate_type: Option<Vec<String>>,
    #[serde(rename = "crate_type")]
    crate_type2: Option<Vec<String>>,

    path: Option<PathValue>,
    test: Option<bool>,
    doctest: Option<bool>,
    bench: Option<bool>,
    doc: Option<bool>,
    plugin: Option<bool>,
    #[serde(rename = "proc-macro")]
    proc_macro: Option<bool>,
    #[serde(rename = "proc_macro")]
    proc_macro2: Option<bool>,
    harness: Option<bool>,
    #[serde(rename = "required-features")]
    required_features: Option<Vec<String>>,
    edition: Option<String>,
}

#[derive(Clone)]
struct PathValue(PathBuf);

impl<'de> de::Deserialize<'de> for PathValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        Ok(PathValue(String::deserialize(deserializer)?.into()))
    }
}

impl ser::Serialize for PathValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        self.0.serialize(serializer)
    }
}

/// Corresponds to a `target` entry, but `TomlTarget` is already used.
#[derive(Serialize, Deserialize, Debug)]
struct TomlPlatform {
    dependencies: Option<BTreeMap<String, TomlDependency>>,
    #[serde(rename = "build-dependencies")]
    build_dependencies: Option<BTreeMap<String, TomlDependency>>,
    #[serde(rename = "build_dependencies")]
    build_dependencies2: Option<BTreeMap<String, TomlDependency>>,
    #[serde(rename = "dev-dependencies")]
    dev_dependencies: Option<BTreeMap<String, TomlDependency>>,
    #[serde(rename = "dev_dependencies")]
    dev_dependencies2: Option<BTreeMap<String, TomlDependency>>,
}

impl TomlTarget {
    fn new() -> TomlTarget {
        TomlTarget::default()
    }

    fn name(&self) -> String {
        match self.name {
            Some(ref name) => name.clone(),
            None => panic!("target name is required"),
        }
    }

    fn proc_macro(&self) -> Option<bool> {
        self.proc_macro.or(self.proc_macro2).or_else(|| {
            if let Some(types) = self.crate_types() {
                if types.contains(&"proc-macro".to_string()) {
                    return Some(true);
                }
            }
            None
        })
    }

    fn crate_types(&self) -> Option<&Vec<String>> {
        self.crate_type
            .as_ref()
            .or_else(|| self.crate_type2.as_ref())
    }
}

impl fmt::Debug for PathValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
