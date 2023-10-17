use super::unit_graph::UnitGraph;
use crate::core::resolver::errors::describe_path;
use crate::core::{PackageId, Resolve};
use crate::util::errors::CargoResult;
use std::collections::{HashMap, HashSet};

/// Validates [`package.links`] field in the manifest file does not conflict
/// between packages.
///
/// NOTE: This is the *old* links validator. Links are usually validated in the
/// resolver. However, the `links` field was added to the index in early 2018
/// (see [rust-lang/cargo#4978]). However, `links` has been around since 2014,
/// so there are still many crates in the index that don't have `links`
/// properly set in the index (over 600 at the time of this writing in 2019).
/// This can probably be removed at some point in the future, though it might
/// be worth considering fixing the index.
///
/// [rust-lang/cargo#4978]: https://github.com/rust-lang/cargo/pull/4978
/// [`package.links`]: https://doc.rust-lang.org/nightly/cargo/reference/build-scripts.html#the-links-manifest-key
pub fn validate_links(resolve: &Resolve, unit_graph: &UnitGraph) -> CargoResult<()> {
    let mut validated: HashSet<PackageId> = HashSet::new();
    let mut links: HashMap<String, PackageId> = HashMap::new();
    let mut units: Vec<_> = unit_graph.keys().collect();
    // Sort primarily to make testing easier.
    units.sort_unstable();
    for unit in units {
        if !validated.insert(unit.pkg.package_id()) {
            continue;
        }
        let Some(lib) = unit.pkg.manifest().links() else {
            continue;
        };
        if let Some(&prev) = links.get(lib) {
            let prev_path = resolve
                .path_to_top(&prev)
                .into_iter()
                .map(|(p, d)| (p, d.and_then(|d| d.iter().next())));
            let pkg = unit.pkg.package_id();
            let path = resolve
                .path_to_top(&pkg)
                .into_iter()
                .map(|(p, d)| (p, d.and_then(|d| d.iter().next())));
            anyhow::bail!(
                "multiple packages link to native library `{}`, \
                 but a native library can be linked only once\n\
                 \n\
                 {}\nlinks to native library `{}`\n\
                 \n\
                 {}\nalso links to native library `{}`",
                lib,
                describe_path(prev_path),
                lib,
                describe_path(path),
                lib
            )
        }
        links.insert(lib.to_string(), unit.pkg.package_id());
    }
    Ok(())
}
