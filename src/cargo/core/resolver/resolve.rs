use std::borrow::Borrow;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::iter::FromIterator;

use crate::core::dependency::Kind;
use crate::core::{Dependency, PackageId, PackageIdSpec, Summary, Target};
use crate::util::errors::CargoResult;
use crate::util::Graph;

use super::encode::Metadata;

/// Represents a fully-resolved package dependency graph. Each node in the graph
/// is a package and edges represent dependencies between packages.
///
/// Each instance of `Resolve` also understands the full set of features used
/// for each package.
#[derive(PartialEq)]
pub struct Resolve {
    /// A graph, whose vertices are packages and edges are dependency specifications
    /// from `Cargo.toml`. We need a `Vec` here because the same package
    /// might be present in both `[dependencies]` and `[build-dependencies]`.
    graph: Graph<PackageId, Vec<Dependency>>,
    /// Replacements from the `[replace]` table.
    replacements: HashMap<PackageId, PackageId>,
    /// Inverted version of `replacements`.
    reverse_replacements: HashMap<PackageId, PackageId>,
    /// An empty `HashSet` to avoid creating a new `HashSet` for every package
    /// that does not have any features, and to avoid using `Option` to
    /// simplify the API.
    empty_features: HashSet<String>,
    /// Features enabled for a given package.
    features: HashMap<PackageId, HashSet<String>>,
    /// Checksum for each package. A SHA256 hash of the `.crate` file used to
    /// validate the correct crate file is used. This is `None` for sources
    /// that do not use `.crate` files, like path or git dependencies.
    checksums: HashMap<PackageId, Option<String>>,
    /// "Unknown" metadata. This is a collection of extra, unrecognized data
    /// found in the `[metadata]` section of `Cargo.lock`, preserved for
    /// forwards compatibility.
    metadata: Metadata,
    /// `[patch]` entries that did not match anything, preserved in
    /// `Cargo.lock` as the `[[patch.unused]]` table array. Tracking unused
    /// patches helps prevent Cargo from being forced to re-update the
    /// registry every time it runs, and keeps the resolve in a locked state
    /// so it doesn't re-resolve the unused entries.
    unused_patches: Vec<PackageId>,
    /// A map from packages to a set of their public dependencies
    public_dependencies: HashMap<PackageId, HashSet<PackageId>>,
    /// Version of the `Cargo.lock` format, see
    /// `cargo::core::resolver::encode` for more.
    version: ResolveVersion,
}

/// A version to indicate how a `Cargo.lock` should be serialized. Currently V1
/// is the default and dates back to the origins of Cargo. A V2 is currently
/// being proposed which provides a much more compact representation of
/// dependency edges and also moves checksums out of `[metadata]`.
///
/// It's theorized that we can add more here over time to track larger changes
/// to the `Cargo.lock` format, but we've yet to see how that strategy pans out.
#[derive(PartialEq, Clone, Debug)]
pub enum ResolveVersion {
    V1,
    V2,
}

impl Resolve {
    pub fn new(
        graph: Graph<PackageId, Vec<Dependency>>,
        replacements: HashMap<PackageId, PackageId>,
        features: HashMap<PackageId, HashSet<String>>,
        checksums: HashMap<PackageId, Option<String>>,
        metadata: Metadata,
        unused_patches: Vec<PackageId>,
        version: ResolveVersion,
    ) -> Resolve {
        let reverse_replacements = replacements.iter().map(|(&p, &r)| (r, p)).collect();
        let public_dependencies = graph
            .iter()
            .map(|p| {
                let public_deps = graph
                    .edges(p)
                    .filter(|(_, deps)| {
                        deps.iter()
                            .any(|d| d.kind() == Kind::Normal && d.is_public())
                    })
                    .map(|(dep_package, _)| *dep_package)
                    .collect::<HashSet<PackageId>>();

                (*p, public_deps)
            })
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
            public_dependencies,
            version,
        }
    }

    /// Resolves one of the paths from the given dependent package up to
    /// the root.
    pub fn path_to_top<'a>(&'a self, pkg: &'a PackageId) -> Vec<&'a PackageId> {
        self.graph.path_to_top(pkg)
    }

    pub fn register_used_patches(&mut self, patches: &[Summary]) {
        for summary in patches {
            if self.iter().any(|id| id == summary.package_id()) {
                continue;
            }
            self.unused_patches.push(summary.package_id());
        }
    }

    pub fn merge_from(&mut self, previous: &Resolve) -> CargoResult<()> {
        // Given a previous instance of resolve, it should be forbidden to ever
        // have a checksums which *differ*. If the same package ID has differing
        // checksums, then something has gone wrong such as:
        //
        // * Something got seriously corrupted
        // * A "mirror" isn't actually a mirror as some changes were made
        // * A replacement source wasn't actually a replacement, some changes
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
                    failure::bail!(
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
                    failure::bail!(
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
                    failure::bail!(
                        "\
checksum for `{}` changed between lock files

this could be indicative of a few possible errors:

    * the lock file is corrupt
    * a replacement source in use (e.g., a mirror) returned a different checksum
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

        // The goal of Cargo is largely to preserve the encoding of
        // `Cargo.lock` that it finds on the filesystem. Sometimes `Cargo.lock`
        // changes are in the works where they haven't been set as the default
        // yet but will become the default soon. We want to preserve those
        // features if we find them.
        //
        // For this reason if the previous `Cargo.lock` is from the future, or
        // otherwise it looks like it's produced with future features we
        // understand, then the new resolve will be encoded with the same
        // version. Note that new instances of `Resolve` always use the default
        // encoding, and this is where we switch it to a future encoding if the
        // future encoding isn't yet the default.
        if previous.version.from_the_future() {
            self.version = previous.version.clone();
        }

        Ok(())
    }

    pub fn contains<Q: ?Sized>(&self, k: &Q) -> bool
    where
        PackageId: Borrow<Q>,
        Q: Ord + Eq,
    {
        self.graph.contains(k)
    }

    pub fn sort(&self) -> Vec<PackageId> {
        self.graph.sort()
    }

    pub fn iter<'a>(&'a self) -> impl Iterator<Item = PackageId> + 'a {
        self.graph.iter().cloned()
    }

    pub fn deps(&self, pkg: PackageId) -> impl Iterator<Item = (PackageId, &[Dependency])> {
        self.deps_not_replaced(pkg)
            .map(move |(id, deps)| (self.replacement(id).unwrap_or(id), deps))
    }

    pub fn deps_not_replaced(
        &self,
        pkg: PackageId,
    ) -> impl Iterator<Item = (PackageId, &[Dependency])> {
        self.graph
            .edges(&pkg)
            .map(|(id, deps)| (*id, deps.as_slice()))
    }

    pub fn replacement(&self, pkg: PackageId) -> Option<PackageId> {
        self.replacements.get(&pkg).cloned()
    }

    pub fn replacements(&self) -> &HashMap<PackageId, PackageId> {
        &self.replacements
    }

    pub fn features(&self, pkg: PackageId) -> &HashSet<String> {
        self.features.get(&pkg).unwrap_or(&self.empty_features)
    }

    pub fn is_public_dep(&self, pkg: PackageId, dep: PackageId) -> bool {
        self.public_dependencies
            .get(&pkg)
            .map(|public_deps| public_deps.contains(&dep))
            .unwrap_or_else(|| panic!("Unknown dependency {:?} for package {:?}", dep, pkg))
    }

    pub fn features_sorted(&self, pkg: PackageId) -> Vec<&str> {
        let mut v = Vec::from_iter(self.features(pkg).iter().map(|s| s.as_ref()));
        v.sort_unstable();
        v
    }

    pub fn query(&self, spec: &str) -> CargoResult<PackageId> {
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
        from: PackageId,
        to: PackageId,
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
                .unwrap_or_else(|| crate_name.clone())
        });
        let name = names.next().unwrap_or_else(|| crate_name.clone());
        for n in names {
            failure::ensure!(
                n == name,
                "the crate `{}` depends on crate `{}` multiple times with different names",
                from,
                to,
            );
        }
        Ok(name)
    }

    fn dependencies_listed(&self, from: PackageId, to: PackageId) -> &[Dependency] {
        // We've got a dependency on `from` to `to`, but this dependency edge
        // may be affected by [replace]. If the `to` package is listed as the
        // target of a replacement (aka the key of a reverse replacement map)
        // then we try to find our dependency edge through that. If that fails
        // then we go down below assuming it's not replaced.
        //
        // Note that we don't treat `from` as if it's been replaced because
        // that's where the dependency originates from, and we only replace
        // targets of dependencies not the originator.
        if let Some(replace) = self.reverse_replacements.get(&to) {
            if let Some(deps) = self.graph.edge(&from, replace) {
                return deps;
            }
        }
        match self.graph.edge(&from, &to) {
            Some(ret) => ret,
            None => panic!("no Dependency listed for `{}` => `{}`", from, to),
        }
    }

    /// Returns the version of the encoding that's being used for this lock
    /// file.
    pub fn version(&self) -> &ResolveVersion {
        &self.version
    }
}

impl fmt::Debug for Resolve {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(fmt, "graph: {:?}", self.graph)?;
        writeln!(fmt, "\nfeatures: {{")?;
        for (pkg, features) in &self.features {
            writeln!(fmt, "  {}: {:?}", pkg, features)?;
        }
        write!(fmt, "}}")
    }
}

impl ResolveVersion {
    /// The default way to encode `Cargo.lock`.
    ///
    /// This is used for new `Cargo.lock` files that are generated without a
    /// previous `Cargo.lock` files, and generally matches with what we want to
    /// encode.
    pub fn default() -> ResolveVersion {
        ResolveVersion::V1
    }

    /// Returns whether this encoding version is "from the future".
    ///
    /// This means that this encoding version is not currently the default but
    /// intended to become the default "soon".
    pub fn from_the_future(&self) -> bool {
        match self {
            ResolveVersion::V2 => true,
            ResolveVersion::V1 => false,
        }
    }
}
