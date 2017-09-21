use std::collections::{HashMap, HashSet};
use std::fmt::Write;

use core::{Resolve, PackageId};
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

    pub fn validate(&mut self, resolve: &Resolve, unit: &Unit<'a>) -> CargoResult<()> {
        if !self.validated.insert(unit.pkg.package_id()) {
            return Ok(())
        }
        let lib = match unit.pkg.manifest().links() {
            Some(lib) => lib,
            None => return Ok(()),
        };
        if let Some(prev) = self.links.get(lib) {
            let pkg = unit.pkg.package_id();

            let describe_path = |pkgid: &PackageId| -> String {
                let dep_path = resolve.path_to_top(pkgid);
                if dep_path.is_empty() {
                    String::from("The root-package ")
                } else {
                    let mut dep_path_desc = format!("Package `{}`\n", pkgid);
                    for dep in dep_path {
                        write!(dep_path_desc,
                               "    ... which is depended on by `{}`\n",
                               dep).unwrap();
                    }
                    dep_path_desc
                }
            };

            bail!("Multiple packages link to native library `{}`. \
                   A native library can be linked only once.\n\
                   \n\
                   {}links to native library `{}`.\n\
                   \n\
                   {}also links to native library `{}`.",
                  lib,
                  describe_path(prev), lib,
                  describe_path(pkg), lib)
        }
        if !unit.pkg.manifest().targets().iter().any(|t| t.is_custom_build()) {
            bail!("package `{}` specifies that it links to `{}` but does not \
                   have a custom build script", unit.pkg.package_id(), lib)
        }
        self.links.insert(lib.to_string(), unit.pkg.package_id());
        Ok(())
    }
}
