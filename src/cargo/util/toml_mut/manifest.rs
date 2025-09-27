//! Parsing and editing of manifest files.

use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};
use std::str;

use anyhow::Context as _;

use super::dependency::Dependency;
use crate::core::dependency::DepKind;
use crate::core::{FeatureValue, Features, Workspace};
use crate::util::closest;
use crate::util::frontmatter::ScriptSource;
use crate::util::toml::is_embedded;
use crate::{CargoResult, GlobalContext};

/// Dependency table to add deps to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DepTable {
    kind: DepKind,
    target: Option<String>,
}

impl DepTable {
    const KINDS: &'static [Self] = &[
        Self::new().set_kind(DepKind::Normal),
        Self::new().set_kind(DepKind::Development),
        Self::new().set_kind(DepKind::Build),
    ];

    /// Reference to a Dependency Table.
    pub const fn new() -> Self {
        Self {
            kind: DepKind::Normal,
            target: None,
        }
    }

    /// Choose the type of dependency.
    pub const fn set_kind(mut self, kind: DepKind) -> Self {
        self.kind = kind;
        self
    }

    /// Choose the platform for the dependency.
    pub fn set_target(mut self, target: impl Into<String>) -> Self {
        self.target = Some(target.into());
        self
    }

    /// Type of dependency.
    pub fn kind(&self) -> DepKind {
        self.kind
    }

    /// Platform for the dependency.
    pub fn target(&self) -> Option<&str> {
        self.target.as_deref()
    }

    /// Keys to the table.
    pub fn to_table(&self) -> Vec<&str> {
        if let Some(target) = &self.target {
            vec!["target", target, self.kind.kind_table()]
        } else {
            vec![self.kind.kind_table()]
        }
    }
}

impl Default for DepTable {
    fn default() -> Self {
        Self::new()
    }
}

impl From<DepKind> for DepTable {
    fn from(other: DepKind) -> Self {
        Self::new().set_kind(other)
    }
}

/// An editable Cargo manifest.
#[derive(Debug, Clone)]
pub struct Manifest {
    /// Manifest contents as TOML data.
    pub data: toml_edit::DocumentMut,
}

impl Manifest {
    /// Get the manifest's package name.
    pub fn package_name(&self) -> CargoResult<&str> {
        self.data
            .as_table()
            .get("package")
            .and_then(|m| m.get("name"))
            .and_then(|m| m.as_str())
            .ok_or_else(parse_manifest_err)
    }

    /// Get the specified table from the manifest.
    pub fn get_table<'a>(&'a self, table_path: &[String]) -> CargoResult<&'a toml_edit::Item> {
        /// Descend into a manifest until the required table is found.
        fn descend<'a>(
            input: &'a toml_edit::Item,
            path: &[String],
        ) -> CargoResult<&'a toml_edit::Item> {
            if let Some(segment) = path.get(0) {
                let value = input
                    .get(&segment)
                    .ok_or_else(|| non_existent_table_err(segment))?;

                if value.is_table_like() {
                    descend(value, &path[1..])
                } else {
                    Err(non_existent_table_err(segment))
                }
            } else {
                Ok(input)
            }
        }

        descend(self.data.as_item(), table_path)
    }

    /// Get the specified table from the manifest.
    pub fn get_table_mut<'a>(
        &'a mut self,
        table_path: &[String],
    ) -> CargoResult<&'a mut toml_edit::Item> {
        /// Descend into a manifest until the required table is found.
        fn descend<'a>(
            input: &'a mut toml_edit::Item,
            path: &[String],
        ) -> CargoResult<&'a mut toml_edit::Item> {
            if let Some(segment) = path.get(0) {
                let mut default_table = toml_edit::Table::new();
                default_table.set_implicit(true);
                let value = input[&segment].or_insert(toml_edit::Item::Table(default_table));

                if value.is_table_like() {
                    descend(value, &path[1..])
                } else {
                    Err(non_existent_table_err(segment))
                }
            } else {
                Ok(input)
            }
        }

        descend(self.data.as_item_mut(), table_path)
    }

    /// Get all sections in the manifest that exist and might contain
    /// dependencies. The returned items are always `Table` or
    /// `InlineTable`.
    pub fn get_sections(&self) -> Vec<(DepTable, toml_edit::Item)> {
        let mut sections = Vec::new();

        for table in DepTable::KINDS {
            let dependency_type = table.kind.kind_table();
            // Dependencies can be in the three standard sections...
            if self
                .data
                .get(dependency_type)
                .map(|t| t.is_table_like())
                .unwrap_or(false)
            {
                sections.push((table.clone(), self.data[dependency_type].clone()))
            }

            // ... and in `target.<target>.(build-/dev-)dependencies`.
            let target_sections = self
                .data
                .as_table()
                .get("target")
                .and_then(toml_edit::Item::as_table_like)
                .into_iter()
                .flat_map(toml_edit::TableLike::iter)
                .filter_map(|(target_name, target_table)| {
                    let dependency_table = target_table.get(dependency_type)?;
                    dependency_table.as_table_like().map(|_| {
                        (
                            table.clone().set_target(target_name),
                            dependency_table.clone(),
                        )
                    })
                });

            sections.extend(target_sections);
        }

        sections
    }

    pub fn get_legacy_sections(&self) -> Vec<String> {
        let mut result = Vec::new();

        for dependency_type in ["dev_dependencies", "build_dependencies"] {
            if self.data.contains_key(dependency_type) {
                result.push(dependency_type.to_owned());
            }

            // ... and in `target.<target>.(build-/dev-)dependencies`.
            result.extend(
                self.data
                    .as_table()
                    .get("target")
                    .and_then(toml_edit::Item::as_table_like)
                    .into_iter()
                    .flat_map(toml_edit::TableLike::iter)
                    .filter_map(|(target_name, target_table)| {
                        if target_table.as_table_like()?.contains_key(dependency_type) {
                            Some(format!("target.{target_name}.{dependency_type}"))
                        } else {
                            None
                        }
                    }),
            );
        }
        result
    }
}

impl str::FromStr for Manifest {
    type Err = anyhow::Error;

    /// Read manifest data from string
    fn from_str(input: &str) -> ::std::result::Result<Self, Self::Err> {
        let d: toml_edit::DocumentMut = input.parse().context("Manifest not valid TOML")?;

        Ok(Manifest { data: d })
    }
}

impl std::fmt::Display for Manifest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.data.fmt(f)
    }
}

/// An editable Cargo manifest that is available locally.
#[derive(Debug, Clone)]
pub struct LocalManifest {
    /// Path to the manifest.
    pub path: PathBuf,
    /// Manifest contents.
    pub manifest: Manifest,
    /// The raw, unparsed package file
    pub raw: String,
    /// Edit location for an embedded manifest, if relevant
    pub embedded: Option<Embedded>,
}

impl Deref for LocalManifest {
    type Target = Manifest;

    fn deref(&self) -> &Manifest {
        &self.manifest
    }
}

impl DerefMut for LocalManifest {
    fn deref_mut(&mut self) -> &mut Manifest {
        &mut self.manifest
    }
}

impl LocalManifest {
    /// Construct the `LocalManifest` corresponding to the `Path` provided..
    pub fn try_new(path: &Path) -> CargoResult<Self> {
        if !path.is_absolute() {
            anyhow::bail!("can only edit absolute paths, got {}", path.display());
        }
        let raw = cargo_util::paths::read(&path)?;
        let mut data = raw.clone();
        let mut embedded = None;
        if is_embedded(path) {
            let source = ScriptSource::parse(&data)?;
            if let Some(frontmatter) = source.frontmatter_span() {
                embedded = Some(Embedded::exists(frontmatter));
                data = source.frontmatter().unwrap().to_owned();
            } else if let Some(shebang) = source.shebang_span() {
                embedded = Some(Embedded::after(shebang));
                data = String::new();
            } else {
                embedded = Some(Embedded::start());
                data = String::new();
            }
        }
        let manifest = data.parse().context("Unable to parse Cargo.toml")?;
        Ok(LocalManifest {
            manifest,
            path: path.to_owned(),
            raw,
            embedded,
        })
    }

    /// Write changes back to the file.
    pub fn write(&self) -> CargoResult<()> {
        let mut manifest = self.manifest.data.to_string();
        let raw = match self.embedded.as_ref() {
            Some(Embedded::Implicit(start)) => {
                if !manifest.ends_with("\n") {
                    manifest.push_str("\n");
                }
                let fence = "---\n";
                let prefix = &self.raw[0..*start];
                let suffix = &self.raw[*start..];
                let empty_line = if prefix.is_empty() { "\n" } else { "" };
                format!("{prefix}{fence}{manifest}{fence}{empty_line}{suffix}")
            }
            Some(Embedded::Explicit(span)) => {
                if !manifest.ends_with("\n") {
                    manifest.push_str("\n");
                }
                let prefix = &self.raw[0..span.start];
                let suffix = &self.raw[span.end..];
                format!("{prefix}{manifest}{suffix}")
            }
            None => manifest,
        };
        let new_contents_bytes = raw.as_bytes();

        cargo_util::paths::write_atomic(&self.path, new_contents_bytes)
    }

    /// Lookup a dependency.
    pub fn get_dependencies<'s>(
        &'s self,
        ws: &'s Workspace<'_>,
        unstable_features: &'s Features,
    ) -> impl Iterator<Item = (String, DepTable, CargoResult<Dependency>)> + 's {
        let crate_root = self.path.parent().expect("manifest path is absolute");
        self.get_sections()
            .into_iter()
            .filter_map(move |(table_path, table)| {
                let table = table.into_table().ok()?;
                Some(
                    table
                        .into_iter()
                        .map(|(key, item)| (table_path.clone(), key, item))
                        .collect::<Vec<_>>(),
                )
            })
            .flatten()
            .map(move |(table_path, dep_key, dep_item)| {
                let dep = Dependency::from_toml(
                    ws.gctx(),
                    ws.root(),
                    crate_root,
                    unstable_features,
                    &dep_key,
                    &dep_item,
                );
                (dep_key, table_path, dep)
            })
    }

    /// Add entry to a Cargo.toml.
    pub fn insert_into_table(
        &mut self,
        table_path: &[String],
        dep: &Dependency,
        gctx: &GlobalContext,
        workspace_root: &Path,
        unstable_features: &Features,
    ) -> CargoResult<()> {
        let crate_root = self
            .path
            .parent()
            .expect("manifest path is absolute")
            .to_owned();
        let dep_key = dep.toml_key();

        let table = self.get_table_mut(table_path)?;
        if let Some((mut dep_key, dep_item)) = table
            .as_table_like_mut()
            .unwrap()
            .get_key_value_mut(dep_key)
        {
            dep.update_toml(
                gctx,
                workspace_root,
                &crate_root,
                unstable_features,
                &mut dep_key,
                dep_item,
            )?;
            if let Some(table) = dep_item.as_inline_table_mut() {
                // So long as we don't have `Cargo.toml` auto-formatting and inline-tables can only
                // be on one line, there isn't really much in the way of interesting formatting to
                // include (no comments), so let's just wipe it clean
                table.fmt();
            }
        } else {
            let new_dependency =
                dep.to_toml(gctx, workspace_root, &crate_root, unstable_features)?;
            table[dep_key] = new_dependency;
        }

        Ok(())
    }

    /// Remove entry from a Cargo.toml.
    pub fn remove_from_table(&mut self, table_path: &[String], name: &str) -> CargoResult<()> {
        let parent_table = self.get_table_mut(table_path)?;

        match parent_table.get_mut(name).filter(|t| !t.is_none()) {
            Some(dep) => {
                // remove the dependency
                *dep = toml_edit::Item::None;

                // remove table if empty
                if parent_table.as_table_like().unwrap().is_empty() {
                    *parent_table = toml_edit::Item::None;
                }
            }
            None => {
                let names = parent_table
                    .as_table_like()
                    .map(|t| t.iter())
                    .into_iter()
                    .flatten();
                let alt_name = closest(name, names.map(|(k, _)| k), |k| k).map(|n| n.to_owned());

                // Search in other tables.
                let sections = self.get_sections();
                let found_table_path = sections.iter().find_map(|(t, i)| {
                    let table_path: Vec<String> =
                        t.to_table().iter().map(|s| s.to_string()).collect();
                    i.get(name).is_some().then(|| table_path.join("."))
                });

                return Err(non_existent_dependency_err(
                    name,
                    table_path.join("."),
                    found_table_path,
                    alt_name.as_deref(),
                ));
            }
        }

        Ok(())
    }

    /// Allow mutating dependencies, wherever they live.
    /// Copied from cargo-edit.
    pub fn get_dependency_tables_mut(
        &mut self,
    ) -> impl Iterator<Item = &mut dyn toml_edit::TableLike> + '_ {
        let root = self.data.as_table_mut();
        root.iter_mut().flat_map(|(k, v)| {
            if DepTable::KINDS
                .iter()
                .any(|dt| dt.kind.kind_table() == k.get())
            {
                v.as_table_like_mut().into_iter().collect::<Vec<_>>()
            } else if k == "workspace" {
                v.as_table_like_mut()
                    .unwrap()
                    .iter_mut()
                    .filter_map(|(k, v)| {
                        if k.get() == "dependencies" {
                            v.as_table_like_mut()
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>()
            } else if k == "target" {
                v.as_table_like_mut()
                    .unwrap()
                    .iter_mut()
                    .flat_map(|(_, v)| {
                        v.as_table_like_mut().into_iter().flat_map(|v| {
                            v.iter_mut().filter_map(|(k, v)| {
                                if DepTable::KINDS
                                    .iter()
                                    .any(|dt| dt.kind.kind_table() == k.get())
                                {
                                    v.as_table_like_mut()
                                } else {
                                    None
                                }
                            })
                        })
                    })
                    .collect::<Vec<_>>()
            } else {
                Vec::new()
            }
        })
    }

    /// Remove references to `dep_key` if its no longer present.
    pub fn gc_dep(&mut self, dep_key: &str) {
        let explicit_dep_activation = self.is_explicit_dep_activation(dep_key);
        let status = self.dep_status(dep_key);

        if let Some(toml_edit::Item::Table(feature_table)) =
            self.data.as_table_mut().get_mut("features")
        {
            for (_feature, mut feature_values) in feature_table.iter_mut() {
                if let toml_edit::Item::Value(toml_edit::Value::Array(feature_values)) =
                    &mut feature_values
                {
                    fix_feature_activations(
                        feature_values,
                        dep_key,
                        status,
                        explicit_dep_activation,
                    );
                }
            }
        }
    }

    pub fn is_explicit_dep_activation(&self, dep_key: &str) -> bool {
        if let Some(toml_edit::Item::Table(feature_table)) = self.data.as_table().get("features") {
            for values in feature_table
                .iter()
                .map(|(_, a)| a)
                .filter_map(|i| i.as_value())
                .filter_map(|v| v.as_array())
            {
                for value in values.iter().filter_map(|v| v.as_str()) {
                    let value = FeatureValue::new(value.into());
                    if let FeatureValue::Dep { dep_name } = &value {
                        if dep_name.as_str() == dep_key {
                            return true;
                        }
                    }
                }
            }
        }

        false
    }

    fn dep_status(&self, dep_key: &str) -> DependencyStatus {
        let mut status = DependencyStatus::None;
        for (_, tbl) in self.get_sections() {
            if let toml_edit::Item::Table(tbl) = tbl {
                if let Some(dep_item) = tbl.get(dep_key) {
                    let optional = dep_item
                        .get("optional")
                        .and_then(|i| i.as_value())
                        .and_then(|i| i.as_bool())
                        .unwrap_or(false);
                    if optional {
                        return DependencyStatus::Optional;
                    } else {
                        status = DependencyStatus::Required;
                    }
                }
            }
        }
        status
    }
}

impl std::fmt::Display for LocalManifest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.manifest.fmt(f)
    }
}

/// Edit location for an embedded manifest
#[derive(Clone, Debug)]
pub enum Embedded {
    /// Manifest is implicit
    ///
    /// This is the insert location for a frontmatter
    Implicit(usize),
    /// Manifest is explicit in a frontmatter
    ///
    /// This is the span of the frontmatter body
    Explicit(std::ops::Range<usize>),
}

impl Embedded {
    fn start() -> Self {
        Self::Implicit(0)
    }

    fn after(after: std::ops::Range<usize>) -> Self {
        Self::Implicit(after.end)
    }

    fn exists(exists: std::ops::Range<usize>) -> Self {
        Self::Explicit(exists)
    }
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum DependencyStatus {
    None,
    Optional,
    Required,
}

fn fix_feature_activations(
    feature_values: &mut toml_edit::Array,
    dep_key: &str,
    status: DependencyStatus,
    explicit_dep_activation: bool,
) {
    let remove_list: Vec<usize> = feature_values
        .iter()
        .enumerate()
        .filter_map(|(idx, value)| value.as_str().map(|s| (idx, s)))
        .filter_map(|(idx, value)| {
            let parsed_value = FeatureValue::new(value.into());
            match status {
                DependencyStatus::None => match (parsed_value, explicit_dep_activation) {
                    (FeatureValue::Feature(dep_name), false)
                    | (FeatureValue::Dep { dep_name }, _)
                    | (FeatureValue::DepFeature { dep_name, .. }, _) => dep_name == dep_key,
                    _ => false,
                },
                DependencyStatus::Optional => false,
                DependencyStatus::Required => match (parsed_value, explicit_dep_activation) {
                    (FeatureValue::Feature(dep_name), false)
                    | (FeatureValue::Dep { dep_name }, _) => dep_name == dep_key,
                    (FeatureValue::Feature(_), true) | (FeatureValue::DepFeature { .. }, _) => {
                        false
                    }
                },
            }
            .then(|| idx)
        })
        .collect();

    // Remove found idx in revers order so we don't invalidate the idx.
    for idx in remove_list.iter().rev() {
        remove_array_index(feature_values, *idx);
    }

    if status == DependencyStatus::Required {
        for value in feature_values.iter_mut() {
            let parsed_value = if let Some(value) = value.as_str() {
                FeatureValue::new(value.into())
            } else {
                continue;
            };
            if let FeatureValue::DepFeature {
                dep_name,
                dep_feature,
                weak,
            } = parsed_value
            {
                if dep_name == dep_key && weak {
                    let mut new_value = toml_edit::Value::from(format!("{dep_name}/{dep_feature}"));
                    *new_value.decor_mut() = value.decor().clone();
                    *value = new_value;
                }
            }
        }
    }
}

pub fn str_or_1_len_table(item: &toml_edit::Item) -> bool {
    item.is_str() || item.as_table_like().map(|t| t.len() == 1).unwrap_or(false)
}

fn parse_manifest_err() -> anyhow::Error {
    anyhow::format_err!("unable to parse external Cargo.toml")
}

fn non_existent_table_err(table: impl std::fmt::Display) -> anyhow::Error {
    anyhow::format_err!("the table `{table}` could not be found.")
}

fn non_existent_dependency_err(
    name: impl std::fmt::Display,
    search_table: impl std::fmt::Display,
    found_table: Option<impl std::fmt::Display>,
    alt_name: Option<&str>,
) -> anyhow::Error {
    let mut msg = format!("the dependency `{name}` could not be found in `{search_table}`");
    if let Some(found_table) = found_table {
        msg.push_str(&format!("; it is present in `{found_table}`",));
    } else if let Some(alt_name) = alt_name {
        msg.push_str(&format!("; dependency `{alt_name}` exists",));
    }
    anyhow::format_err!(msg)
}

fn remove_array_index(array: &mut toml_edit::Array, index: usize) {
    let value = array.remove(index);

    // Captures all lines before leading whitespace
    let prefix_lines = value
        .decor()
        .prefix()
        .and_then(|p| p.as_str().expect("spans removed").rsplit_once('\n'))
        .map(|(lines, _current)| lines);
    // Captures all lines after trailing whitespace, before the next comma
    let suffix_lines = value
        .decor()
        .suffix()
        .and_then(|p| p.as_str().expect("spans removed").split_once('\n'))
        .map(|(_current, lines)| lines);
    let mut merged_lines = String::new();
    if let Some(prefix_lines) = prefix_lines {
        merged_lines.push_str(prefix_lines);
        merged_lines.push('\n');
    }
    if let Some(suffix_lines) = suffix_lines {
        merged_lines.push_str(suffix_lines);
        merged_lines.push('\n');
    }

    let next_index = index; // Since `index` was removed, that effectively auto-advances us
    if let Some(next) = array.get_mut(next_index) {
        let next_decor = next.decor_mut();
        let next_prefix = next_decor
            .prefix()
            .map(|s| s.as_str().expect("spans removed"))
            .unwrap_or_default();
        merged_lines.push_str(next_prefix);
        next_decor.set_prefix(merged_lines);
    } else {
        let trailing = array.trailing().as_str().expect("spans removed");
        merged_lines.push_str(trailing);
        array.set_trailing(merged_lines);
    }
}
