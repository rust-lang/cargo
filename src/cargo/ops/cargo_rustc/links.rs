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
                    String::from("(This is the root-package)")
                } else {
                    let mut pkg_path_desc = String::from("(Dependency via ");
                    let mut dep_path_iter = dep_path.into_iter().peekable();
                    while let Some(dep) = dep_path_iter.next() {
                        write!(pkg_path_desc, "{}", dep).unwrap();
                        if dep_path_iter.peek().is_some() {
                            pkg_path_desc.push_str(" => ");
                        }
                    }
                    pkg_path_desc.push(')');
                    pkg_path_desc
                }
            };

            bail!("More than one package links to native library `{}`, \
                   which can only be linked once.\n\n\
                   Package {} links to native library `{}`.\n\
                   {}\n\
                   \n\
                   Package {} also links to native library `{}`.\n\
                   {}\n\
                   \n\
                   Try updating or pinning your dependencies to ensure that \
                   native library `{}` is only linked once.",
                  lib,
                  prev, lib,
                  describe_path(prev),
                  pkg, lib,
                  describe_path(pkg),
                  lib)
        }
        if !unit.pkg.manifest().targets().iter().any(|t| t.is_custom_build()) {
            bail!("package `{}` specifies that it links to `{}` but does not \
                   have a custom build script", unit.pkg.package_id(), lib)
        }
        self.links.insert(lib.to_string(), unit.pkg.package_id());
        Ok(())
    }
}
