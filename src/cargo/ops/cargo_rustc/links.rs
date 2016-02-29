use std::collections::{HashMap, HashSet};

use core::PackageId;
use util::CargoResult;
use super::Unit;

pub struct Links<'a> {
    validated: HashSet<&'a PackageId>,
    links: HashMap<String, &'a PackageId>,
}

impl<'a> Links<'a> {
    pub fn new() -> Links<'a> {
        Links {
            validated: HashSet::new(),
            links: HashMap::new(),
        }
    }

    pub fn validate(&mut self, unit: &Unit<'a>) -> CargoResult<()> {
        if !self.validated.insert(unit.pkg.package_id()) {
            return Ok(())
        }
        let lib = match unit.pkg.manifest().links() {
            Some(lib) => lib,
            None => return Ok(()),
        };
        if let Some(prev) = self.links.get(lib) {
            let pkg = unit.pkg.package_id();
            if prev.name() == pkg.name() && prev.source_id() == pkg.source_id() {
                bail!("native library `{}` is being linked to by more \
                       than one version of the same package, but it can \
                       only be linked once; try updating or pinning your \
                       dependencies to ensure that this package only shows \
                       up once\n\n  {}\n  {}", lib, prev, pkg)
            } else {
                bail!("native library `{}` is being linked to by more than \
                       one package, and can only be linked to by one \
                       package\n\n  {}\n  {}", lib, prev, pkg)
            }
        }
        if !unit.pkg.manifest().targets().iter().any(|t| t.is_custom_build()) {
            bail!("package `{}` specifies that it links to `{}` but does not \
                   have a custom build script", unit.pkg.package_id(), lib)
        }
        self.links.insert(lib.to_string(), unit.pkg.package_id());
        Ok(())
    }
}
