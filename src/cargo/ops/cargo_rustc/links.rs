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
                let mut dep_path_desc = format!("package `{}`", dep_path[0]);
                for dep in dep_path.iter().skip(1) {
                    write!(dep_path_desc,
                           "\n    ... which is depended on by `{}`",
                           dep).unwrap();
                }
                dep_path_desc
            };

            bail!("multiple packages link to native library `{}`, \
                   but a native library can be linked only once\n\
                   \n\
                   {}\nlinks to native library `{}`\n\
                   \n\
                   {}\nalso links to native library `{}`",
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
