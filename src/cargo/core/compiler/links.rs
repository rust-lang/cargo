use super::unit_dependencies::UnitGraph;
use crate::core::{PackageId, Resolve};
use crate::util::errors::CargoResult;
use std::collections::{HashMap, HashSet};
use std::fmt::Write;

/// Validate `links` field does not conflict between packages.
pub fn validate_links(resolve: &Resolve, unit_graph: &UnitGraph<'_>) -> CargoResult<()> {
    // NOTE: This is the *old* links validator. Links are usually validated in
    // the resolver. However, the `links` field was added to the index in
    // early 2018 (see https://github.com/rust-lang/cargo/pull/4978). However,
    // `links` has been around since 2014, so there are still many crates in
    // the index that don't have `links` properly set in the index (over 600
    // at the time of this writing in 2019). This can probably be removed at
    // some point in the future, though it might be worth considering fixing
    // the index.
    let mut validated: HashSet<PackageId> = HashSet::new();
    let mut links: HashMap<String, PackageId> = HashMap::new();
    let mut units: Vec<_> = unit_graph.keys().collect();
    // Sort primarily to make testing easier.
    units.sort_unstable();
    for unit in units {
        if !validated.insert(unit.pkg.package_id()) {
            continue;
        }
        let lib = match unit.pkg.manifest().links() {
            Some(lib) => lib,
            None => continue,
        };
        if let Some(&prev) = links.get(lib) {
            let pkg = unit.pkg.package_id();

            let describe_path = |pkgid: PackageId| -> String {
                let dep_path = resolve.path_to_top(&pkgid);
                let mut dep_path_desc = format!("package `{}`", dep_path[0]);
                for dep in dep_path.iter().skip(1) {
                    write!(dep_path_desc, "\n    ... which is depended on by `{}`", dep).unwrap();
                }
                dep_path_desc
            };

            anyhow::bail!(
                "multiple packages link to native library `{}`, \
                 but a native library can be linked only once\n\
                 \n\
                 {}\nlinks to native library `{}`\n\
                 \n\
                 {}\nalso links to native library `{}`",
                lib,
                describe_path(prev),
                lib,
                describe_path(pkg),
                lib
            )
        }
        links.insert(lib.to_string(), unit.pkg.package_id());
    }
    Ok(())
}
