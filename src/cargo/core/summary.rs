use std::collections::HashMap;
use std::mem;

use semver::Version;
use core::{Dependency, PackageId, SourceId};

use util::{CargoResult, human};

/// Subset of a `Manifest`. Contains only the most important informations about
/// a package.
///
/// Summaries are cloned, and should not be mutated after creation
#[deriving(Show,Clone)]
pub struct Summary {
    package_id: PackageId,
    dependencies: Vec<Dependency>,
    features: HashMap<String, Vec<String>>,
}

impl Summary {
    pub fn new(pkg_id: PackageId,
               dependencies: Vec<Dependency>,
               features: HashMap<String, Vec<String>>) -> CargoResult<Summary> {
        for dep in dependencies.iter() {
            if features.find_equiv(dep.get_name()).is_some() {
                return Err(human(format!("Features and dependencies cannot have \
                                          the same name: `{}`", dep.get_name())))
            }
            if dep.is_optional() && !dep.is_transitive() {
                return Err(human(format!("Dev-dependencies are not allowed \
                                          to be optional: `{}`",
                                          dep.get_name())))
            }
        }
        for (feature, list) in features.iter() {
            for dep in list.iter() {
                let mut parts = dep.as_slice().splitn(1, '/');
                let dep = parts.next().unwrap();
                let is_reexport = parts.next().is_some();
                if !is_reexport && features.find_equiv(dep).is_some() { continue }
                match dependencies.iter().find(|d| d.get_name() == dep) {
                    Some(d) => {
                        if d.is_optional() || is_reexport { continue }
                        return Err(human(format!("Feature `{}` depends on `{}` \
                                                  which is not an optional \
                                                  dependency.\nConsider adding \
                                                  `optional = true` to the \
                                                  dependency", feature, dep)))
                    }
                    None if is_reexport => {
                        return Err(human(format!("Feature `{}` requires `{}` \
                                                  which is not an optional \
                                                  dependency", feature, dep)))
                    }
                    None => {
                        return Err(human(format!("Feature `{}` includes `{}` \
                                                  which is neither a dependency \
                                                  nor another feature",
                                                  feature, dep)))
                    }
                }
            }
        }
        Ok(Summary {
            package_id: pkg_id,
            dependencies: dependencies,
            features: features,
        })
    }

    pub fn get_package_id(&self) -> &PackageId {
        &self.package_id
    }

    pub fn get_name(&self) -> &str {
        self.get_package_id().get_name()
    }

    pub fn get_version(&self) -> &Version {
        self.get_package_id().get_version()
    }

    pub fn get_source_id(&self) -> &SourceId {
        self.package_id.get_source_id()
    }

    pub fn get_dependencies(&self) -> &[Dependency] {
        self.dependencies.as_slice()
    }

    pub fn get_features(&self) -> &HashMap<String, Vec<String>> {
        &self.features
    }

    pub fn override_id(mut self, id: PackageId) -> Summary {
        self.package_id = id;
        self
    }

    pub fn map_dependencies(mut self, f: |Dependency| -> Dependency) -> Summary {
        let deps = mem::replace(&mut self.dependencies, Vec::new());
        self.dependencies = deps.into_iter().map(f).collect();
        self
    }
}

impl PartialEq for Summary {
    fn eq(&self, other: &Summary) -> bool {
        self.package_id == other.package_id
    }
}

pub trait SummaryVec {
    fn names(&self) -> Vec<String>;
}

impl SummaryVec for Vec<Summary> {
    // TODO: Move to Registry
    fn names(&self) -> Vec<String> {
        self.iter().map(|summary| summary.get_name().to_string()).collect()
    }

}
