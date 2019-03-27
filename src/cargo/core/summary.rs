use std::borrow::Borrow;
use std::collections::{BTreeMap, HashMap};
use std::fmt::Display;
use std::mem;
use std::rc::Rc;

use serde::{Serialize, Serializer};

use crate::core::interning::InternedString;
use crate::core::{Dependency, PackageId, SourceId};
use semver::Version;

use crate::util::CargoResult;

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
    features: FeatureMap,
    checksum: Option<String>,
    links: Option<InternedString>,
    namespaced_features: bool,
}

impl Summary {
    pub fn new<K>(
        pkg_id: PackageId,
        dependencies: Vec<Dependency>,
        features: &BTreeMap<K, Vec<impl AsRef<str>>>,
        links: Option<impl AsRef<str>>,
        namespaced_features: bool,
    ) -> CargoResult<Summary>
    where
        K: Borrow<str> + Ord + Display,
    {
        for dep in dependencies.iter() {
            let feature = dep.name_in_toml();
            if !namespaced_features && features.get(&*feature).is_some() {
                failure::bail!(
                    "Features and dependencies cannot have the \
                     same name: `{}`",
                    feature
                )
            }
            if dep.is_optional() && !dep.is_transitive() {
                failure::bail!(
                    "Dev-dependencies are not allowed to be optional: `{}`",
                    feature
                )
            }
        }
        let feature_map = build_feature_map(features, &dependencies, namespaced_features)?;
        Ok(Summary {
            inner: Rc::new(Inner {
                package_id: pkg_id,
                dependencies,
                features: feature_map,
                checksum: None,
                links: links.map(|l| InternedString::new(l.as_ref())),
                namespaced_features,
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
        self.inner.checksum.as_ref().map(|s| &s[..])
    }
    pub fn links(&self) -> Option<InternedString> {
        self.inner.links
    }
    pub fn namespaced_features(&self) -> bool {
        self.inner.namespaced_features
    }

    pub fn override_id(mut self, id: PackageId) -> Summary {
        Rc::make_mut(&mut self.inner).package_id = id;
        self
    }

    pub fn set_checksum(mut self, cksum: String) -> Summary {
        Rc::make_mut(&mut self.inner).checksum = Some(cksum);
        self
    }

    pub fn map_dependencies<F>(mut self, f: F) -> Summary
    where
        F: FnMut(Dependency) -> Dependency,
    {
        {
            let slot = &mut Rc::make_mut(&mut self.inner).dependencies;
            let deps = mem::replace(slot, Vec::new());
            *slot = deps.into_iter().map(f).collect();
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

// Checks features for errors, bailing out a CargoResult:Err if invalid,
// and creates FeatureValues for each feature.
fn build_feature_map<K>(
    features: &BTreeMap<K, Vec<impl AsRef<str>>>,
    dependencies: &[Dependency],
    namespaced: bool,
) -> CargoResult<FeatureMap>
where
    K: Borrow<str> + Ord + Display,
{
    use self::FeatureValue::*;
    let mut dep_map = HashMap::new();
    for dep in dependencies.iter() {
        dep_map
            .entry(dep.name_in_toml())
            .or_insert_with(Vec::new)
            .push(dep);
    }

    let mut map = BTreeMap::new();
    for (feature, list) in features.iter() {
        // If namespaced features is active and the key is the same as that of an
        // optional dependency, that dependency must be included in the values.
        // Thus, if a `feature` is found that has the same name as a dependency, we
        // (a) bail out if the dependency is non-optional, and (b) we track if the
        // feature requirements include the dependency `crate:feature` in the list.
        // This is done with the `dependency_found` variable, which can only be
        // false if features are namespaced and the current feature key is the same
        // as the name of an optional dependency. If so, it gets set to true during
        // iteration over the list if the dependency is found in the list.
        let mut dependency_found = if namespaced {
            match dep_map.get(feature.borrow()) {
                Some(dep_data) => {
                    if !dep_data.iter().any(|d| d.is_optional()) {
                        failure::bail!(
                            "Feature `{}` includes the dependency of the same name, but this is \
                             left implicit in the features included by this feature.\n\
                             Additionally, the dependency must be marked as optional to be \
                             included in the feature definition.\n\
                             Consider adding `crate:{}` to this feature's requirements \
                             and marking the dependency as `optional = true`",
                            feature,
                            feature
                        )
                    } else {
                        false
                    }
                }
                None => true,
            }
        } else {
            true
        };

        let mut values = vec![];
        for dep in list {
            let val = FeatureValue::build(
                InternedString::new(dep.as_ref()),
                |fs| features.contains_key(fs.as_str()),
                namespaced,
            );

            // Find data for the referenced dependency...
            let dep_data = {
                match val {
                    Feature(ref dep_name) | Crate(ref dep_name) | CrateFeature(ref dep_name, _) => {
                        dep_map.get(dep_name.as_str())
                    }
                }
            };
            let is_optional_dep = dep_data
                .iter()
                .flat_map(|d| d.iter())
                .any(|d| d.is_optional());
            if let FeatureValue::Crate(ref dep_name) = val {
                // If we have a dependency value, check if this is the dependency named
                // the same as the feature that we were looking for.
                if !dependency_found && feature.borrow() == dep_name.as_str() {
                    dependency_found = true;
                }
            }

            match (&val, dep_data.is_some(), is_optional_dep) {
                // The value is a feature. If features are namespaced, this just means
                // it's not prefixed with `crate:`, so we have to check whether the
                // feature actually exist. If the feature is not defined *and* an optional
                // dependency of the same name exists, the feature is defined implicitly
                // here by adding it to the feature map, pointing to the dependency.
                // If features are not namespaced, it's been validated as a feature already
                // while instantiating the `FeatureValue` in `FeatureValue::build()`, so
                // we don't have to do so here.
                (&Feature(feat), _, true) => {
                    if namespaced && !features.contains_key(&*feat) {
                        map.insert(feat, vec![FeatureValue::Crate(feat)]);
                    }
                }
                // If features are namespaced and the value is not defined as a feature
                // and there is no optional dependency of the same name, error out.
                // If features are not namespaced, there must be an existing feature
                // here (checked by `FeatureValue::build()`), so it will always be defined.
                (&Feature(feat), dep_exists, false) => {
                    if namespaced && !features.contains_key(&*feat) {
                        if dep_exists {
                            failure::bail!(
                                "Feature `{}` includes `{}` which is not defined as a feature.\n\
                                 A non-optional dependency of the same name is defined; consider \
                                 adding `optional = true` to its definition",
                                feature,
                                feat
                            )
                        } else {
                            failure::bail!(
                                "Feature `{}` includes `{}` which is not defined as a feature",
                                feature,
                                feat
                            )
                        }
                    }
                }
                // The value is a dependency. If features are namespaced, it is explicitly
                // tagged as such (`crate:value`). If features are not namespaced, any value
                // not recognized as a feature is pegged as a `Crate`. Here we handle the case
                // where the dependency exists but is non-optional. It branches on namespaced
                // just to provide the correct string for the crate dependency in the error.
                (&Crate(ref dep), true, false) => {
                    if namespaced {
                        failure::bail!(
                            "Feature `{}` includes `crate:{}` which is not an \
                             optional dependency.\nConsider adding \
                             `optional = true` to the dependency",
                            feature,
                            dep
                        )
                    } else {
                        failure::bail!(
                            "Feature `{}` depends on `{}` which is not an \
                             optional dependency.\nConsider adding \
                             `optional = true` to the dependency",
                            feature,
                            dep
                        )
                    }
                }
                // If namespaced, the value was tagged as a dependency; if not namespaced,
                // this could be anything not defined as a feature. This handles the case
                // where no such dependency is actually defined; again, the branch on
                // namespaced here is just to provide the correct string in the error.
                (&Crate(ref dep), false, _) => {
                    if namespaced {
                        failure::bail!(
                            "Feature `{}` includes `crate:{}` which is not a known \
                             dependency",
                            feature,
                            dep
                        )
                    } else {
                        failure::bail!(
                            "Feature `{}` includes `{}` which is neither a dependency nor \
                             another feature",
                            feature,
                            dep
                        )
                    }
                }
                (&Crate(_), true, true) => {}
                // If the value is a feature for one of the dependencies, bail out if no such
                // dependency is actually defined in the manifest.
                (&CrateFeature(ref dep, _), false, _) => failure::bail!(
                    "Feature `{}` requires a feature of `{}` which is not a \
                     dependency",
                    feature,
                    dep
                ),
                (&CrateFeature(_, _), true, _) => {}
            }
            values.push(val);
        }

        if !dependency_found {
            // If we have not found the dependency of the same-named feature, we should
            // bail here.
            failure::bail!(
                "Feature `{}` includes the optional dependency of the \
                 same name, but this is left implicit in the features \
                 included by this feature.\nConsider adding \
                 `crate:{}` to this feature's requirements.",
                feature,
                feature
            )
        }

        map.insert(InternedString::new(feature.borrow()), values);
    }
    Ok(map)
}

/// FeatureValue represents the types of dependencies a feature can have:
///
/// * Another feature
/// * An optional dependency
/// * A feature in a dependency
///
/// The selection between these 3 things happens as part of the construction of the FeatureValue.
#[derive(Clone, Debug)]
pub enum FeatureValue {
    Feature(InternedString),
    Crate(InternedString),
    CrateFeature(InternedString, InternedString),
}

impl FeatureValue {
    fn build<T>(feature: InternedString, is_feature: T, namespaced: bool) -> FeatureValue
    where
        T: Fn(InternedString) -> bool,
    {
        match (feature.find('/'), namespaced) {
            (Some(pos), _) => {
                let (dep, dep_feat) = feature.split_at(pos);
                let dep_feat = &dep_feat[1..];
                FeatureValue::CrateFeature(InternedString::new(dep), InternedString::new(dep_feat))
            }
            (None, true) if feature.starts_with("crate:") => {
                FeatureValue::Crate(InternedString::new(&feature[6..]))
            }
            (None, true) => FeatureValue::Feature(feature),
            (None, false) if is_feature(feature) => FeatureValue::Feature(feature),
            (None, false) => FeatureValue::Crate(feature),
        }
    }

    pub fn new(feature: InternedString, s: &Summary) -> FeatureValue {
        Self::build(
            feature,
            |fs| s.features().contains_key(&fs),
            s.namespaced_features(),
        )
    }

    pub fn to_string(&self, s: &Summary) -> String {
        use self::FeatureValue::*;
        match *self {
            Feature(ref f) => f.to_string(),
            Crate(ref c) => {
                if s.namespaced_features() {
                    format!("crate:{}", &c)
                } else {
                    c.to_string()
                }
            }
            CrateFeature(ref c, ref f) => [c.as_ref(), f.as_ref()].join("/"),
        }
    }
}

impl Serialize for FeatureValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use self::FeatureValue::*;
        match *self {
            Feature(ref f) => serializer.serialize_str(f),
            Crate(ref c) => serializer.serialize_str(c),
            CrateFeature(ref c, ref f) => {
                serializer.serialize_str(&[c.as_ref(), f.as_ref()].join("/"))
            }
        }
    }
}

pub type FeatureMap = BTreeMap<InternedString, Vec<FeatureValue>>;
