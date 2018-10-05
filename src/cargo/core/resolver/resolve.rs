use std::collections::{HashMap, HashSet};
use std::fmt;
use std::iter::FromIterator;

use url::Url;

use core::{Dependency, PackageId, PackageIdSpec, Summary, Target};
use util::errors::CargoResult;
use util::Graph;

use super::encode::Metadata;

/// Represents a fully resolved package dependency graph. Each node in the graph
/// is a package and edges represent dependencies between packages.
///
/// Each instance of `Resolve` also understands the full set of features used
/// for each package.
#[derive(PartialEq)]
pub struct Resolve {
    /// A graph, whose vertices are packages and edges are dependency specifications
    /// from Cargo.toml. We need a `Vec` here because the same package
    /// might be present in both `[dependencies]` and `[build-dependencies]`.
    graph: Graph<PackageId, Vec<Dependency>>,
    replacements: HashMap<PackageId, PackageId>,
    reverse_replacements: HashMap<PackageId, PackageId>,
    empty_features: HashSet<String>,
    features: HashMap<PackageId, HashSet<String>>,
    checksums: HashMap<PackageId, Option<String>>,
    metadata: Metadata,
    unused_patches: Vec<PackageId>,
}

impl Resolve {
    pub fn new(
        graph: Graph<PackageId, Vec<Dependency>>,
        replacements: HashMap<PackageId, PackageId>,
        features: HashMap<PackageId, HashSet<String>>,
        checksums: HashMap<PackageId, Option<String>>,
        metadata: Metadata,
        unused_patches: Vec<PackageId>,
    ) -> Resolve {
        let reverse_replacements = replacements
            .iter()
            .map(|p| (p.1.clone(), p.0.clone()))
            .collect();
        Resolve {
            graph,
            replacements,
            features,
            checksums,
            metadata,
            unused_patches,
            empty_features: HashSet::new(),
            reverse_replacements,
        }
    }

    /// Resolves one of the paths from the given dependent package up to
    /// the root.
    pub fn path_to_top<'a>(&'a self, pkg: &'a PackageId) -> Vec<&'a PackageId> {
        self.graph.path_to_top(pkg)
    }

    pub fn register_used_patches(&mut self, patches: &HashMap<Url, Vec<Summary>>) {
        for summary in patches.values().flat_map(|v| v) {
            if self.iter().any(|id| id == summary.package_id()) {
                continue;
            }
            self.unused_patches.push(summary.package_id().clone());
        }
    }

    pub fn merge_from(&mut self, previous: &Resolve) -> CargoResult<()> {
        // Given a previous instance of resolve, it should be forbidden to ever
        // have a checksums which *differ*. If the same package id has differing
        // checksums, then something has gone wrong such as:
        //
        // * Something got seriously corrupted
        // * A "mirror" isn't actually a mirror as some changes were made
        // * A replacement source wasn't actually a replacment, some changes
        //   were made
        //
        // In all of these cases, we want to report an error to indicate that
        // something is awry. Normal execution (esp just using crates.io) should
        // never run into this.
        for (id, cksum) in previous.checksums.iter() {
            if let Some(mine) = self.checksums.get(id) {
                if mine == cksum {
                    continue;
                }

                // If the previous checksum wasn't calculated, the current
                // checksum is `Some`. This may indicate that a source was
                // erroneously replaced or was replaced with something that
                // desires stronger checksum guarantees than can be afforded
                // elsewhere.
                if cksum.is_none() {
                    bail!(
                        "\
checksum for `{}` was not previously calculated, but a checksum could now \
be calculated

this could be indicative of a few possible situations:

    * the source `{}` did not previously support checksums,
      but was replaced with one that does
    * newer Cargo implementations know how to checksum this source, but this
      older implementation does not
    * the lock file is corrupt
",
                        id,
                        id.source_id()
                    )

                // If our checksum hasn't been calculated, then it could mean
                // that future Cargo figured out how to checksum something or
                // more realistically we were overridden with a source that does
                // not have checksums.
                } else if mine.is_none() {
                    bail!(
                        "\
checksum for `{}` could not be calculated, but a checksum is listed in \
the existing lock file

this could be indicative of a few possible situations:

    * the source `{}` supports checksums,
      but was replaced with one that doesn't
    * the lock file is corrupt

unable to verify that `{0}` is the same as when the lockfile was generated
",
                        id,
                        id.source_id()
                    )

                // If the checksums aren't equal, and neither is None, then they
                // must both be Some, in which case the checksum now differs.
                // That's quite bad!
                } else {
                    bail!(
                        "\
checksum for `{}` changed between lock files

this could be indicative of a few possible errors:

    * the lock file is corrupt
    * a replacement source in use (e.g. a mirror) returned a different checksum
    * the source itself may be corrupt in one way or another

unable to verify that `{0}` is the same as when the lockfile was generated
",
                        id
                    );
                }
            }
        }

        // Be sure to just copy over any unknown metadata.
        self.metadata = previous.metadata.clone();
        Ok(())
    }

    pub fn iter(&self) -> impl Iterator<Item = &PackageId> {
        self.graph.iter()
    }

    pub fn deps(&self, pkg: &PackageId) -> impl Iterator<Item = (&PackageId, &[Dependency])> {
        self.graph
            .edges(pkg)
            .map(move |(id, deps)| (self.replacement(id).unwrap_or(id), deps.as_slice()))
    }

    pub fn deps_not_replaced(&self, pkg: &PackageId) -> impl Iterator<Item = &PackageId> {
        self.graph.edges(pkg).map(|(id, _)| id)
    }

    pub fn replacement(&self, pkg: &PackageId) -> Option<&PackageId> {
        self.replacements.get(pkg)
    }

    pub fn replacements(&self) -> &HashMap<PackageId, PackageId> {
        &self.replacements
    }

    pub fn features(&self, pkg: &PackageId) -> &HashSet<String> {
        self.features.get(pkg).unwrap_or(&self.empty_features)
    }

    pub fn features_sorted(&self, pkg: &PackageId) -> Vec<&str> {
        let mut v = Vec::from_iter(self.features(pkg).iter().map(|s| s.as_ref()));
        v.sort_unstable();
        v
    }

    pub fn query(&self, spec: &str) -> CargoResult<&PackageId> {
        PackageIdSpec::query_str(spec, self.iter())
    }

    pub fn unused_patches(&self) -> &[PackageId] {
        &self.unused_patches
    }

    pub fn checksums(&self) -> &HashMap<PackageId, Option<String>> {
        &self.checksums
    }

    pub fn metadata(&self) -> &Metadata {
        &self.metadata
    }

    pub fn extern_crate_name(
        &self,
        from: &PackageId,
        to: &PackageId,
        to_target: &Target,
    ) -> CargoResult<String> {
        let deps = if from == to {
            &[]
        } else {
            self.dependencies_listed(from, to)
        };

        let crate_name = to_target.crate_name();
        let mut names = deps.iter().map(|d| {
            d.explicit_name_in_toml()
                .map(|s| s.as_str().replace("-", "_"))
                .unwrap_or(crate_name.clone())
        });
        let name = names.next().unwrap_or(crate_name.clone());
        for n in names {
            if n == name {
                continue;
            }
            bail!(
                "multiple dependencies listed for the same crate must \
                 all have the same name, but the dependency on `{}` \
                 is listed as having different names",
                to
            );
        }
        Ok(name.to_string())
    }

    fn dependencies_listed(&self, from: &PackageId, to: &PackageId) -> &[Dependency] {
        // We've got a dependency on `from` to `to`, but this dependency edge
        // may be affected by [replace]. If the `to` package is listed as the
        // target of a replacement (aka the key of a reverse replacement map)
        // then we try to find our dependency edge through that. If that fails
        // then we go down below assuming it's not replaced.
        //
        // Note that we don't treat `from` as if it's been replaced because
        // that's where the dependency originates from, and we only replace
        // targets of dependencies not the originator.
        if let Some(replace) = self.reverse_replacements.get(to) {
            if let Some(deps) = self.graph.edge(from, replace) {
                return deps;
            }
        }
        match self.graph.edge(from, to) {
            Some(ret) => ret,
            None => panic!("no Dependency listed for `{}` => `{}`", from, to),
        }
    }
}

impl fmt::Debug for Resolve {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        writeln!(fmt, "graph: {:?}", self.graph)?;
        writeln!(fmt, "\nfeatures: {{")?;
        for (pkg, features) in &self.features {
            writeln!(fmt, "  {}: {:?}", pkg, features)?;
        }
        write!(fmt, "}}")
    }
}
