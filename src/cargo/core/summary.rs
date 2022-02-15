use crate::core::{Dependency, PackageId, SourceId};
use crate::util::interning::InternedString;
use crate::util::{CargoResult, Config};
use anyhow::bail;
use semver::Version;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt;
use std::hash::{Hash, Hasher};
use std::mem;
use std::rc::Rc;

/// Subset of a `Manifest`. Contains only the most important information about
/// a package.
///
/// Summaries are cloned, and should not be mutated after creation
#[derive(Debug, Clone)]
pub struct Summary {
    inner: Rc<Inner>,
}

#[derive(Debug, Clone)]
struct Inner {
    package_id: PackageId,
    dependencies: Vec<Dependency>,
    features: Rc<FeatureMap>,
    checksum: Option<String>,
    links: Option<InternedString>,
}

impl Summary {
    pub fn new(
        config: &Config,
        pkg_id: PackageId,
        dependencies: Vec<Dependency>,
        features: &BTreeMap<InternedString, Vec<InternedString>>,
        links: Option<impl Into<InternedString>>,
    ) -> CargoResult<Summary> {
        // ****CAUTION**** If you change anything here that may raise a new
        // error, be sure to coordinate that change with either the index
        // schema field or the SummariesCache version.
        for dep in dependencies.iter() {
            let dep_name = dep.name_in_toml();
            if dep.is_optional() && !dep.is_transitive() {
                bail!(
                    "dev-dependencies are not allowed to be optional: `{}`",
                    dep_name
                )
            }
        }
        let feature_map = build_feature_map(config, pkg_id, features, &dependencies)?;
        Ok(Summary {
            inner: Rc::new(Inner {
                package_id: pkg_id,
                dependencies,
                features: Rc::new(feature_map),
                checksum: None,
                links: links.map(|l| l.into()),
            }),
        })
    }

    pub fn package_id(&self) -> PackageId {
        self.inner.package_id
    }
    pub fn name(&self) -> InternedString {
        self.package_id().name()
    }
    pub fn version(&self) -> &Version {
        self.package_id().version()
    }
    pub fn source_id(&self) -> SourceId {
        self.package_id().source_id()
    }
    pub fn dependencies(&self) -> &[Dependency] {
        &self.inner.dependencies
    }
    pub fn features(&self) -> &FeatureMap {
        &self.inner.features
    }

    pub fn checksum(&self) -> Option<&str> {
        self.inner.checksum.as_deref()
    }
    pub fn links(&self) -> Option<InternedString> {
        self.inner.links
    }

    pub fn override_id(mut self, id: PackageId) -> Summary {
        Rc::make_mut(&mut self.inner).package_id = id;
        self
    }

    pub fn set_checksum(&mut self, cksum: String) {
        Rc::make_mut(&mut self.inner).checksum = Some(cksum);
    }

    pub fn map_dependencies<F>(mut self, f: F) -> Summary
    where
        F: FnMut(Dependency) -> Dependency,
    {
        {
            let slot = &mut Rc::make_mut(&mut self.inner).dependencies;
            *slot = mem::take(slot).into_iter().map(f).collect();
        }
        self
    }

    pub fn map_source(self, to_replace: SourceId, replace_with: SourceId) -> Summary {
        let me = if self.package_id().source_id() == to_replace {
            let new_id = self.package_id().with_source_id(replace_with);
            self.override_id(new_id)
        } else {
            self
        };
        me.map_dependencies(|dep| dep.map_source(to_replace, replace_with))
    }
}

impl PartialEq for Summary {
    fn eq(&self, other: &Summary) -> bool {
        self.inner.package_id == other.inner.package_id
    }
}

impl Eq for Summary {}

impl Hash for Summary {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.inner.package_id.hash(state);
    }
}

/// Checks features for errors, bailing out a CargoResult:Err if invalid,
/// and creates FeatureValues for each feature.
fn build_feature_map(
    config: &Config,
    pkg_id: PackageId,
    features: &BTreeMap<InternedString, Vec<InternedString>>,
    dependencies: &[Dependency],
) -> CargoResult<FeatureMap> {
    use self::FeatureValue::*;
    let mut dep_map = HashMap::new();
    for dep in dependencies.iter() {
        dep_map
            .entry(dep.name_in_toml())
            .or_insert_with(Vec::new)
            .push(dep);
    }

    let mut map: FeatureMap = features
        .iter()
        .map(|(feature, list)| {
            let fvs: Vec<_> = list
                .iter()
                .map(|feat_value| FeatureValue::new(*feat_value))
                .collect();
            (*feature, fvs)
        })
        .collect();

    // Add implicit features for optional dependencies if they weren't
    // explicitly listed anywhere.
    let explicitly_listed: HashSet<_> = map
        .values()
        .flatten()
        .filter_map(|fv| match fv {
            Dep { dep_name } => Some(*dep_name),
            _ => None,
        })
        .collect();
    for dep in dependencies {
        if !dep.is_optional() {
            continue;
        }
        let dep_name_in_toml = dep.name_in_toml();
        if features.contains_key(&dep_name_in_toml) || explicitly_listed.contains(&dep_name_in_toml)
        {
            continue;
        }
        let fv = Dep {
            dep_name: dep_name_in_toml,
        };
        map.insert(dep_name_in_toml, vec![fv]);
    }

    // Validate features are listed properly.
    for (feature, fvs) in &map {
        if feature.starts_with("dep:") {
            bail!(
                "feature named `{}` is not allowed to start with `dep:`",
                feature
            );
        }
        if feature.contains('/') {
            bail!(
                "feature named `{}` is not allowed to contain slashes",
                feature
            );
        }
        validate_feature_name(config, pkg_id, feature)?;
        for fv in fvs {
            // Find data for the referenced dependency...
            let dep_data = {
                match fv {
                    Feature(dep_name) | Dep { dep_name, .. } | DepFeature { dep_name, .. } => {
                        dep_map.get(dep_name)
                    }
                }
            };
            let is_optional_dep = dep_data
                .iter()
                .flat_map(|d| d.iter())
                .any(|d| d.is_optional());
            let is_any_dep = dep_data.is_some();
            match fv {
                Feature(f) => {
                    if !features.contains_key(f) {
                        if !is_any_dep {
                            bail!(
                                "feature `{}` includes `{}` which is neither a dependency \
                                 nor another feature",
                                feature,
                                fv
                            );
                        }
                        if is_optional_dep {
                            if !map.contains_key(f) {
                                bail!(
                                    "feature `{}` includes `{}`, but `{}` is an \
                                     optional dependency without an implicit feature\n\
                                     Use `dep:{}` to enable the dependency.",
                                    feature,
                                    fv,
                                    f,
                                    f
                                );
                            }
                        } else {
                            bail!("feature `{}` includes `{}`, but `{}` is not an optional dependency\n\
                                A non-optional dependency of the same name is defined; \
                                consider adding `optional = true` to its definition.",
                                feature, fv, f);
                        }
                    }
                }
                Dep { dep_name } => {
                    if !is_any_dep {
                        bail!(
                            "feature `{}` includes `{}`, but `{}` is not listed as a dependency",
                            feature,
                            fv,
                            dep_name
                        );
                    }
                    if !is_optional_dep {
                        bail!(
                            "feature `{}` includes `{}`, but `{}` is not an optional dependency\n\
                             A non-optional dependency of the same name is defined; \
                             consider adding `optional = true` to its definition.",
                            feature,
                            fv,
                            dep_name
                        );
                    }
                }
                DepFeature {
                    dep_name,
                    dep_feature,
                    weak,
                    ..
                } => {
                    // Early check for some unlikely syntax.
                    if dep_feature.contains('/') {
                        bail!(
                            "multiple slashes in feature `{}` (included by feature `{}`) are not allowed",
                            fv,
                            feature
                        );
                    }
                    // Validation of the feature name will be performed in the resolver.
                    if !is_any_dep {
                        bail!(
                            "feature `{}` includes `{}`, but `{}` is not a dependency",
                            feature,
                            fv,
                            dep_name
                        );
                    }
                    if *weak && !is_optional_dep {
                        bail!("feature `{}` includes `{}` with a `?`, but `{}` is not an optional dependency\n\
                            A non-optional dependency of the same name is defined; \
                            consider removing the `?` or changing the dependency to be optional",
                            feature, fv, dep_name);
                    }
                }
            }
        }
    }

    // Make sure every optional dep is mentioned at least once.
    let used: HashSet<_> = map
        .values()
        .flatten()
        .filter_map(|fv| match fv {
            Dep { dep_name } | DepFeature { dep_name, .. } => Some(dep_name),
            _ => None,
        })
        .collect();
    if let Some(dep) = dependencies
        .iter()
        .find(|dep| dep.is_optional() && !used.contains(&dep.name_in_toml()))
    {
        bail!(
            "optional dependency `{}` is not included in any feature\n\
            Make sure that `dep:{}` is included in one of features in the [features] table.",
            dep.name_in_toml(),
            dep.name_in_toml(),
        );
    }

    Ok(map)
}

/// FeatureValue represents the types of dependencies a feature can have.
#[derive(Clone, Debug, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub enum FeatureValue {
    /// A feature enabling another feature.
    Feature(InternedString),
    /// A feature enabling a dependency with `dep:dep_name` syntax.
    Dep { dep_name: InternedString },
    /// A feature enabling a feature on a dependency with `crate_name/feat_name` syntax.
    DepFeature {
        dep_name: InternedString,
        dep_feature: InternedString,
        /// If `true`, indicates the `?` syntax is used, which means this will
        /// not automatically enable the dependency unless the dependency is
        /// activated through some other means.
        weak: bool,
    },
}

impl FeatureValue {
    pub fn new(feature: InternedString) -> FeatureValue {
        match feature.find('/') {
            Some(pos) => {
                let (dep, dep_feat) = feature.split_at(pos);
                let dep_feat = &dep_feat[1..];
                let (dep, weak) = if let Some(dep) = dep.strip_suffix('?') {
                    (dep, true)
                } else {
                    (dep, false)
                };
                FeatureValue::DepFeature {
                    dep_name: InternedString::new(dep),
                    dep_feature: InternedString::new(dep_feat),
                    weak,
                }
            }
            None => {
                if let Some(dep_name) = feature.strip_prefix("dep:") {
                    FeatureValue::Dep {
                        dep_name: InternedString::new(dep_name),
                    }
                } else {
                    FeatureValue::Feature(feature)
                }
            }
        }
    }

    /// Returns `true` if this feature explicitly used `dep:` syntax.
    pub fn has_dep_prefix(&self) -> bool {
        matches!(self, FeatureValue::Dep { .. })
    }
}

impl fmt::Display for FeatureValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use self::FeatureValue::*;
        match self {
            Feature(feat) => write!(f, "{}", feat),
            Dep { dep_name } => write!(f, "dep:{}", dep_name),
            DepFeature {
                dep_name,
                dep_feature,
                weak,
            } => {
                let weak = if *weak { "?" } else { "" };
                write!(f, "{}{}/{}", dep_name, weak, dep_feature)
            }
        }
    }
}

pub type FeatureMap = BTreeMap<InternedString, Vec<FeatureValue>>;

fn validate_feature_name(config: &Config, pkg_id: PackageId, name: &str) -> CargoResult<()> {
    let mut chars = name.chars();
    const FUTURE: &str = "This was previously accepted but is being phased out; \
        it will become a hard error in a future release.\n\
        For more information, see issue #8813 <https://github.com/rust-lang/cargo/issues/8813>, \
        and please leave a comment if this will be a problem for your project.";
    if let Some(ch) = chars.next() {
        if !(unicode_xid::UnicodeXID::is_xid_start(ch) || ch == '_' || ch.is_digit(10)) {
            config.shell().warn(&format!(
                "invalid character `{}` in feature `{}` in package {}, \
                the first character must be a Unicode XID start character or digit \
                (most letters or `_` or `0` to `9`)\n\
                {}",
                ch, name, pkg_id, FUTURE
            ))?;
        }
    }
    for ch in chars {
        if !(unicode_xid::UnicodeXID::is_xid_continue(ch) || ch == '-' || ch == '+' || ch == '.') {
            config.shell().warn(&format!(
                "invalid character `{}` in feature `{}` in package {}, \
                characters must be Unicode XID characters, `+`, or `.` \
                (numbers, `+`, `-`, `_`, `.`, or most letters)\n\
                {}",
                ch, name, pkg_id, FUTURE
            ))?;
        }
    }
    Ok(())
}
