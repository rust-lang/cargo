use std::collections::HashMap;

use core::{PackageId, PackageSet};
use util::CargoResult;

// Validate that there are no duplicated native libraries among packages and
// that all packages with `links` also have a build script.
pub fn validate(deps: &PackageSet) -> CargoResult<()> {
    let mut map: HashMap<_, &PackageId> = HashMap::new();

    for dep in deps.packages() {
        let lib = match dep.manifest().links() {
            Some(lib) => lib,
            None => continue,
        };
        if let Some(prev) = map.get(&lib) {
            let dep = dep.package_id();
            if prev.name() == dep.name() && prev.source_id() == dep.source_id() {
                bail!("native library `{}` is being linked to by more \
                       than one version of the same package, but it can \
                       only be linked once; try updating or pinning your \
                       dependencies to ensure that this package only shows \
                       up once\n\n  {}\n  {}", lib, prev, dep)
            } else {
                bail!("native library `{}` is being linked to by more than \
                       one package, and can only be linked to by one \
                       package\n\n  {}\n  {}", lib, prev, dep)
            }
        }
        if !dep.manifest().targets().iter().any(|t| t.is_custom_build()) {
            bail!("package `{}` specifies that it links to `{}` but does not \
                   have a custom build script", dep.package_id(), lib)
        }
        map.insert(lib, dep.package_id());
    }

    Ok(())
}
