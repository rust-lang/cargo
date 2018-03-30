use std::collections::{HashMap, HashSet};
use std::fmt;
use std::iter::FromIterator;

use url::Url;

use core::{PackageId, Summary};
use core::PackageIdSpec;
use util::Graph;
use util::errors::CargoResult;
use util::graph::{Edges, Nodes};

use super::encode::Metadata;

/// Represents a fully resolved package dependency graph. Each node in the graph
/// is a package and edges represent dependencies between packages.
///
/// Each instance of `Resolve` also understands the full set of features used
/// for each package.
#[derive(PartialEq)]
pub struct Resolve {
    pub graph: Graph<PackageId>,
    pub replacements: HashMap<PackageId, PackageId>,
    pub empty_features: HashSet<String>,
    pub features: HashMap<PackageId, HashSet<String>>,
    pub checksums: HashMap<PackageId, Option<String>>,
    pub metadata: Metadata,
    pub unused_patches: Vec<PackageId>,
}

impl Resolve {
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

    pub fn iter(&self) -> Nodes<PackageId> {
        self.graph.iter()
    }

    pub fn deps(&self, pkg: &PackageId) -> Deps {
        Deps {
            edges: self.graph.edges(pkg),
            resolve: self,
        }
    }

    pub fn deps_not_replaced(&self, pkg: &PackageId) -> DepsNotReplaced {
        DepsNotReplaced {
            edges: self.graph.edges(pkg),
        }
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
        v.sort();
        v
    }

    pub fn query(&self, spec: &str) -> CargoResult<&PackageId> {
        PackageIdSpec::query_str(spec, self.iter())
    }

    pub fn unused_patches(&self) -> &[PackageId] {
        &self.unused_patches
    }
}

impl fmt::Debug for Resolve {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "graph: {:?}\n", self.graph)?;
        write!(fmt, "\nfeatures: {{\n")?;
        for (pkg, features) in &self.features {
            write!(fmt, "  {}: {:?}\n", pkg, features)?;
        }
        write!(fmt, "}}")
    }
}

pub struct Deps<'a> {
    edges: Option<Edges<'a, PackageId>>,
    resolve: &'a Resolve,
}

impl<'a> Iterator for Deps<'a> {
    type Item = &'a PackageId;

    fn next(&mut self) -> Option<&'a PackageId> {
        self.edges
            .as_mut()
            .and_then(|e| e.next())
            .map(|id| self.resolve.replacement(id).unwrap_or(id))
    }
}

pub struct DepsNotReplaced<'a> {
    edges: Option<Edges<'a, PackageId>>,
}

impl<'a> Iterator for DepsNotReplaced<'a> {
    type Item = &'a PackageId;

    fn next(&mut self) -> Option<&'a PackageId> {
        self.edges.as_mut().and_then(|e| e.next())
    }
}
