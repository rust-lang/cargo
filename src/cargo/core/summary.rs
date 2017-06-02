use std::collections::HashMap;
use std::mem;
use std::rc::Rc;

use semver::Version;
use core::{Dependency, PackageId, SourceId};

use util::CargoResult;

/// Subset of a `Manifest`. Contains only the most important informations about
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
    features: HashMap<String, Vec<String>>,
    checksum: Option<String>,
}

impl Summary {
    pub fn new(pkg_id: PackageId,
               dependencies: Vec<Dependency>,
               features: HashMap<String, Vec<String>>) -> CargoResult<Summary> {
        for dep in dependencies.iter() {
            if features.get(dep.name()).is_some() {
                bail!("Features and dependencies cannot have the \
                       same name: `{}`", dep.name())
            }
            if dep.is_optional() && !dep.is_transitive() {
                bail!("Dev-dependencies are not allowed to be optional: `{}`",
                      dep.name())
            }
        }
        for (feature, list) in features.iter() {
            for dep in list.iter() {
                let mut parts = dep.splitn(2, '/');
                let dep = parts.next().unwrap();
                let is_reexport = parts.next().is_some();
                if !is_reexport && features.get(dep).is_some() { continue }
                match dependencies.iter().find(|d| d.name() == dep) {
                    Some(d) => {
                        if d.is_optional() || is_reexport { continue }
                        bail!("Feature `{}` depends on `{}` which is not an \
                               optional dependency.\nConsider adding \
                               `optional = true` to the dependency",
                               feature, dep)
                    }
                    None if is_reexport => {
                        bail!("Feature `{}` requires `{}` which is not an \
                               optional dependency", feature, dep)
                    }
                    None => {
                        bail!("Feature `{}` includes `{}` which is neither \
                               a dependency nor another feature", feature, dep)
                    }
                }
            }
        }
        Ok(Summary {
            inner: Rc::new(Inner {
                package_id: pkg_id,
                dependencies: dependencies,
                features: features,
                checksum: None,
            }),
        })
    }

    pub fn package_id(&self) -> &PackageId { &self.inner.package_id }
    pub fn name(&self) -> &str { self.package_id().name() }
    pub fn version(&self) -> &Version { self.package_id().version() }
    pub fn source_id(&self) -> &SourceId { self.package_id().source_id() }
    pub fn dependencies(&self) -> &[Dependency] { &self.inner.dependencies }
    pub fn features(&self) -> &HashMap<String, Vec<String>> { &self.inner.features }
    pub fn checksum(&self) -> Option<&str> {
        self.inner.checksum.as_ref().map(|s| &s[..])
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
                               where F: FnMut(Dependency) -> Dependency {
        {
            let slot = &mut Rc::make_mut(&mut self.inner).dependencies;
            let deps = mem::replace(slot, Vec::new());
            *slot = deps.into_iter().map(f).collect();
        }
        self
    }

    pub fn map_source(self, to_replace: &SourceId, replace_with: &SourceId)
                      -> Summary {
        let me = if self.package_id().source_id() == to_replace {
            let new_id = self.package_id().with_source_id(replace_with);
            self.override_id(new_id)
        } else {
            self
        };
        me.map_dependencies(|dep| {
            dep.map_source(to_replace, replace_with)
        })
    }
}

impl PartialEq for Summary {
    fn eq(&self, other: &Summary) -> bool {
        self.inner.package_id == other.inner.package_id
    }
}
