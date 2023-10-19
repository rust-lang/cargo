//! Information about dependencies in a manifest.

use std::fmt::{Display, Formatter};
use std::path::{Path, PathBuf};

use indexmap::IndexSet;
use toml_edit::KeyMut;

use super::manifest::str_or_1_len_table;
use crate::core::GitReference;
use crate::core::SourceId;
use crate::core::Summary;
use crate::CargoResult;
use crate::Config;

/// A dependency handled by Cargo.
///
/// `None` means the field will be blank in TOML.
#[derive(Debug, PartialEq, Eq, Clone)]
#[non_exhaustive]
pub struct Dependency {
    /// The name of the dependency (as it is set in its `Cargo.toml` and known
    /// to crates.io).
    pub name: String,
    /// Whether the dependency is opted-in with a feature flag.
    pub optional: Option<bool>,

    /// List of features to add (or None to keep features unchanged).
    pub features: Option<IndexSet<String>>,
    /// Whether default features are enabled.
    pub default_features: Option<bool>,
    /// List of features inherited from a workspace dependency.
    pub inherited_features: Option<IndexSet<String>>,

    /// Where the dependency comes from.
    pub source: Option<Source>,
    /// Non-default registry.
    pub registry: Option<String>,

    /// If the dependency is renamed, this is the new name for the dependency
    /// as a string.  None if it is not renamed.
    pub rename: Option<String>,
}

impl Dependency {
    /// Create a new dependency with a name.
    pub fn new(name: &str) -> Self {
        Self {
            name: name.into(),
            optional: None,
            features: None,
            default_features: None,
            inherited_features: None,
            source: None,
            registry: None,
            rename: None,
        }
    }

    /// Set dependency to a given version.
    pub fn set_source(mut self, source: impl Into<Source>) -> Self {
        self.source = Some(source.into());
        self
    }

    /// Remove the existing version requirement.
    pub fn clear_version(mut self) -> Self {
        match &mut self.source {
            Some(Source::Registry(_)) => {
                self.source = None;
            }
            Some(Source::Path(path)) => {
                path.version = None;
            }
            Some(Source::Git(git)) => {
                git.version = None;
            }
            Some(Source::Workspace(_workspace)) => {}
            None => {}
        }
        self
    }

    /// Set whether the dependency is optional.
    #[allow(dead_code)]
    pub fn set_optional(mut self, opt: bool) -> Self {
        self.optional = Some(opt);
        self
    }

    /// Set features as an array of string (does some basic parsing).
    #[allow(dead_code)]
    pub fn set_features(mut self, features: IndexSet<String>) -> Self {
        self.features = Some(features);
        self
    }

    /// Set features as an array of string (does some basic parsing).
    pub fn extend_features(mut self, features: impl IntoIterator<Item = String>) -> Self {
        self.features
            .get_or_insert_with(Default::default)
            .extend(features);
        self
    }

    /// Set the value of default-features for the dependency.
    #[allow(dead_code)]
    pub fn set_default_features(mut self, default_features: bool) -> Self {
        self.default_features = Some(default_features);
        self
    }

    /// Set the alias for the dependency.
    pub fn set_rename(mut self, rename: &str) -> Self {
        self.rename = Some(rename.into());
        self
    }

    /// Set the value of registry for the dependency.
    pub fn set_registry(mut self, registry: impl Into<String>) -> Self {
        self.registry = Some(registry.into());
        self
    }

    /// Set features as an array of string (does some basic parsing).
    pub fn set_inherited_features(mut self, features: IndexSet<String>) -> Self {
        self.inherited_features = Some(features);
        self
    }

    /// Get the dependency source.
    pub fn source(&self) -> Option<&Source> {
        self.source.as_ref()
    }

    /// Get version of dependency.
    pub fn version(&self) -> Option<&str> {
        match self.source()? {
            Source::Registry(src) => Some(src.version.as_str()),
            Source::Path(src) => src.version.as_deref(),
            Source::Git(src) => src.version.as_deref(),
            Source::Workspace(_) => None,
        }
    }

    /// Get registry of the dependency.
    pub fn registry(&self) -> Option<&str> {
        self.registry.as_deref()
    }

    /// Get the alias for the dependency (if any).
    pub fn rename(&self) -> Option<&str> {
        self.rename.as_deref()
    }

    /// Whether default features are activated.
    pub fn default_features(&self) -> Option<bool> {
        self.default_features
    }

    /// Get whether the dep is optional.
    pub fn optional(&self) -> Option<bool> {
        self.optional
    }

    /// Get the SourceID for this dependency.
    pub fn source_id(&self, config: &Config) -> CargoResult<MaybeWorkspace<SourceId>> {
        match &self.source.as_ref() {
            Some(Source::Registry(_)) | None => {
                if let Some(r) = self.registry() {
                    let source_id = SourceId::alt_registry(config, r)?;
                    Ok(MaybeWorkspace::Other(source_id))
                } else {
                    let source_id = SourceId::crates_io(config)?;
                    Ok(MaybeWorkspace::Other(source_id))
                }
            }
            Some(Source::Path(source)) => Ok(MaybeWorkspace::Other(source.source_id()?)),
            Some(Source::Git(source)) => Ok(MaybeWorkspace::Other(source.source_id()?)),
            Some(Source::Workspace(workspace)) => Ok(MaybeWorkspace::Workspace(workspace.clone())),
        }
    }

    /// Query to find this dependency.
    pub fn query(
        &self,
        config: &Config,
    ) -> CargoResult<MaybeWorkspace<crate::core::dependency::Dependency>> {
        let source_id = self.source_id(config)?;
        match source_id {
            MaybeWorkspace::Workspace(workspace) => Ok(MaybeWorkspace::Workspace(workspace)),
            MaybeWorkspace::Other(source_id) => Ok(MaybeWorkspace::Other(
                crate::core::dependency::Dependency::parse(
                    self.name.as_str(),
                    self.version(),
                    source_id,
                )?,
            )),
        }
    }
}

/// Either a workspace or another type.
pub enum MaybeWorkspace<T> {
    Workspace(WorkspaceSource),
    Other(T),
}

impl Dependency {
    /// Create a dependency from a TOML table entry.
    pub fn from_toml(crate_root: &Path, key: &str, item: &toml_edit::Item) -> CargoResult<Self> {
        if let Some(version) = item.as_str() {
            let dep = Self::new(key).set_source(RegistrySource::new(version));
            Ok(dep)
        } else if let Some(table) = item.as_table_like() {
            let (name, rename) = if let Some(value) = table.get("package") {
                (
                    value
                        .as_str()
                        .ok_or_else(|| invalid_type(key, "package", value.type_name(), "string"))?
                        .to_owned(),
                    Some(key.to_owned()),
                )
            } else {
                (key.to_owned(), None)
            };

            let source: Source = if let Some(git) = table.get("git") {
                let mut src = GitSource::new(
                    git.as_str()
                        .ok_or_else(|| invalid_type(key, "git", git.type_name(), "string"))?,
                );
                if let Some(value) = table.get("branch") {
                    src =
                        src.set_branch(value.as_str().ok_or_else(|| {
                            invalid_type(key, "branch", value.type_name(), "string")
                        })?);
                }
                if let Some(value) = table.get("tag") {
                    src =
                        src.set_tag(value.as_str().ok_or_else(|| {
                            invalid_type(key, "tag", value.type_name(), "string")
                        })?);
                }
                if let Some(value) = table.get("rev") {
                    src =
                        src.set_rev(value.as_str().ok_or_else(|| {
                            invalid_type(key, "rev", value.type_name(), "string")
                        })?);
                }
                if let Some(value) = table.get("version") {
                    src = src.set_version(value.as_str().ok_or_else(|| {
                        invalid_type(key, "version", value.type_name(), "string")
                    })?);
                }
                src.into()
            } else if let Some(path) = table.get("path") {
                let path =
                    crate_root
                        .join(path.as_str().ok_or_else(|| {
                            invalid_type(key, "path", path.type_name(), "string")
                        })?);
                let mut src = PathSource::new(path);
                if let Some(value) = table.get("version") {
                    src = src.set_version(value.as_str().ok_or_else(|| {
                        invalid_type(key, "version", value.type_name(), "string")
                    })?);
                }
                src.into()
            } else if let Some(version) = table.get("version") {
                let src =
                    RegistrySource::new(version.as_str().ok_or_else(|| {
                        invalid_type(key, "version", version.type_name(), "string")
                    })?);
                src.into()
            } else if let Some(workspace) = table.get("workspace") {
                let workspace_bool = workspace
                    .as_bool()
                    .ok_or_else(|| invalid_type(key, "workspace", workspace.type_name(), "bool"))?;
                if !workspace_bool {
                    anyhow::bail!("`{key}.workspace = false` is unsupported")
                }
                let src = WorkspaceSource::new();
                src.into()
            } else {
                let mut msg = format!("unrecognized dependency source for `{key}`");
                if table.is_empty() {
                    msg.push_str(
                        ", expected a local path, Git repository, version, or workspace dependency to be specified",
                    );
                }
                anyhow::bail!(msg);
            };
            let registry = if let Some(value) = table.get("registry") {
                Some(
                    value
                        .as_str()
                        .ok_or_else(|| invalid_type(key, "registry", value.type_name(), "string"))?
                        .to_owned(),
                )
            } else {
                None
            };

            let default_features = table.get("default-features").and_then(|v| v.as_bool());
            if table.contains_key("default_features") {
                anyhow::bail!("Use of `default_features` in `{key}` is unsupported, please switch to `default-features`");
            }

            let features = if let Some(value) = table.get("features") {
                Some(
                    value
                        .as_array()
                        .ok_or_else(|| invalid_type(key, "features", value.type_name(), "array"))?
                        .iter()
                        .map(|v| {
                            v.as_str().map(|s| s.to_owned()).ok_or_else(|| {
                                invalid_type(key, "features", v.type_name(), "string")
                            })
                        })
                        .collect::<CargoResult<IndexSet<String>>>()?,
                )
            } else {
                None
            };

            let optional = table.get("optional").and_then(|v| v.as_bool());

            let dep = Self {
                name,
                rename,
                source: Some(source),
                registry,
                default_features,
                features,
                optional,
                inherited_features: None,
            };
            Ok(dep)
        } else {
            anyhow::bail!("Unrecognized` dependency entry format for `{key}");
        }
    }

    /// Get the dependency name as defined in the manifest,
    /// that is, either the alias (rename field if Some),
    /// or the official package name (name field).
    pub fn toml_key(&self) -> &str {
        self.rename().unwrap_or(&self.name)
    }

    /// Convert dependency to TOML.
    ///
    /// Returns a tuple with the dependency's name and either the version as a
    /// `String` or the path/git repository as an `InlineTable`.
    /// (If the dependency is set as `optional` or `default-features` is set to
    /// `false`, an `InlineTable` is returned in any case.)
    ///
    /// # Panic
    ///
    /// Panics if the path is relative
    pub fn to_toml(&self, crate_root: &Path) -> toml_edit::Item {
        assert!(
            crate_root.is_absolute(),
            "Absolute path needed, got: {}",
            crate_root.display()
        );
        let table: toml_edit::Item = match (
            self.optional.unwrap_or(false),
            self.features.as_ref(),
            self.default_features.unwrap_or(true),
            self.source.as_ref(),
            self.registry.as_ref(),
            self.rename.as_ref(),
        ) {
            // Extra short when version flag only
            (
                false,
                None,
                true,
                Some(Source::Registry(RegistrySource { version: v })),
                None,
                None,
            ) => toml_edit::value(v),
            (false, None, true, Some(Source::Workspace(WorkspaceSource {})), None, None) => {
                let mut table = toml_edit::InlineTable::default();
                table.set_dotted(true);
                table.insert("workspace", true.into());
                toml_edit::value(toml_edit::Value::InlineTable(table))
            }
            // Other cases are represented as an inline table
            (_, _, _, _, _, _) => {
                let mut table = toml_edit::InlineTable::default();

                match &self.source {
                    Some(Source::Registry(src)) => {
                        table.insert("version", src.version.as_str().into());
                    }
                    Some(Source::Path(src)) => {
                        let relpath = path_field(crate_root, &src.path);
                        if let Some(r) = src.version.as_deref() {
                            table.insert("version", r.into());
                        }
                        table.insert("path", relpath.into());
                    }
                    Some(Source::Git(src)) => {
                        table.insert("git", src.git.as_str().into());
                        if let Some(branch) = src.branch.as_deref() {
                            table.insert("branch", branch.into());
                        }
                        if let Some(tag) = src.tag.as_deref() {
                            table.insert("tag", tag.into());
                        }
                        if let Some(rev) = src.rev.as_deref() {
                            table.insert("rev", rev.into());
                        }
                        if let Some(r) = src.version.as_deref() {
                            table.insert("version", r.into());
                        }
                    }
                    Some(Source::Workspace(_)) => {
                        table.insert("workspace", true.into());
                    }
                    None => {}
                }
                if table.contains_key("version") {
                    if let Some(r) = self.registry.as_deref() {
                        table.insert("registry", r.into());
                    }
                }

                if self.rename.is_some() {
                    table.insert("package", self.name.as_str().into());
                }
                if let Some(v) = self.default_features {
                    table.insert("default-features", v.into());
                }
                if let Some(features) = self.features.as_ref() {
                    let features: toml_edit::Value = features.iter().cloned().collect();
                    table.insert("features", features);
                }
                if let Some(v) = self.optional {
                    table.insert("optional", v.into());
                }

                toml_edit::value(toml_edit::Value::InlineTable(table))
            }
        };

        table
    }

    /// Modify existing entry to match this dependency.
    pub fn update_toml<'k>(
        &self,
        crate_root: &Path,
        key: &mut KeyMut<'k>,
        item: &mut toml_edit::Item,
    ) {
        if str_or_1_len_table(item) {
            // Nothing to preserve
            *item = self.to_toml(crate_root);
            key.fmt();
        } else if let Some(table) = item.as_table_like_mut() {
            match &self.source {
                Some(Source::Registry(src)) => {
                    overwrite_value(table, "version", src.version.as_str());

                    for key in ["path", "git", "branch", "tag", "rev", "workspace"] {
                        table.remove(key);
                    }
                }
                Some(Source::Path(src)) => {
                    let relpath = path_field(crate_root, &src.path);
                    overwrite_value(table, "path", relpath);
                    if let Some(r) = src.version.as_deref() {
                        overwrite_value(table, "version", r);
                    } else {
                        table.remove("version");
                    }

                    for key in ["git", "branch", "tag", "rev", "workspace"] {
                        table.remove(key);
                    }
                }
                Some(Source::Git(src)) => {
                    overwrite_value(table, "git", src.git.as_str());
                    if let Some(branch) = src.branch.as_deref() {
                        overwrite_value(table, "branch", branch);
                    } else {
                        table.remove("branch");
                    }
                    if let Some(tag) = src.tag.as_deref() {
                        overwrite_value(table, "tag", tag);
                    } else {
                        table.remove("tag");
                    }
                    if let Some(rev) = src.rev.as_deref() {
                        overwrite_value(table, "rev", rev);
                    } else {
                        table.remove("rev");
                    }
                    if let Some(r) = src.version.as_deref() {
                        overwrite_value(table, "version", r);
                    } else {
                        table.remove("version");
                    }

                    for key in ["path", "workspace"] {
                        table.remove(key);
                    }
                }
                Some(Source::Workspace(_)) => {
                    overwrite_value(table, "workspace", true);
                    table.set_dotted(true);
                    key.fmt();
                    for key in [
                        "version",
                        "registry",
                        "registry-index",
                        "path",
                        "git",
                        "branch",
                        "tag",
                        "rev",
                        "package",
                        "default-features",
                    ] {
                        table.remove(key);
                    }
                }
                None => {}
            }
            if table.contains_key("version") {
                if let Some(r) = self.registry.as_deref() {
                    overwrite_value(table, "registry", r);
                } else {
                    table.remove("registry");
                }
            } else {
                table.remove("registry");
            }

            if self.rename.is_some() {
                overwrite_value(table, "package", self.name.as_str());
            }
            match self.default_features {
                Some(v) => {
                    overwrite_value(table, "default-features", v);
                }
                None => {
                    table.remove("default-features");
                }
            }
            if let Some(new_features) = self.features.as_ref() {
                let mut features = table
                    .get("features")
                    .and_then(|i| i.as_value())
                    .and_then(|v| v.as_array())
                    .and_then(|a| {
                        a.iter()
                            .map(|v| v.as_str())
                            .collect::<Option<IndexSet<_>>>()
                    })
                    .unwrap_or_default();
                features.extend(new_features.iter().map(|s| s.as_str()));
                let features = features.into_iter().collect::<toml_edit::Value>();
                table.set_dotted(false);
                overwrite_value(table, "features", features);
            } else {
                table.remove("features");
            }
            match self.optional {
                Some(v) => {
                    table.set_dotted(false);
                    overwrite_value(table, "optional", v);
                }
                None => {
                    table.remove("optional");
                }
            }
        } else {
            unreachable!("Invalid dependency type: {}", item.type_name());
        }
    }
}

fn overwrite_value(
    table: &mut dyn toml_edit::TableLike,
    key: &str,
    value: impl Into<toml_edit::Value>,
) {
    let mut value = value.into();
    let existing = table.entry(key).or_insert_with(|| Default::default());
    if let Some(existing_value) = existing.as_value() {
        *value.decor_mut() = existing_value.decor().clone();
    }
    *existing = toml_edit::Item::Value(value);
}

fn invalid_type(dep: &str, key: &str, actual: &str, expected: &str) -> anyhow::Error {
    anyhow::format_err!("Found {actual} for {key} when {expected} was expected for {dep}")
}

impl std::fmt::Display for Dependency {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(source) = self.source() {
            write!(f, "{}@{}", self.name, source)
        } else {
            self.toml_key().fmt(f)
        }
    }
}

impl<'s> From<&'s Summary> for Dependency {
    fn from(other: &'s Summary) -> Self {
        let source: Source = if let Some(path) = other.source_id().local_path() {
            PathSource::new(path)
                .set_version(other.version().to_string())
                .into()
        } else if let Some(git_ref) = other.source_id().git_reference() {
            let mut src = GitSource::new(other.source_id().url().to_string())
                .set_version(other.version().to_string());
            match git_ref {
                GitReference::Branch(branch) => src = src.set_branch(branch),
                GitReference::Tag(tag) => src = src.set_tag(tag),
                GitReference::Rev(rev) => src = src.set_rev(rev),
                GitReference::DefaultBranch => {}
            }
            src.into()
        } else {
            RegistrySource::new(other.version().to_string()).into()
        };
        Dependency::new(other.name().as_str()).set_source(source)
    }
}

impl From<Summary> for Dependency {
    fn from(other: Summary) -> Self {
        (&other).into()
    }
}

fn path_field(crate_root: &Path, abs_path: &Path) -> String {
    let relpath = pathdiff::diff_paths(abs_path, crate_root).expect("both paths are absolute");
    let relpath = relpath.to_str().unwrap().replace('\\', "/");
    relpath
}

/// Primary location of a dependency.
#[derive(Debug, Hash, PartialEq, Eq, Clone)]
pub enum Source {
    /// Dependency from a registry.
    Registry(RegistrySource),
    /// Dependency from a local path.
    Path(PathSource),
    /// Dependency from a git repo.
    Git(GitSource),
    /// Dependency from a workspace.
    Workspace(WorkspaceSource),
}

impl Source {
    /// Access the registry source, if present.
    pub fn as_registry(&self) -> Option<&RegistrySource> {
        match self {
            Self::Registry(src) => Some(src),
            _ => None,
        }
    }

    /// Access the path source, if present.
    #[allow(dead_code)]
    pub fn as_path(&self) -> Option<&PathSource> {
        match self {
            Self::Path(src) => Some(src),
            _ => None,
        }
    }

    /// Access the git source, if present.
    #[allow(dead_code)]
    pub fn as_git(&self) -> Option<&GitSource> {
        match self {
            Self::Git(src) => Some(src),
            _ => None,
        }
    }

    /// Access the workspace source, if present.
    #[allow(dead_code)]
    pub fn as_workspace(&self) -> Option<&WorkspaceSource> {
        match self {
            Self::Workspace(src) => Some(src),
            _ => None,
        }
    }
}

impl std::fmt::Display for Source {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Registry(src) => src.fmt(f),
            Self::Path(src) => src.fmt(f),
            Self::Git(src) => src.fmt(f),
            Self::Workspace(src) => src.fmt(f),
        }
    }
}

impl<'s> From<&'s Source> for Source {
    fn from(inner: &'s Source) -> Self {
        inner.clone()
    }
}

impl From<RegistrySource> for Source {
    fn from(inner: RegistrySource) -> Self {
        Self::Registry(inner)
    }
}

impl From<PathSource> for Source {
    fn from(inner: PathSource) -> Self {
        Self::Path(inner)
    }
}

impl From<GitSource> for Source {
    fn from(inner: GitSource) -> Self {
        Self::Git(inner)
    }
}

impl From<WorkspaceSource> for Source {
    fn from(inner: WorkspaceSource) -> Self {
        Self::Workspace(inner)
    }
}

/// Dependency from a registry.
#[derive(Debug, Hash, PartialEq, Eq, Clone)]
#[non_exhaustive]
pub struct RegistrySource {
    /// Version requirement.
    pub version: String,
}

impl RegistrySource {
    /// Specify dependency by version requirement.
    pub fn new(version: impl AsRef<str>) -> Self {
        // versions might have semver metadata appended which we do not want to
        // store in the cargo toml files.  This would cause a warning upon compilation
        // ("version requirement […] includes semver metadata which will be ignored")
        let version = version.as_ref().split('+').next().unwrap();
        Self {
            version: version.to_owned(),
        }
    }
}

impl std::fmt::Display for RegistrySource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.version.fmt(f)
    }
}

/// Dependency from a local path.
#[derive(Debug, Hash, PartialEq, Eq, Clone)]
#[non_exhaustive]
pub struct PathSource {
    /// Local, absolute path.
    pub path: PathBuf,
    /// Version requirement for when published.
    pub version: Option<String>,
}

impl PathSource {
    /// Specify dependency from a path.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            version: None,
        }
    }

    /// Set an optional version requirement.
    pub fn set_version(mut self, version: impl AsRef<str>) -> Self {
        // versions might have semver metadata appended which we do not want to
        // store in the cargo toml files.  This would cause a warning upon compilation
        // ("version requirement […] includes semver metadata which will be ignored")
        let version = version.as_ref().split('+').next().unwrap();
        self.version = Some(version.to_owned());
        self
    }

    /// Get the SourceID for this dependency.
    pub fn source_id(&self) -> CargoResult<SourceId> {
        SourceId::for_path(&self.path)
    }
}

impl std::fmt::Display for PathSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.path.display().fmt(f)
    }
}

/// Dependency from a git repo.
#[derive(Debug, Hash, PartialEq, Eq, Clone)]
#[non_exhaustive]
pub struct GitSource {
    /// Repository URL.
    pub git: String,
    /// Select specific branch.
    pub branch: Option<String>,
    /// Select specific tag.
    pub tag: Option<String>,
    /// Select specific rev.
    pub rev: Option<String>,
    /// Version requirement for when published.
    pub version: Option<String>,
}

impl GitSource {
    /// Specify dependency from a git repo.
    pub fn new(git: impl Into<String>) -> Self {
        Self {
            git: git.into(),
            branch: None,
            tag: None,
            rev: None,
            version: None,
        }
    }

    /// Specify an optional branch.
    pub fn set_branch(mut self, branch: impl Into<String>) -> Self {
        self.branch = Some(branch.into());
        self.tag = None;
        self.rev = None;
        self
    }

    /// Specify an optional tag.
    pub fn set_tag(mut self, tag: impl Into<String>) -> Self {
        self.branch = None;
        self.tag = Some(tag.into());
        self.rev = None;
        self
    }

    /// Specify an optional rev.
    pub fn set_rev(mut self, rev: impl Into<String>) -> Self {
        self.branch = None;
        self.tag = None;
        self.rev = Some(rev.into());
        self
    }

    /// Get the SourceID for this dependency.
    pub fn source_id(&self) -> CargoResult<SourceId> {
        let git_url = self.git.parse::<url::Url>()?;
        let git_ref = self.git_ref();
        SourceId::for_git(&git_url, git_ref)
    }

    fn git_ref(&self) -> GitReference {
        match (
            self.branch.as_deref(),
            self.tag.as_deref(),
            self.rev.as_deref(),
        ) {
            (Some(branch), _, _) => GitReference::Branch(branch.to_owned()),
            (_, Some(tag), _) => GitReference::Tag(tag.to_owned()),
            (_, _, Some(rev)) => GitReference::Rev(rev.to_owned()),
            _ => GitReference::DefaultBranch,
        }
    }

    /// Set an optional version requirement.
    pub fn set_version(mut self, version: impl AsRef<str>) -> Self {
        // versions might have semver metadata appended which we do not want to
        // store in the cargo toml files.  This would cause a warning upon compilation
        // ("version requirement […] includes semver metadata which will be ignored")
        let version = version.as_ref().split('+').next().unwrap();
        self.version = Some(version.to_owned());
        self
    }
}

impl std::fmt::Display for GitSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let git_ref = self.git_ref();

        // TODO(-Znext-lockfile-bump): set it to true when stabilizing
        // lockfile v4, because we want Source ID serialization to be
        // consistent with lockfile.
        if let Some(pretty_ref) = git_ref.pretty_ref(false) {
            write!(f, "{}?{}", self.git, pretty_ref)
        } else {
            write!(f, "{}", self.git)
        }
    }
}

/// Dependency from a workspace.
#[derive(Debug, Hash, PartialEq, Eq, Clone)]
#[non_exhaustive]
pub struct WorkspaceSource;

impl WorkspaceSource {
    pub fn new() -> Self {
        Self
    }
}

impl Display for WorkspaceSource {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        "workspace".fmt(f)
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::util::toml_mut::manifest::LocalManifest;
    use cargo_util::paths;

    use super::*;

    #[test]
    fn to_toml_simple_dep() {
        let crate_root =
            paths::normalize_path(&std::env::current_dir().unwrap().join(Path::new("/")));
        let dep = Dependency::new("dep").set_source(RegistrySource::new("1.0"));
        let key = dep.toml_key();
        let item = dep.to_toml(&crate_root);

        assert_eq!(key, "dep".to_owned());

        verify_roundtrip(&crate_root, key, &item);
    }

    #[test]
    fn to_toml_simple_dep_with_version() {
        let crate_root =
            paths::normalize_path(&std::env::current_dir().unwrap().join(Path::new("/")));
        let dep = Dependency::new("dep").set_source(RegistrySource::new("1.0"));
        let key = dep.toml_key();
        let item = dep.to_toml(&crate_root);

        assert_eq!(key, "dep".to_owned());
        assert_eq!(item.as_str(), Some("1.0"));

        verify_roundtrip(&crate_root, key, &item);
    }

    #[test]
    fn to_toml_optional_dep() {
        let crate_root =
            paths::normalize_path(&std::env::current_dir().unwrap().join(Path::new("/")));
        let dep = Dependency::new("dep")
            .set_source(RegistrySource::new("1.0"))
            .set_optional(true);
        let key = dep.toml_key();
        let item = dep.to_toml(&crate_root);

        assert_eq!(key, "dep".to_owned());
        assert!(item.is_inline_table());

        let dep = item.as_inline_table().unwrap();
        assert_eq!(dep.get("optional").unwrap().as_bool(), Some(true));

        verify_roundtrip(&crate_root, key, &item);
    }

    #[test]
    fn to_toml_dep_without_default_features() {
        let crate_root =
            paths::normalize_path(&std::env::current_dir().unwrap().join(Path::new("/")));
        let dep = Dependency::new("dep")
            .set_source(RegistrySource::new("1.0"))
            .set_default_features(false);
        let key = dep.toml_key();
        let item = dep.to_toml(&crate_root);

        assert_eq!(key, "dep".to_owned());
        assert!(item.is_inline_table());

        let dep = item.as_inline_table().unwrap();
        assert_eq!(dep.get("default-features").unwrap().as_bool(), Some(false));

        verify_roundtrip(&crate_root, key, &item);
    }

    #[test]
    fn to_toml_dep_with_path_source() {
        let root = paths::normalize_path(&std::env::current_dir().unwrap().join(Path::new("/")));
        let crate_root = root.join("foo");
        let dep = Dependency::new("dep").set_source(PathSource::new(root.join("bar")));
        let key = dep.toml_key();
        let item = dep.to_toml(&crate_root);

        assert_eq!(key, "dep".to_owned());
        assert!(item.is_inline_table());

        let dep = item.as_inline_table().unwrap();
        assert_eq!(dep.get("path").unwrap().as_str(), Some("../bar"));

        verify_roundtrip(&crate_root, key, &item);
    }

    #[test]
    fn to_toml_dep_with_git_source() {
        let crate_root =
            paths::normalize_path(&std::env::current_dir().unwrap().join(Path::new("/")));
        let dep = Dependency::new("dep").set_source(GitSource::new("https://foor/bar.git"));
        let key = dep.toml_key();
        let item = dep.to_toml(&crate_root);

        assert_eq!(key, "dep".to_owned());
        assert!(item.is_inline_table());

        let dep = item.as_inline_table().unwrap();
        assert_eq!(
            dep.get("git").unwrap().as_str(),
            Some("https://foor/bar.git")
        );

        verify_roundtrip(&crate_root, key, &item);
    }

    #[test]
    fn to_toml_renamed_dep() {
        let crate_root =
            paths::normalize_path(&std::env::current_dir().unwrap().join(Path::new("/")));
        let dep = Dependency::new("dep")
            .set_source(RegistrySource::new("1.0"))
            .set_rename("d");
        let key = dep.toml_key();
        let item = dep.to_toml(&crate_root);

        assert_eq!(key, "d".to_owned());
        assert!(item.is_inline_table());

        let dep = item.as_inline_table().unwrap();
        assert_eq!(dep.get("package").unwrap().as_str(), Some("dep"));

        verify_roundtrip(&crate_root, key, &item);
    }

    #[test]
    fn to_toml_dep_from_alt_registry() {
        let crate_root =
            paths::normalize_path(&std::env::current_dir().unwrap().join(Path::new("/")));
        let dep = Dependency::new("dep")
            .set_source(RegistrySource::new("1.0"))
            .set_registry("alternative");
        let key = dep.toml_key();
        let item = dep.to_toml(&crate_root);

        assert_eq!(key, "dep".to_owned());
        assert!(item.is_inline_table());

        let dep = item.as_inline_table().unwrap();
        assert_eq!(dep.get("registry").unwrap().as_str(), Some("alternative"));

        verify_roundtrip(&crate_root, key, &item);
    }

    #[test]
    fn to_toml_complex_dep() {
        let crate_root =
            paths::normalize_path(&std::env::current_dir().unwrap().join(Path::new("/")));
        let dep = Dependency::new("dep")
            .set_source(RegistrySource::new("1.0"))
            .set_default_features(false)
            .set_rename("d");
        let key = dep.toml_key();
        let item = dep.to_toml(&crate_root);

        assert_eq!(key, "d".to_owned());
        assert!(item.is_inline_table());

        let dep = item.as_inline_table().unwrap();
        assert_eq!(dep.get("package").unwrap().as_str(), Some("dep"));
        assert_eq!(dep.get("version").unwrap().as_str(), Some("1.0"));
        assert_eq!(dep.get("default-features").unwrap().as_bool(), Some(false));

        verify_roundtrip(&crate_root, key, &item);
    }

    #[test]
    fn paths_with_forward_slashes_are_left_as_is() {
        let crate_root =
            paths::normalize_path(&std::env::current_dir().unwrap().join(Path::new("/")));
        let path = crate_root.join("sibling/crate");
        let relpath = "sibling/crate";
        let dep = Dependency::new("dep").set_source(PathSource::new(path));
        let key = dep.toml_key();
        let item = dep.to_toml(&crate_root);

        let table = item.as_inline_table().unwrap();
        let got = table.get("path").unwrap().as_str().unwrap();
        assert_eq!(got, relpath);

        verify_roundtrip(&crate_root, key, &item);
    }

    #[test]
    fn overwrite_with_workspace_source_fmt_key() {
        let crate_root =
            paths::normalize_path(&std::env::current_dir().unwrap().join(Path::new("./")));
        let toml = "dep = \"1.0\"\n";
        let manifest = toml.parse().unwrap();
        let mut local = LocalManifest {
            path: crate_root.clone(),
            manifest,
        };
        assert_eq!(local.manifest.to_string(), toml);
        for (key, item) in local.data.clone().iter() {
            let dep = Dependency::from_toml(&crate_root, key, item).unwrap();
            let dep = dep.set_source(WorkspaceSource::new());
            local.insert_into_table(&vec![], &dep).unwrap();
            assert_eq!(local.data.to_string(), "dep.workspace = true\n");
        }
    }

    #[test]
    #[cfg(windows)]
    fn normalise_windows_style_paths() {
        let crate_root =
            paths::normalize_path(&std::env::current_dir().unwrap().join(Path::new("/")));
        let original = crate_root.join(r"sibling\crate");
        let should_be = "sibling/crate";
        let dep = Dependency::new("dep").set_source(PathSource::new(original));
        let key = dep.toml_key();
        let item = dep.to_toml(&crate_root);

        let table = item.as_inline_table().unwrap();
        let got = table.get("path").unwrap().as_str().unwrap();
        assert_eq!(got, should_be);

        verify_roundtrip(&crate_root, key, &item);
    }

    #[track_caller]
    fn verify_roundtrip(crate_root: &Path, key: &str, item: &toml_edit::Item) {
        let roundtrip = Dependency::from_toml(crate_root, key, item).unwrap();
        let round_key = roundtrip.toml_key();
        let round_item = roundtrip.to_toml(crate_root);
        assert_eq!(key, round_key);
        assert_eq!(item.to_string(), round_item.to_string());
    }
}
