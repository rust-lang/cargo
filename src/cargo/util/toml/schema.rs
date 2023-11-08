use std::collections::BTreeMap;
use std::fmt::{self, Display, Write};
use std::path::PathBuf;
use std::str;

use serde::de::{self, IntoDeserializer as _, Unexpected};
use serde::ser;
use serde::{Deserialize, Serialize};
use serde_untagged::UntaggedEnumVisitor;

use crate::core::PackageIdSpec;
use crate::util::RustVersion;

/// This type is used to deserialize `Cargo.toml` files.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct TomlManifest {
    pub cargo_features: Option<Vec<String>>,
    pub package: Option<Box<TomlPackage>>,
    pub project: Option<Box<TomlPackage>>,
    pub profile: Option<TomlProfiles>,
    pub lib: Option<TomlLibTarget>,
    pub bin: Option<Vec<TomlBinTarget>>,
    pub example: Option<Vec<TomlExampleTarget>>,
    pub test: Option<Vec<TomlTestTarget>>,
    pub bench: Option<Vec<TomlTestTarget>>,
    pub dependencies: Option<BTreeMap<String, MaybeWorkspaceDependency>>,
    pub dev_dependencies: Option<BTreeMap<String, MaybeWorkspaceDependency>>,
    #[serde(rename = "dev_dependencies")]
    pub dev_dependencies2: Option<BTreeMap<String, MaybeWorkspaceDependency>>,
    pub build_dependencies: Option<BTreeMap<String, MaybeWorkspaceDependency>>,
    #[serde(rename = "build_dependencies")]
    pub build_dependencies2: Option<BTreeMap<String, MaybeWorkspaceDependency>>,
    pub features: Option<BTreeMap<String, Vec<String>>>,
    pub target: Option<BTreeMap<String, TomlPlatform>>,
    pub replace: Option<BTreeMap<String, TomlDependency>>,
    pub patch: Option<BTreeMap<String, BTreeMap<String, TomlDependency>>>,
    pub workspace: Option<TomlWorkspace>,
    pub badges: Option<MaybeWorkspaceBtreeMap>,
    pub lints: Option<MaybeWorkspaceLints>,
}

impl TomlManifest {
    pub fn has_profiles(&self) -> bool {
        self.profile.is_some()
    }

    pub fn dev_dependencies(&self) -> Option<&BTreeMap<String, MaybeWorkspaceDependency>> {
        self.dev_dependencies
            .as_ref()
            .or(self.dev_dependencies2.as_ref())
    }

    pub fn build_dependencies(&self) -> Option<&BTreeMap<String, MaybeWorkspaceDependency>> {
        self.build_dependencies
            .as_ref()
            .or(self.build_dependencies2.as_ref())
    }

    pub fn features(&self) -> Option<&BTreeMap<String, Vec<String>>> {
        self.features.as_ref()
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct TomlWorkspace {
    pub members: Option<Vec<String>>,
    pub exclude: Option<Vec<String>>,
    pub default_members: Option<Vec<String>>,
    pub resolver: Option<String>,
    pub metadata: Option<toml::Value>,

    // Properties that can be inherited by members.
    pub package: Option<InheritableFields>,
    pub dependencies: Option<BTreeMap<String, TomlDependency>>,
    pub lints: Option<TomlLints>,
}

/// A group of fields that are inheritable by members of the workspace
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct InheritableFields {
    // We use skip here since it will never be present when deserializing
    // and we don't want it present when serializing
    #[serde(skip)]
    pub dependencies: Option<BTreeMap<String, TomlDependency>>,
    #[serde(skip)]
    pub lints: Option<TomlLints>,

    pub version: Option<semver::Version>,
    pub authors: Option<Vec<String>>,
    pub description: Option<String>,
    pub homepage: Option<String>,
    pub documentation: Option<String>,
    pub readme: Option<StringOrBool>,
    pub keywords: Option<Vec<String>>,
    pub categories: Option<Vec<String>>,
    pub license: Option<String>,
    pub license_file: Option<String>,
    pub repository: Option<String>,
    pub publish: Option<VecStringOrBool>,
    pub edition: Option<String>,
    pub badges: Option<BTreeMap<String, BTreeMap<String, String>>>,
    pub exclude: Option<Vec<String>>,
    pub include: Option<Vec<String>>,
    pub rust_version: Option<RustVersion>,
    // We use skip here since it will never be present when deserializing
    // and we don't want it present when serializing
    #[serde(skip)]
    pub ws_root: PathBuf,
}

/// Represents the `package`/`project` sections of a `Cargo.toml`.
///
/// Note that the order of the fields matters, since this is the order they
/// are serialized to a TOML file. For example, you cannot have values after
/// the field `metadata`, since it is a table and values cannot appear after
/// tables.
#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "kebab-case")]
pub struct TomlPackage {
    pub edition: Option<MaybeWorkspaceString>,
    pub rust_version: Option<MaybeWorkspaceRustVersion>,
    pub name: String,
    pub version: Option<MaybeWorkspaceSemverVersion>,
    pub authors: Option<MaybeWorkspaceVecString>,
    pub build: Option<StringOrBool>,
    pub metabuild: Option<StringOrVec>,
    pub default_target: Option<String>,
    pub forced_target: Option<String>,
    pub links: Option<String>,
    pub exclude: Option<MaybeWorkspaceVecString>,
    pub include: Option<MaybeWorkspaceVecString>,
    pub publish: Option<MaybeWorkspaceVecStringOrBool>,
    pub workspace: Option<String>,
    pub im_a_teapot: Option<bool>,
    pub autobins: Option<bool>,
    pub autoexamples: Option<bool>,
    pub autotests: Option<bool>,
    pub autobenches: Option<bool>,
    pub default_run: Option<String>,

    // Package metadata.
    pub description: Option<MaybeWorkspaceString>,
    pub homepage: Option<MaybeWorkspaceString>,
    pub documentation: Option<MaybeWorkspaceString>,
    pub readme: Option<MaybeWorkspaceStringOrBool>,
    pub keywords: Option<MaybeWorkspaceVecString>,
    pub categories: Option<MaybeWorkspaceVecString>,
    pub license: Option<MaybeWorkspaceString>,
    pub license_file: Option<MaybeWorkspaceString>,
    pub repository: Option<MaybeWorkspaceString>,
    pub resolver: Option<String>,

    pub metadata: Option<toml::Value>,

    /// Provide a helpful error message for a common user error.
    #[serde(rename = "cargo-features", skip_serializing)]
    pub _invalid_cargo_features: Option<InvalidCargoFeatures>,
}

/// An enum that allows for inheriting keys from a workspace in a Cargo.toml.
#[derive(Serialize, Copy, Clone, Debug)]
#[serde(untagged)]
pub enum MaybeWorkspace<T, W> {
    /// The "defined" type, or the type that that is used when not inheriting from a workspace.
    Defined(T),
    /// The type when inheriting from a workspace.
    Workspace(W),
}

//. This already has a `Deserialize` impl from version_trim_whitespace
pub type MaybeWorkspaceSemverVersion = MaybeWorkspace<semver::Version, TomlWorkspaceField>;
impl<'de> de::Deserialize<'de> for MaybeWorkspaceSemverVersion {
    fn deserialize<D>(d: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        UntaggedEnumVisitor::new()
            .expecting("SemVer version")
            .string(
                |value| match value.trim().parse().map_err(de::Error::custom) {
                    Ok(parsed) => Ok(MaybeWorkspace::Defined(parsed)),
                    Err(e) => Err(e),
                },
            )
            .map(|value| value.deserialize().map(MaybeWorkspace::Workspace))
            .deserialize(d)
    }
}

pub type MaybeWorkspaceString = MaybeWorkspace<String, TomlWorkspaceField>;
impl<'de> de::Deserialize<'de> for MaybeWorkspaceString {
    fn deserialize<D>(d: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = MaybeWorkspaceString;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
                f.write_str("a string or workspace")
            }

            fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(MaybeWorkspaceString::Defined(value))
            }

            fn visit_map<V>(self, map: V) -> Result<Self::Value, V::Error>
            where
                V: de::MapAccess<'de>,
            {
                let mvd = de::value::MapAccessDeserializer::new(map);
                TomlWorkspaceField::deserialize(mvd).map(MaybeWorkspace::Workspace)
            }
        }

        d.deserialize_any(Visitor)
    }
}

pub type MaybeWorkspaceRustVersion = MaybeWorkspace<RustVersion, TomlWorkspaceField>;
impl<'de> de::Deserialize<'de> for MaybeWorkspaceRustVersion {
    fn deserialize<D>(d: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = MaybeWorkspaceRustVersion;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
                f.write_str("a semver or workspace")
            }

            fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                let value = value.parse::<RustVersion>().map_err(|e| E::custom(e))?;
                Ok(MaybeWorkspaceRustVersion::Defined(value))
            }

            fn visit_map<V>(self, map: V) -> Result<Self::Value, V::Error>
            where
                V: de::MapAccess<'de>,
            {
                let mvd = de::value::MapAccessDeserializer::new(map);
                TomlWorkspaceField::deserialize(mvd).map(MaybeWorkspace::Workspace)
            }
        }

        d.deserialize_any(Visitor)
    }
}

pub type MaybeWorkspaceVecString = MaybeWorkspace<Vec<String>, TomlWorkspaceField>;
impl<'de> de::Deserialize<'de> for MaybeWorkspaceVecString {
    fn deserialize<D>(d: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = MaybeWorkspaceVecString;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
                f.write_str("a vector of strings or workspace")
            }
            fn visit_seq<A>(self, v: A) -> Result<Self::Value, A::Error>
            where
                A: de::SeqAccess<'de>,
            {
                let seq = de::value::SeqAccessDeserializer::new(v);
                Vec::deserialize(seq).map(MaybeWorkspace::Defined)
            }

            fn visit_map<V>(self, map: V) -> Result<Self::Value, V::Error>
            where
                V: de::MapAccess<'de>,
            {
                let mvd = de::value::MapAccessDeserializer::new(map);
                TomlWorkspaceField::deserialize(mvd).map(MaybeWorkspace::Workspace)
            }
        }

        d.deserialize_any(Visitor)
    }
}

pub type MaybeWorkspaceStringOrBool = MaybeWorkspace<StringOrBool, TomlWorkspaceField>;
impl<'de> de::Deserialize<'de> for MaybeWorkspaceStringOrBool {
    fn deserialize<D>(d: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = MaybeWorkspaceStringOrBool;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
                f.write_str("a string, a bool, or workspace")
            }

            fn visit_bool<E>(self, v: bool) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                let b = de::value::BoolDeserializer::new(v);
                StringOrBool::deserialize(b).map(MaybeWorkspace::Defined)
            }

            fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                let string = de::value::StringDeserializer::new(v);
                StringOrBool::deserialize(string).map(MaybeWorkspace::Defined)
            }

            fn visit_map<V>(self, map: V) -> Result<Self::Value, V::Error>
            where
                V: de::MapAccess<'de>,
            {
                let mvd = de::value::MapAccessDeserializer::new(map);
                TomlWorkspaceField::deserialize(mvd).map(MaybeWorkspace::Workspace)
            }
        }

        d.deserialize_any(Visitor)
    }
}

pub type MaybeWorkspaceVecStringOrBool = MaybeWorkspace<VecStringOrBool, TomlWorkspaceField>;
impl<'de> de::Deserialize<'de> for MaybeWorkspaceVecStringOrBool {
    fn deserialize<D>(d: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = MaybeWorkspaceVecStringOrBool;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
                f.write_str("a boolean, a vector of strings, or workspace")
            }

            fn visit_bool<E>(self, v: bool) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                let b = de::value::BoolDeserializer::new(v);
                VecStringOrBool::deserialize(b).map(MaybeWorkspace::Defined)
            }

            fn visit_seq<A>(self, v: A) -> Result<Self::Value, A::Error>
            where
                A: de::SeqAccess<'de>,
            {
                let seq = de::value::SeqAccessDeserializer::new(v);
                VecStringOrBool::deserialize(seq).map(MaybeWorkspace::Defined)
            }

            fn visit_map<V>(self, map: V) -> Result<Self::Value, V::Error>
            where
                V: de::MapAccess<'de>,
            {
                let mvd = de::value::MapAccessDeserializer::new(map);
                TomlWorkspaceField::deserialize(mvd).map(MaybeWorkspace::Workspace)
            }
        }

        d.deserialize_any(Visitor)
    }
}

pub type MaybeWorkspaceBtreeMap =
    MaybeWorkspace<BTreeMap<String, BTreeMap<String, String>>, TomlWorkspaceField>;

impl<'de> de::Deserialize<'de> for MaybeWorkspaceBtreeMap {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        let value = serde_value::Value::deserialize(deserializer)?;

        if let Ok(w) = TomlWorkspaceField::deserialize(
            serde_value::ValueDeserializer::<D::Error>::new(value.clone()),
        ) {
            return if w.workspace {
                Ok(MaybeWorkspace::Workspace(w))
            } else {
                Err(de::Error::custom("`workspace` cannot be false"))
            };
        }
        BTreeMap::deserialize(serde_value::ValueDeserializer::<D::Error>::new(value))
            .map(MaybeWorkspace::Defined)
    }
}

#[derive(Deserialize, Serialize, Copy, Clone, Debug)]
#[serde(rename_all = "kebab-case")]
pub struct TomlWorkspaceField {
    #[serde(deserialize_with = "bool_no_false")]
    pub workspace: bool,
}

fn bool_no_false<'de, D: de::Deserializer<'de>>(deserializer: D) -> Result<bool, D::Error> {
    let b: bool = Deserialize::deserialize(deserializer)?;
    if b {
        Ok(b)
    } else {
        Err(de::Error::custom("`workspace` cannot be false"))
    }
}

pub type MaybeWorkspaceDependency = MaybeWorkspace<TomlDependency, TomlWorkspaceDependency>;

impl MaybeWorkspaceDependency {
    pub fn unused_keys(&self) -> Vec<String> {
        match self {
            MaybeWorkspaceDependency::Defined(d) => d.unused_keys(),
            MaybeWorkspaceDependency::Workspace(w) => w.unused_keys.keys().cloned().collect(),
        }
    }
}

impl<'de> de::Deserialize<'de> for MaybeWorkspaceDependency {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        let value = serde_value::Value::deserialize(deserializer)?;

        if let Ok(w) = TomlWorkspaceDependency::deserialize(serde_value::ValueDeserializer::<
            D::Error,
        >::new(value.clone()))
        {
            return if w.workspace {
                Ok(MaybeWorkspace::Workspace(w))
            } else {
                Err(de::Error::custom("`workspace` cannot be false"))
            };
        }
        TomlDependency::deserialize(serde_value::ValueDeserializer::<D::Error>::new(value))
            .map(MaybeWorkspace::Defined)
    }
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "kebab-case")]
pub struct TomlWorkspaceDependency {
    pub workspace: bool,
    pub features: Option<Vec<String>>,
    pub default_features: Option<bool>,
    #[serde(rename = "default_features")]
    pub default_features2: Option<bool>,
    pub optional: Option<bool>,
    pub public: Option<bool>,

    /// This is here to provide a way to see the "unused manifest keys" when deserializing
    #[serde(skip_serializing)]
    #[serde(flatten)]
    pub unused_keys: BTreeMap<String, toml::Value>,
}

impl TomlWorkspaceDependency {
    pub fn default_features(&self) -> Option<bool> {
        self.default_features.or(self.default_features2)
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(untagged)]
pub enum TomlDependency<P: Clone = String> {
    /// In the simple format, only a version is specified, eg.
    /// `package = "<version>"`
    Simple(String),
    /// The simple format is equivalent to a detailed dependency
    /// specifying only a version, eg.
    /// `package = { version = "<version>" }`
    Detailed(DetailedTomlDependency<P>),
}

impl TomlDependency {
    pub fn is_version_specified(&self) -> bool {
        match self {
            TomlDependency::Detailed(d) => d.version.is_some(),
            TomlDependency::Simple(..) => true,
        }
    }

    pub fn is_optional(&self) -> bool {
        match self {
            TomlDependency::Detailed(d) => d.optional.unwrap_or(false),
            TomlDependency::Simple(..) => false,
        }
    }

    pub fn unused_keys(&self) -> Vec<String> {
        match self {
            TomlDependency::Simple(_) => vec![],
            TomlDependency::Detailed(detailed) => detailed.unused_keys.keys().cloned().collect(),
        }
    }
}

impl<'de, P: Deserialize<'de> + Clone> de::Deserialize<'de> for TomlDependency<P> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        UntaggedEnumVisitor::new()
            .expecting(
                "a version string like \"0.9.8\" or a \
                     detailed dependency like { version = \"0.9.8\" }",
            )
            .string(|value| Ok(TomlDependency::Simple(value.to_owned())))
            .map(|value| value.deserialize().map(TomlDependency::Detailed))
            .deserialize(deserializer)
    }
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "kebab-case")]
pub struct DetailedTomlDependency<P: Clone = String> {
    pub version: Option<String>,
    pub registry: Option<String>,
    /// The URL of the `registry` field.
    /// This is an internal implementation detail. When Cargo creates a
    /// package, it replaces `registry` with `registry-index` so that the
    /// manifest contains the correct URL. All users won't have the same
    /// registry names configured, so Cargo can't rely on just the name for
    /// crates published by other users.
    pub registry_index: Option<String>,
    // `path` is relative to the file it appears in. If that's a `Cargo.toml`, it'll be relative to
    // that TOML file, and if it's a `.cargo/config` file, it'll be relative to that file.
    pub path: Option<P>,
    pub git: Option<String>,
    pub branch: Option<String>,
    pub tag: Option<String>,
    pub rev: Option<String>,
    pub features: Option<Vec<String>>,
    pub optional: Option<bool>,
    pub default_features: Option<bool>,
    #[serde(rename = "default_features")]
    pub default_features2: Option<bool>,
    pub package: Option<String>,
    pub public: Option<bool>,

    /// One or more of `bin`, `cdylib`, `staticlib`, `bin:<name>`.
    pub artifact: Option<StringOrVec>,
    /// If set, the artifact should also be a dependency
    pub lib: Option<bool>,
    /// A platform name, like `x86_64-apple-darwin`
    pub target: Option<String>,

    /// This is here to provide a way to see the "unused manifest keys" when deserializing
    #[serde(skip_serializing)]
    #[serde(flatten)]
    pub unused_keys: BTreeMap<String, toml::Value>,
}

impl<P: Clone> DetailedTomlDependency<P> {
    pub fn default_features(&self) -> Option<bool> {
        self.default_features.or(self.default_features2)
    }
}

// Explicit implementation so we avoid pulling in P: Default
impl<P: Clone> Default for DetailedTomlDependency<P> {
    fn default() -> Self {
        Self {
            version: Default::default(),
            registry: Default::default(),
            registry_index: Default::default(),
            path: Default::default(),
            git: Default::default(),
            branch: Default::default(),
            tag: Default::default(),
            rev: Default::default(),
            features: Default::default(),
            optional: Default::default(),
            default_features: Default::default(),
            default_features2: Default::default(),
            package: Default::default(),
            public: Default::default(),
            artifact: Default::default(),
            lib: Default::default(),
            target: Default::default(),
            unused_keys: Default::default(),
        }
    }
}

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
pub struct TomlProfiles(pub BTreeMap<String, TomlProfile>);

impl TomlProfiles {
    pub fn get_all(&self) -> &BTreeMap<String, TomlProfile> {
        &self.0
    }

    pub fn get(&self, name: &str) -> Option<&TomlProfile> {
        self.0.get(name)
    }
}

#[derive(Deserialize, Serialize, Clone, Debug, Default, Eq, PartialEq)]
#[serde(default, rename_all = "kebab-case")]
pub struct TomlProfile {
    pub opt_level: Option<TomlOptLevel>,
    pub lto: Option<StringOrBool>,
    pub codegen_backend: Option<String>,
    pub codegen_units: Option<u32>,
    pub debug: Option<TomlDebugInfo>,
    pub split_debuginfo: Option<String>,
    pub debug_assertions: Option<bool>,
    pub rpath: Option<bool>,
    pub panic: Option<String>,
    pub overflow_checks: Option<bool>,
    pub incremental: Option<bool>,
    pub dir_name: Option<String>,
    pub inherits: Option<String>,
    pub strip: Option<StringOrBool>,
    // Note that `rustflags` is used for the cargo-feature `profile_rustflags`
    pub rustflags: Option<Vec<String>>,
    // These two fields must be last because they are sub-tables, and TOML
    // requires all non-tables to be listed first.
    pub package: Option<BTreeMap<ProfilePackageSpec, TomlProfile>>,
    pub build_override: Option<Box<TomlProfile>>,
    /// Unstable feature `-Ztrim-paths`.
    pub trim_paths: Option<TomlTrimPaths>,
}

#[derive(Clone, Debug, PartialEq, Eq, Ord, PartialOrd, Hash)]
pub enum ProfilePackageSpec {
    Spec(PackageIdSpec),
    All,
}

impl fmt::Display for ProfilePackageSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProfilePackageSpec::Spec(spec) => spec.fmt(f),
            ProfilePackageSpec::All => f.write_str("*"),
        }
    }
}

impl ser::Serialize for ProfilePackageSpec {
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        self.to_string().serialize(s)
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TomlOptLevel(pub String);

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

impl<'de> de::Deserialize<'de> for TomlOptLevel {
    fn deserialize<D>(d: D) -> Result<TomlOptLevel, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        use serde::de::Error as _;
        UntaggedEnumVisitor::new()
            .expecting("an optimization level")
            .i64(|value| Ok(TomlOptLevel(value.to_string())))
            .string(|value| {
                if value == "s" || value == "z" {
                    Ok(TomlOptLevel(value.to_string()))
                } else {
                    Err(serde_untagged::de::Error::custom(format!(
                        "must be `0`, `1`, `2`, `3`, `s` or `z`, \
                         but found the string: \"{}\"",
                        value
                    )))
                }
            })
            .deserialize(d)
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord)]
pub enum TomlDebugInfo {
    None,
    LineDirectivesOnly,
    LineTablesOnly,
    Limited,
    Full,
}

impl Display for TomlDebugInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TomlDebugInfo::None => f.write_char('0'),
            TomlDebugInfo::Limited => f.write_char('1'),
            TomlDebugInfo::Full => f.write_char('2'),
            TomlDebugInfo::LineDirectivesOnly => f.write_str("line-directives-only"),
            TomlDebugInfo::LineTablesOnly => f.write_str("line-tables-only"),
        }
    }
}

impl ser::Serialize for TomlDebugInfo {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        match self {
            Self::None => 0.serialize(serializer),
            Self::LineDirectivesOnly => "line-directives-only".serialize(serializer),
            Self::LineTablesOnly => "line-tables-only".serialize(serializer),
            Self::Limited => 1.serialize(serializer),
            Self::Full => 2.serialize(serializer),
        }
    }
}

impl<'de> de::Deserialize<'de> for TomlDebugInfo {
    fn deserialize<D>(d: D) -> Result<TomlDebugInfo, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        use serde::de::Error as _;
        let expecting = "a boolean, 0, 1, 2, \"line-tables-only\", or \"line-directives-only\"";
        UntaggedEnumVisitor::new()
            .expecting(expecting)
            .bool(|value| {
                Ok(if value {
                    TomlDebugInfo::Full
                } else {
                    TomlDebugInfo::None
                })
            })
            .i64(|value| {
                let debuginfo = match value {
                    0 => TomlDebugInfo::None,
                    1 => TomlDebugInfo::Limited,
                    2 => TomlDebugInfo::Full,
                    _ => {
                        return Err(serde_untagged::de::Error::invalid_value(
                            Unexpected::Signed(value),
                            &expecting,
                        ))
                    }
                };
                Ok(debuginfo)
            })
            .string(|value| {
                let debuginfo = match value {
                    "none" => TomlDebugInfo::None,
                    "limited" => TomlDebugInfo::Limited,
                    "full" => TomlDebugInfo::Full,
                    "line-directives-only" => TomlDebugInfo::LineDirectivesOnly,
                    "line-tables-only" => TomlDebugInfo::LineTablesOnly,
                    _ => {
                        return Err(serde_untagged::de::Error::invalid_value(
                            Unexpected::Str(value),
                            &expecting,
                        ))
                    }
                };
                Ok(debuginfo)
            })
            .deserialize(d)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Ord, PartialOrd, Hash, Serialize)]
#[serde(untagged, rename_all = "kebab-case")]
pub enum TomlTrimPaths {
    Values(Vec<TomlTrimPathsValue>),
    All,
}

impl TomlTrimPaths {
    pub fn none() -> Self {
        TomlTrimPaths::Values(Vec::new())
    }

    pub fn is_none(&self) -> bool {
        match self {
            TomlTrimPaths::Values(v) => v.is_empty(),
            TomlTrimPaths::All => false,
        }
    }
}

impl<'de> de::Deserialize<'de> for TomlTrimPaths {
    fn deserialize<D>(d: D) -> Result<TomlTrimPaths, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        use serde::de::Error as _;
        let expecting = r#"a boolean, "none", "diagnostics", "macro", "object", "all", or an array with these options"#;
        UntaggedEnumVisitor::new()
            .expecting(expecting)
            .bool(|value| {
                Ok(if value {
                    TomlTrimPaths::All
                } else {
                    TomlTrimPaths::none()
                })
            })
            .string(|v| match v {
                "none" => Ok(TomlTrimPaths::none()),
                "all" => Ok(TomlTrimPaths::All),
                v => {
                    let d = v.into_deserializer();
                    let err = |_: D::Error| {
                        serde_untagged::de::Error::custom(format!("expected {expecting}"))
                    };
                    TomlTrimPathsValue::deserialize(d)
                        .map_err(err)
                        .map(|v| v.into())
                }
            })
            .seq(|seq| {
                let seq: Vec<String> = seq.deserialize()?;
                let seq: Vec<_> = seq
                    .into_iter()
                    .map(|s| TomlTrimPathsValue::deserialize(s.into_deserializer()))
                    .collect::<Result<_, _>>()?;
                Ok(seq.into())
            })
            .deserialize(d)
    }
}

impl fmt::Display for TomlTrimPaths {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TomlTrimPaths::All => write!(f, "all"),
            TomlTrimPaths::Values(v) if v.is_empty() => write!(f, "none"),
            TomlTrimPaths::Values(v) => {
                let mut iter = v.iter();
                if let Some(value) = iter.next() {
                    write!(f, "{value}")?;
                }
                for value in iter {
                    write!(f, ",{value}")?;
                }
                Ok(())
            }
        }
    }
}

impl From<TomlTrimPathsValue> for TomlTrimPaths {
    fn from(value: TomlTrimPathsValue) -> Self {
        TomlTrimPaths::Values(vec![value])
    }
}

impl From<Vec<TomlTrimPathsValue>> for TomlTrimPaths {
    fn from(value: Vec<TomlTrimPathsValue>) -> Self {
        TomlTrimPaths::Values(value)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TomlTrimPathsValue {
    Diagnostics,
    Macro,
    Object,
}

impl TomlTrimPathsValue {
    pub fn as_str(&self) -> &'static str {
        match self {
            TomlTrimPathsValue::Diagnostics => "diagnostics",
            TomlTrimPathsValue::Macro => "macro",
            TomlTrimPathsValue::Object => "object",
        }
    }
}

impl fmt::Display for TomlTrimPathsValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

pub type TomlLibTarget = TomlTarget;
pub type TomlBinTarget = TomlTarget;
pub type TomlExampleTarget = TomlTarget;
pub type TomlTestTarget = TomlTarget;
pub type TomlBenchTarget = TomlTarget;

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct TomlTarget {
    pub name: Option<String>,

    // The intention was to only accept `crate-type` here but historical
    // versions of Cargo also accepted `crate_type`, so look for both.
    pub crate_type: Option<Vec<String>>,
    #[serde(rename = "crate_type")]
    pub crate_type2: Option<Vec<String>>,

    pub path: Option<PathValue>,
    // Note that `filename` is used for the cargo-feature `different_binary_name`
    pub filename: Option<String>,
    pub test: Option<bool>,
    pub doctest: Option<bool>,
    pub bench: Option<bool>,
    pub doc: Option<bool>,
    pub plugin: Option<bool>,
    pub doc_scrape_examples: Option<bool>,
    #[serde(rename = "proc-macro")]
    pub proc_macro_raw: Option<bool>,
    #[serde(rename = "proc_macro")]
    pub proc_macro_raw2: Option<bool>,
    pub harness: Option<bool>,
    pub required_features: Option<Vec<String>>,
    pub edition: Option<String>,
}

impl TomlTarget {
    pub fn new() -> TomlTarget {
        TomlTarget::default()
    }

    pub fn proc_macro(&self) -> Option<bool> {
        self.proc_macro_raw.or(self.proc_macro_raw2).or_else(|| {
            if let Some(types) = self.crate_types() {
                if types.contains(&"proc-macro".to_string()) {
                    return Some(true);
                }
            }
            None
        })
    }

    pub fn crate_types(&self) -> Option<&Vec<String>> {
        self.crate_type
            .as_ref()
            .or_else(|| self.crate_type2.as_ref())
    }
}

/// Corresponds to a `target` entry, but `TomlTarget` is already used.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct TomlPlatform {
    pub dependencies: Option<BTreeMap<String, MaybeWorkspaceDependency>>,
    pub build_dependencies: Option<BTreeMap<String, MaybeWorkspaceDependency>>,
    #[serde(rename = "build_dependencies")]
    pub build_dependencies2: Option<BTreeMap<String, MaybeWorkspaceDependency>>,
    pub dev_dependencies: Option<BTreeMap<String, MaybeWorkspaceDependency>>,
    #[serde(rename = "dev_dependencies")]
    pub dev_dependencies2: Option<BTreeMap<String, MaybeWorkspaceDependency>>,
}

impl TomlPlatform {
    pub fn dev_dependencies(&self) -> Option<&BTreeMap<String, MaybeWorkspaceDependency>> {
        self.dev_dependencies
            .as_ref()
            .or(self.dev_dependencies2.as_ref())
    }

    pub fn build_dependencies(&self) -> Option<&BTreeMap<String, MaybeWorkspaceDependency>> {
        self.build_dependencies
            .as_ref()
            .or(self.build_dependencies2.as_ref())
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(expecting = "a lints table")]
#[serde(rename_all = "kebab-case")]
pub struct MaybeWorkspaceLints {
    #[serde(skip_serializing_if = "is_false")]
    #[serde(deserialize_with = "bool_no_false", default)]
    pub workspace: bool,
    #[serde(flatten)]
    pub lints: TomlLints,
}

fn is_false(b: &bool) -> bool {
    !b
}

pub type TomlLints = BTreeMap<String, TomlToolLints>;

pub type TomlToolLints = BTreeMap<String, TomlLint>;

#[derive(Serialize, Debug, Clone)]
#[serde(untagged)]
pub enum TomlLint {
    Level(TomlLintLevel),
    Config(TomlLintConfig),
}

impl<'de> Deserialize<'de> for TomlLint {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        UntaggedEnumVisitor::new()
            .string(|string| {
                TomlLintLevel::deserialize(string.into_deserializer()).map(TomlLint::Level)
            })
            .map(|map| map.deserialize().map(TomlLint::Config))
            .deserialize(deserializer)
    }
}

impl TomlLint {
    pub fn level(&self) -> TomlLintLevel {
        match self {
            Self::Level(level) => *level,
            Self::Config(config) => config.level,
        }
    }

    pub fn priority(&self) -> i8 {
        match self {
            Self::Level(_) => 0,
            Self::Config(config) => config.priority,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct TomlLintConfig {
    pub level: TomlLintLevel,
    #[serde(default)]
    pub priority: i8,
}

#[derive(Serialize, Deserialize, Debug, Copy, Clone)]
#[serde(rename_all = "kebab-case")]
pub enum TomlLintLevel {
    Forbid,
    Deny,
    Warn,
    Allow,
}

#[derive(Copy, Clone, Debug)]
pub struct InvalidCargoFeatures {}

impl<'de> de::Deserialize<'de> for InvalidCargoFeatures {
    fn deserialize<D>(_d: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        use serde::de::Error as _;

        Err(D::Error::custom(
            "the field `cargo-features` should be set at the top of Cargo.toml before any tables",
        ))
    }
}

/// A StringOrVec can be parsed from either a TOML string or array,
/// but is always stored as a vector.
#[derive(Clone, Debug, Serialize, Eq, PartialEq, PartialOrd, Ord)]
pub struct StringOrVec(pub Vec<String>);

impl StringOrVec {
    pub fn iter<'a>(&'a self) -> std::slice::Iter<'a, String> {
        self.0.iter()
    }
}

impl<'de> de::Deserialize<'de> for StringOrVec {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        UntaggedEnumVisitor::new()
            .expecting("string or list of strings")
            .string(|value| Ok(StringOrVec(vec![value.to_owned()])))
            .seq(|value| value.deserialize().map(StringOrVec))
            .deserialize(deserializer)
    }
}

#[derive(Clone, Debug, Serialize, Eq, PartialEq)]
#[serde(untagged)]
pub enum StringOrBool {
    String(String),
    Bool(bool),
}

impl<'de> Deserialize<'de> for StringOrBool {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        UntaggedEnumVisitor::new()
            .bool(|b| Ok(StringOrBool::Bool(b)))
            .string(|s| Ok(StringOrBool::String(s.to_owned())))
            .deserialize(deserializer)
    }
}

#[derive(PartialEq, Clone, Debug, Serialize)]
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
        UntaggedEnumVisitor::new()
            .expecting("a boolean or vector of strings")
            .bool(|value| Ok(VecStringOrBool::Bool(value)))
            .seq(|value| value.deserialize().map(VecStringOrBool::VecString))
            .deserialize(deserializer)
    }
}

#[derive(Clone)]
pub struct PathValue(pub PathBuf);

impl fmt::Debug for PathValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
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

impl<'de> de::Deserialize<'de> for PathValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        Ok(PathValue(String::deserialize(deserializer)?.into()))
    }
}
