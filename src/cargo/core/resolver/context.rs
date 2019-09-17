use std::collections::HashMap;
use std::num::NonZeroU64;
use std::rc::Rc;

// "ensure" seems to require "bail" be in scope (macro hygiene issue?).
#[allow(unused_imports)]
use failure::{bail, ensure};
use log::debug;

use crate::core::interning::InternedString;
use crate::core::{Dependency, PackageId, SourceId, Summary};
use crate::util::CargoResult;
use crate::util::Graph;

use super::dep_cache::RegistryQueryer;
use super::types::{ConflictMap, FeaturesSet, ResolveOpts};

pub use super::encode::Metadata;
pub use super::encode::{EncodableDependency, EncodablePackageId, EncodableResolve};
pub use super::resolve::Resolve;

// A `Context` is basically a bunch of local resolution information which is
// kept around for all `BacktrackFrame` instances. As a result, this runs the
// risk of being cloned *a lot* so we want to make this as cheap to clone as
// possible.
#[derive(Clone)]
pub struct Context {
    pub activations: Activations,
    /// list the features that are activated for each package
    pub resolve_features: im_rc::HashMap<PackageId, FeaturesSet>,
    /// get the package that will be linking to a native library by its links attribute
    pub links: im_rc::HashMap<InternedString, PackageId>,
    /// for each package the list of names it can see,
    /// then for each name the exact version that name represents and weather the name is public.
    pub public_dependency:
        Option<im_rc::HashMap<PackageId, im_rc::HashMap<InternedString, (PackageId, bool)>>>,

    /// a way to look up for a package in activations what packages required it
    /// and all of the exact deps that it fulfilled.
    pub parents: Graph<PackageId, Rc<Vec<Dependency>>>,
}

/// When backtracking it can be useful to know how far back to go.
/// The `ContextAge` of a `Context` is a monotonically increasing counter of the number
/// of decisions made to get to this state.
/// Several structures store the `ContextAge` when it was added,
/// to be used in `find_candidate` for backtracking.
pub type ContextAge = usize;

/// Find the activated version of a crate based on the name, source, and semver compatibility.
/// By storing this in a hash map we ensure that there is only one
/// semver compatible version of each crate.
/// This all so stores the `ContextAge`.
pub type Activations =
    im_rc::HashMap<(InternedString, SourceId, SemverCompatibility), (Summary, ContextAge)>;

/// A type that represents when cargo treats two Versions as compatible.
/// Versions `a` and `b` are compatible if their left-most nonzero digit is the
/// same.
#[derive(Clone, Copy, Eq, PartialEq, Hash, Debug)]
pub enum SemverCompatibility {
    Major(NonZeroU64),
    Minor(NonZeroU64),
    Patch(u64),
}

impl From<&semver::Version> for SemverCompatibility {
    fn from(ver: &semver::Version) -> Self {
        if let Some(m) = NonZeroU64::new(ver.major) {
            return SemverCompatibility::Major(m);
        }
        if let Some(m) = NonZeroU64::new(ver.minor) {
            return SemverCompatibility::Minor(m);
        }
        SemverCompatibility::Patch(ver.patch)
    }
}

impl PackageId {
    pub fn as_activations_key(self) -> (InternedString, SourceId, SemverCompatibility) {
        (self.name(), self.source_id(), self.version().into())
    }
}

impl Context {
    pub fn new(check_public_visible_dependencies: bool) -> Context {
        Context {
            resolve_features: im_rc::HashMap::new(),
            links: im_rc::HashMap::new(),
            public_dependency: if check_public_visible_dependencies {
                Some(im_rc::HashMap::new())
            } else {
                None
            },
            parents: Graph::new(),
            activations: im_rc::HashMap::new(),
        }
    }

    /// Activate this summary by inserting it into our list of known activations.
    ///
    /// The `parent` passed in here is the parent summary/dependency edge which
    /// cased `summary` to get activated. This may not be present for the root
    /// crate, for example.
    ///
    /// Returns `true` if this summary with the given features is already activated.
    pub fn flag_activated(
        &mut self,
        summary: &Summary,
        opts: &ResolveOpts,
        parent: Option<(&Summary, &Dependency)>,
    ) -> CargoResult<bool> {
        let id = summary.package_id();
        let age: ContextAge = self.age();
        match self.activations.entry(id.as_activations_key()) {
            im_rc::hashmap::Entry::Occupied(o) => {
                debug_assert_eq!(
                    &o.get().0,
                    summary,
                    "cargo does not allow two semver compatible versions"
                );
            }
            im_rc::hashmap::Entry::Vacant(v) => {
                if let Some(link) = summary.links() {
                    ensure!(
                        self.links.insert(link, id).is_none(),
                        "Attempting to resolve a dependency with more then one crate with the \
                         links={}.\nThis will not build as is. Consider rebuilding the .lock file.",
                        &*link
                    );
                }
                v.insert((summary.clone(), age));

                // If we've got a parent dependency which activated us, *and*
                // the dependency has a different source id listed than the
                // `summary` itself, then things get interesting. This basically
                // means that a `[patch]` was used to augment `dep.source_id()`
                // with `summary`.
                //
                // In this scenario we want to consider the activation key, as
                // viewed from the perspective of `dep.source_id()`, as being
                // fulfilled. This means that we need to add a second entry in
                // the activations map for the source that was patched, in
                // addition to the source of the actual `summary` itself.
                //
                // Without this it would be possible to have both 1.0.0 and
                // 1.1.0 "from crates.io" in a dependency graph if one of those
                // versions came from a `[patch]` source.
                if let Some((_, dep)) = parent {
                    if dep.source_id() != id.source_id() {
                        let key = (id.name(), dep.source_id(), id.version().into());
                        let prev = self.activations.insert(key, (summary.clone(), age));
                        assert!(prev.is_none());
                    }
                }

                return Ok(false);
            }
        }
        debug!("checking if {} is already activated", summary.package_id());
        if opts.all_features {
            return Ok(false);
        }

        let has_default_feature = summary.features().contains_key("default");
        Ok(match self.resolve_features.get(&id) {
            Some(prev) => {
                opts.features.is_subset(prev)
                    && (!opts.uses_default_features
                        || prev.contains("default")
                        || !has_default_feature)
            }
            None => {
                opts.features.is_empty() && (!opts.uses_default_features || !has_default_feature)
            }
        })
    }

    /// Returns the `ContextAge` of this `Context`.
    /// For now we use (len of activations) as the age.
    /// See the `ContextAge` docs for more details.
    pub fn age(&self) -> ContextAge {
        self.activations.len()
    }

    /// If the package is active returns the `ContextAge` when it was added
    pub fn is_active(&self, id: PackageId) -> Option<ContextAge> {
        self.activations
            .get(&id.as_activations_key())
            .and_then(|(s, l)| if s.package_id() == id { Some(*l) } else { None })
    }

    /// Checks whether all of `parent` and the keys of `conflicting activations`
    /// are still active.
    /// If so returns the `ContextAge` when the newest one was added.
    pub fn is_conflicting(
        &self,
        parent: Option<PackageId>,
        conflicting_activations: &ConflictMap,
    ) -> Option<usize> {
        let mut max = 0;
        for &id in conflicting_activations.keys().chain(parent.as_ref()) {
            if let Some(age) = self.is_active(id) {
                max = std::cmp::max(max, age);
            } else {
                return None;
            }
        }
        Some(max)
    }

    pub fn resolve_replacements(
        &self,
        registry: &RegistryQueryer<'_>,
    ) -> HashMap<PackageId, PackageId> {
        self.activations
            .values()
            .filter_map(|(s, _)| registry.used_replacement_for(s.package_id()))
            .collect()
    }

    pub fn graph(&self) -> Graph<PackageId, Vec<Dependency>> {
        let mut graph: Graph<PackageId, Vec<Dependency>> = Graph::new();
        self.activations
            .values()
            .for_each(|(r, _)| graph.add(r.package_id()));
        for i in self.parents.iter() {
            graph.add(*i);
            for (o, e) in self.parents.edges(i) {
                let old_link = graph.link(*o, *i);
                assert!(old_link.is_empty());
                *old_link = e.to_vec();
            }
        }
        graph
    }
}
