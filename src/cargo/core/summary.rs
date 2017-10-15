use std::collections::BTreeMap;
use std::mem;
use std::rc::Rc;

use semver::Version;
use core::{Dependency, PackageId, SourceId};
use core::interning::InternedString;

use util::CargoResult;

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
}

impl Summary {
    pub fn new(
        pkg_id: PackageId,
        dependencies: Vec<Dependency>,
        features: BTreeMap<String, Vec<String>>,
        links: Option<String>,
    ) -> CargoResult<Summary> {
        for dep in dependencies.iter() {
            if features.get(&*dep.name()).is_some() {
                bail!(
                    "Features and dependencies cannot have the \
                     same name: `{}`",
                    dep.name()
                )
            }
            if dep.is_optional() && !dep.is_transitive() {
                bail!(
                    "Dev-dependencies are not allowed to be optional: `{}`",
                    dep.name()
                )
            }
        }
        let mut features_new = BTreeMap::new();
        for (feature, list) in features.iter() {
            let mut values = vec![];
            for dep in list {
                use self::FeatureValue::*;
                let val = FeatureValue::build(dep, |fs| (&features).get(fs).is_some());
                if let &Feature(_) = &val {
                    // Return early to avoid doing unnecessary work
                    values.push(val);
                    continue;
                }
                // Find data for the referenced dependency...
                let dep_data = {
                    let dep_name = match &val {
                        &Feature(_) => "",
                        &Crate(ref dep_name) | &CrateFeature(ref dep_name, _) => dep_name,
                    };
                    dependencies.iter().find(|d| *d.name() == *dep_name)
                };
                match (&val, dep_data) {
                    (&Crate(ref dep), Some(d)) => {
                        if !d.is_optional() {
                            bail!(
                                "Feature `{}` depends on `{}` which is not an \
                                 optional dependency.\nConsider adding \
                                 `optional = true` to the dependency",
                                feature,
                                dep
                            )
                        }
                    }
                    (&CrateFeature(ref dep_name, _), None) => bail!(
                        "Feature `{}` requires a feature of `{}` which is not a \
                         dependency",
                        feature,
                        dep_name
                    ),
                    (&Crate(ref dep), None) => bail!(
                        "Feature `{}` includes `{}` which is neither \
                         a dependency nor another feature",
                        feature,
                        dep
                    ),
                    (&CrateFeature(_, _), Some(_)) | (&Feature(_), _) => {}
                }
                values.push(val);
            }
            features_new.insert(feature.clone(), values);
        }
        Ok(Summary {
            inner: Rc::new(Inner {
                package_id: pkg_id,
                dependencies,
                features: features_new,
                checksum: None,
                links: links.map(|l| InternedString::new(&l)),
            }),
        })
    }

    pub fn package_id(&self) -> &PackageId {
        &self.inner.package_id
    }
    pub fn name(&self) -> InternedString {
        self.package_id().name()
    }
    pub fn version(&self) -> &Version {
        self.package_id().version()
    }
    pub fn source_id(&self) -> &SourceId {
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

    pub fn map_source(self, to_replace: &SourceId, replace_with: &SourceId) -> Summary {
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

/// FeatureValue represents the types of dependencies a feature can have:
///
/// * Another feature
/// * An optional dependency
/// * A feature in a depedency
///
/// The selection between these 3 things happens as part of the construction of the FeatureValue.
#[derive(Clone, Debug, Serialize)]
pub enum FeatureValue {
    Feature(InternedString),
    Crate(InternedString),
    CrateFeature(InternedString, InternedString),
}

impl FeatureValue {
    fn build<T>(feature: &str, is_feature: T) -> FeatureValue
    where
        T: Fn(&str) -> bool,
    {
        match feature.find('/') {
            Some(pos) => {
                let (dep, dep_feat) = feature.split_at(pos);
                let dep_feat = &dep_feat[1..];
                FeatureValue::CrateFeature(InternedString::new(dep), InternedString::new(dep_feat))
            }
            None if is_feature(&feature) => FeatureValue::Feature(InternedString::new(feature)),
            None => FeatureValue::Crate(InternedString::new(feature)),
        }
    }

    pub fn new(feature: &str, s: &Summary) -> FeatureValue {
        Self::build(feature, |fs| s.features().get(fs).is_some())
    }

    pub fn to_string(&self) -> String {
        use self::FeatureValue::*;
        match *self {
            Feature(ref f) => f.to_string(),
            Crate(ref c) => c.to_string(),
            CrateFeature(ref c, ref f) => [c.as_ref(), f.as_ref()].join("/"),
        }
    }
}

pub type FeatureMap = BTreeMap<String, Vec<FeatureValue>>;
