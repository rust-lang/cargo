//! Resolution of the entire dependency graph for a crate
//!
//! This module implements the core logic in taking the world of crates and
//! constraints and creating a resolved graph with locked versions for all
//! crates and their dependencies. This is separate from the registry module
//! which is more worried about discovering crates from various sources, this
//! module just uses the Registry trait as a source to learn about crates from.
//!
//! Actually solving a constraint graph is an NP-hard problem. This algorithm
//! is basically a nice heuristic to make sure we get roughly the best answer
//! most of the time. The constraints that we're working with are:
//!
//! 1. Each crate can have any number of dependencies. Each dependency can
//!    declare a version range that it is compatible with.
//! 2. Crates can be activated with multiple version (e.g. show up in the
//!    dependency graph twice) so long as each pairwise instance have
//!    semver-incompatible versions.
//!
//! The algorithm employed here is fairly simple, we simply do a DFS, activating
//! the "newest crate" (highest version) first and then going to the next
//! option. The heuristics we employ are:
//!
//! * Never try to activate a crate version which is incompatible. This means we
//!   only try crates which will actually satisfy a dependency and we won't ever
//!   try to activate a crate that's semver compatible with something else
//!   activated (as we're only allowed to have one) nor try to activate a crate
//!   that has the same links attribute as something else
//!   activated.
//! * Always try to activate the highest version crate first. The default
//!   dependency in Cargo (e.g. when you write `foo = "0.1.2"`) is
//!   semver-compatible, so selecting the highest version possible will allow us
//!   to hopefully satisfy as many dependencies at once.
//!
//! Beyond that, what's implemented below is just a naive backtracking version
//! which should in theory try all possible combinations of dependencies and
//! versions to see if one works. The first resolution that works causes
//! everything to bail out immediately and return success, and only if *nothing*
//! works do we actually return an error up the stack.
//!
//! ## Performance
//!
//! Note that this is a relatively performance-critical portion of Cargo. The
//! data that we're processing is proportional to the size of the dependency
//! graph, which can often be quite large (e.g. take a look at Servo). To make
//! matters worse the DFS algorithm we're implemented is inherently quite
//! inefficient. When we add the requirement of backtracking on top it means
//! that we're implementing something that probably shouldn't be allocating all
//! over the place.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BinaryHeap, HashMap, HashSet};
use std::fmt;
use std::iter::FromIterator;
use std::ops::Range;
use std::rc::Rc;
use std::time::{Duration, Instant};

use semver;
use url::Url;

use core::{Dependency, PackageId, Registry, SourceId, Summary};
use core::PackageIdSpec;
use core::interning::InternedString;
use util::config::Config;
use util::Graph;
use util::errors::{CargoError, CargoResult};
use util::profile;
use util::graph::{Edges, Nodes};

pub use self::encode::{EncodableDependency, EncodablePackageId, EncodableResolve};
pub use self::encode::{Metadata, WorkspaceResolve};

mod encode;

/// Represents a fully resolved package dependency graph. Each node in the graph
/// is a package and edges represent dependencies between packages.
///
/// Each instance of `Resolve` also understands the full set of features used
/// for each package.
#[derive(PartialEq)]
pub struct Resolve {
    graph: Graph<PackageId>,
    replacements: HashMap<PackageId, PackageId>,
    empty_features: HashSet<String>,
    features: HashMap<PackageId, HashSet<String>>,
    checksums: HashMap<PackageId, Option<String>>,
    metadata: Metadata,
    unused_patches: Vec<PackageId>,
}

pub struct Deps<'a> {
    edges: Option<Edges<'a, PackageId>>,
    resolve: &'a Resolve,
}

pub struct DepsNotReplaced<'a> {
    edges: Option<Edges<'a, PackageId>>,
}

#[derive(Clone, Copy)]
pub enum Method<'a> {
    Everything, // equivalent to Required { dev_deps: true, all_features: true, .. }
    Required {
        dev_deps: bool,
        features: &'a [String],
        all_features: bool,
        uses_default_features: bool,
    },
}

impl<'r> Method<'r> {
    pub fn split_features(features: &[String]) -> Vec<String> {
        features
            .iter()
            .flat_map(|s| s.split_whitespace())
            .flat_map(|s| s.split(','))
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect::<Vec<String>>()
    }
}

// Information about the dependencies for a crate, a tuple of:
//
// (dependency info, candidates, features activated)
type DepInfo = (Dependency, Rc<Vec<Candidate>>, Rc<Vec<String>>);

#[derive(Clone)]
struct Candidate {
    summary: Summary,
    replace: Option<Summary>,
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

impl<'a> Iterator for Deps<'a> {
    type Item = &'a PackageId;

    fn next(&mut self) -> Option<&'a PackageId> {
        self.edges
            .as_mut()
            .and_then(|e| e.next())
            .map(|id| self.resolve.replacement(id).unwrap_or(id))
    }
}

impl<'a> Iterator for DepsNotReplaced<'a> {
    type Item = &'a PackageId;

    fn next(&mut self) -> Option<&'a PackageId> {
        self.edges.as_mut().and_then(|e| e.next())
    }
}

struct RcList<T> {
    head: Option<Rc<(T, RcList<T>)>>,
}

impl<T> RcList<T> {
    fn new() -> RcList<T> {
        RcList { head: None }
    }

    fn push(&mut self, data: T) {
        let node = Rc::new((
            data,
            RcList {
                head: self.head.take(),
            },
        ));
        self.head = Some(node);
    }
}

// Not derived to avoid `T: Clone`
impl<T> Clone for RcList<T> {
    fn clone(&self) -> RcList<T> {
        RcList {
            head: self.head.clone(),
        }
    }
}

// Avoid stack overflows on drop by turning recursion into a loop
impl<T> Drop for RcList<T> {
    fn drop(&mut self) {
        let mut cur = self.head.take();
        while let Some(head) = cur {
            match Rc::try_unwrap(head) {
                Ok((_data, mut next)) => cur = next.head.take(),
                Err(_) => break,
            }
        }
    }
}

enum GraphNode {
    Add(PackageId),
    Link(PackageId, PackageId),
}

// A `Context` is basically a bunch of local resolution information which is
// kept around for all `BacktrackFrame` instances. As a result, this runs the
// risk of being cloned *a lot* so we want to make this as cheap to clone as
// possible.
#[derive(Clone)]
struct Context {
    // TODO: Both this and the two maps below are super expensive to clone. We should
    //       switch to persistent hash maps if we can at some point or otherwise
    //       make these much cheaper to clone in general.
    activations: Activations,
    resolve_features: HashMap<PackageId, HashSet<InternedString>>,
    links: HashMap<InternedString, PackageId>,

    // These are two cheaply-cloneable lists (O(1) clone) which are effectively
    // hash maps but are built up as "construction lists". We'll iterate these
    // at the very end and actually construct the map that we're making.
    resolve_graph: RcList<GraphNode>,
    resolve_replacements: RcList<(PackageId, PackageId)>,

    // These warnings are printed after resolution.
    warnings: RcList<String>,
}

type Activations = HashMap<(InternedString, SourceId), Rc<Vec<Summary>>>;

/// Builds the list of all packages required to build the first argument.
pub fn resolve(
    summaries: &[(Summary, Method)],
    replacements: &[(PackageIdSpec, Dependency)],
    registry: &mut Registry,
    config: Option<&Config>,
    print_warnings: bool,
) -> CargoResult<Resolve> {
    let cx = Context {
        resolve_graph: RcList::new(),
        resolve_features: HashMap::new(),
        links: HashMap::new(),
        resolve_replacements: RcList::new(),
        activations: HashMap::new(),
        warnings: RcList::new(),
    };
    let _p = profile::start("resolving");
    let cx = activate_deps_loop(
        cx,
        &mut RegistryQueryer::new(registry, replacements),
        summaries,
        config,
    )?;

    let mut resolve = Resolve {
        graph: cx.graph(),
        empty_features: HashSet::new(),
        checksums: HashMap::new(),
        metadata: BTreeMap::new(),
        replacements: cx.resolve_replacements(),
        features: cx.resolve_features
            .iter()
            .map(|(k, v)| (k.clone(), v.iter().map(|x| x.to_string()).collect()))
            .collect(),
        unused_patches: Vec::new(),
    };

    for summary in cx.activations.values().flat_map(|v| v.iter()) {
        let cksum = summary.checksum().map(|s| s.to_string());
        resolve
            .checksums
            .insert(summary.package_id().clone(), cksum);
    }

    check_cycles(&resolve, &cx.activations)?;
    trace!("resolved: {:?}", resolve);

    // If we have a shell, emit warnings about required deps used as feature.
    if let Some(config) = config {
        if print_warnings {
            let mut shell = config.shell();
            let mut warnings = &cx.warnings;
            while let Some(ref head) = warnings.head {
                shell.warn(&head.0)?;
                warnings = &head.1;
            }
        }
    }

    Ok(resolve)
}

/// Attempts to activate the summary `candidate` in the context `cx`.
///
/// This function will pull dependency summaries from the registry provided, and
/// the dependencies of the package will be determined by the `method` provided.
/// If `candidate` was activated, this function returns the dependency frame to
/// iterate through next.
fn activate(
    cx: &mut Context,
    registry: &mut RegistryQueryer,
    parent: Option<&Summary>,
    candidate: Candidate,
    method: &Method,
) -> ActivateResult<Option<(DepsFrame, Duration)>> {
    if let Some(parent) = parent {
        cx.resolve_graph.push(GraphNode::Link(
            parent.package_id().clone(),
            candidate.summary.package_id().clone(),
        ));
    }

    let activated = cx.flag_activated(&candidate.summary, method)?;

    let candidate = match candidate.replace {
        Some(replace) => {
            cx.resolve_replacements.push((
                candidate.summary.package_id().clone(),
                replace.package_id().clone(),
            ));
            if cx.flag_activated(&replace, method)? && activated {
                return Ok(None);
            }
            trace!(
                "activating {} (replacing {})",
                replace.package_id(),
                candidate.summary.package_id()
            );
            replace
        }
        None => {
            if activated {
                return Ok(None);
            }
            trace!("activating {}", candidate.summary.package_id());
            candidate.summary
        }
    };

    let now = Instant::now();
    let deps = cx.build_deps(registry, parent, &candidate, method)?;
    let frame = DepsFrame {
        parent: candidate,
        just_for_error_messages: false,
        remaining_siblings: RcVecIter::new(Rc::new(deps)),
    };
    Ok(Some((frame, now.elapsed())))
}

struct RcVecIter<T> {
    vec: Rc<Vec<T>>,
    rest: Range<usize>,
}

impl<T> RcVecIter<T> {
    fn new(vec: Rc<Vec<T>>) -> RcVecIter<T> {
        RcVecIter {
            rest: 0..vec.len(),
            vec,
        }
    }
}

// Not derived to avoid `T: Clone`
impl<T> Clone for RcVecIter<T> {
    fn clone(&self) -> RcVecIter<T> {
        RcVecIter {
            vec: self.vec.clone(),
            rest: self.rest.clone(),
        }
    }
}

impl<T> Iterator for RcVecIter<T>
where
    T: Clone,
{
    type Item = (usize, T);

    fn next(&mut self) -> Option<(usize, T)> {
        self.rest
            .next()
            .and_then(|i| self.vec.get(i).map(|val| (i, val.clone())))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.rest.size_hint()
    }
}

#[derive(Clone)]
struct DepsFrame {
    parent: Summary,
    just_for_error_messages: bool,
    remaining_siblings: RcVecIter<DepInfo>,
}

impl DepsFrame {
    /// Returns the least number of candidates that any of this frame's siblings
    /// has.
    ///
    /// The `remaining_siblings` array is already sorted with the smallest
    /// number of candidates at the front, so we just return the number of
    /// candidates in that entry.
    fn min_candidates(&self) -> usize {
        self.remaining_siblings
            .clone()
            .next()
            .map(|(_, (_, candidates, _))| candidates.len())
            .unwrap_or(0)
    }
}

impl PartialEq for DepsFrame {
    fn eq(&self, other: &DepsFrame) -> bool {
        self.just_for_error_messages == other.just_for_error_messages
            && self.min_candidates() == other.min_candidates()
    }
}

impl Eq for DepsFrame {}

impl PartialOrd for DepsFrame {
    fn partial_cmp(&self, other: &DepsFrame) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for DepsFrame {
    fn cmp(&self, other: &DepsFrame) -> Ordering {
        self.just_for_error_messages
            .cmp(&other.just_for_error_messages)
            .then_with(||
            // the frame with the sibling that has the least number of candidates
            // needs to get bubbled up to the top of the heap we use below, so
            // reverse comparison here.
            self.min_candidates().cmp(&other.min_candidates()).reverse())
    }
}

#[derive(Debug, Clone, PartialOrd, Ord, PartialEq, Eq)]
enum ConflictReason {
    Semver,
    Links(String),
    MissingFeatures(String),
}

enum ActivateError {
    Error(::failure::Error),
    Conflict(PackageId, ConflictReason),
}
type ActivateResult<T> = Result<T, ActivateError>;

impl From<::failure::Error> for ActivateError {
    fn from(t: ::failure::Error) -> Self {
        ActivateError::Error(t)
    }
}

impl From<(PackageId, ConflictReason)> for ActivateError {
    fn from(t: (PackageId, ConflictReason)) -> Self {
        ActivateError::Conflict(t.0, t.1)
    }
}

impl ConflictReason {
    fn is_links(&self) -> bool {
        if let ConflictReason::Links(_) = *self {
            return true;
        }
        false
    }

    fn is_missing_features(&self) -> bool {
        if let ConflictReason::MissingFeatures(_) = *self {
            return true;
        }
        false
    }
}

struct RegistryQueryer<'a> {
    registry: &'a mut (Registry + 'a),
    replacements: &'a [(PackageIdSpec, Dependency)],
    // TODO: with nll the Rc can be removed
    cache: HashMap<Dependency, Rc<Vec<Candidate>>>,
}

impl<'a> RegistryQueryer<'a> {
    fn new(registry: &'a mut Registry, replacements: &'a [(PackageIdSpec, Dependency)]) -> Self {
        RegistryQueryer {
            registry,
            replacements,
            cache: HashMap::new(),
        }
    }

    /// Queries the `registry` to return a list of candidates for `dep`.
    ///
    /// This method is the location where overrides are taken into account. If
    /// any candidates are returned which match an override then the override is
    /// applied by performing a second query for what the override should
    /// return.
    fn query(&mut self, dep: &Dependency) -> CargoResult<Rc<Vec<Candidate>>> {
        if let Some(out) = self.cache.get(dep).cloned() {
            return Ok(out);
        }

        let mut ret = Vec::new();
        self.registry.query(dep, &mut |s| {
            ret.push(Candidate {
                summary: s,
                replace: None,
            });
        })?;
        for candidate in ret.iter_mut() {
            let summary = &candidate.summary;

            let mut potential_matches = self.replacements
                .iter()
                .filter(|&&(ref spec, _)| spec.matches(summary.package_id()));

            let &(ref spec, ref dep) = match potential_matches.next() {
                None => continue,
                Some(replacement) => replacement,
            };
            debug!("found an override for {} {}", dep.name(), dep.version_req());

            let mut summaries = self.registry.query_vec(dep)?.into_iter();
            let s = summaries.next().ok_or_else(|| {
                format_err!(
                    "no matching package for override `{}` found\n\
                     location searched: {}\n\
                     version required: {}",
                    spec,
                    dep.source_id(),
                    dep.version_req()
                )
            })?;
            let summaries = summaries.collect::<Vec<_>>();
            if !summaries.is_empty() {
                let bullets = summaries
                    .iter()
                    .map(|s| format!("  * {}", s.package_id()))
                    .collect::<Vec<_>>();
                bail!(
                    "the replacement specification `{}` matched \
                     multiple packages:\n  * {}\n{}",
                    spec,
                    s.package_id(),
                    bullets.join("\n")
                );
            }

            // The dependency should be hard-coded to have the same name and an
            // exact version requirement, so both of these assertions should
            // never fail.
            assert_eq!(s.version(), summary.version());
            assert_eq!(s.name(), summary.name());

            let replace = if s.source_id() == summary.source_id() {
                debug!("Preventing\n{:?}\nfrom replacing\n{:?}", summary, s);
                None
            } else {
                Some(s)
            };
            let matched_spec = spec.clone();

            // Make sure no duplicates
            if let Some(&(ref spec, _)) = potential_matches.next() {
                bail!(
                    "overlapping replacement specifications found:\n\n  \
                     * {}\n  * {}\n\nboth specifications match: {}",
                    matched_spec,
                    spec,
                    summary.package_id()
                );
            }

            for dep in summary.dependencies() {
                debug!("\t{} => {}", dep.name(), dep.version_req());
            }

            candidate.replace = replace;
        }

        // When we attempt versions for a package, we'll want to start at
        // the maximum version and work our way down.
        ret.sort_unstable_by(|a, b| b.summary.version().cmp(a.summary.version()));

        let out = Rc::new(ret);

        self.cache.insert(dep.clone(), out.clone());

        Ok(out)
    }
}

#[derive(Clone)]
struct BacktrackFrame {
    cur: usize,
    context_backup: Context,
    deps_backup: BinaryHeap<DepsFrame>,
    remaining_candidates: RemainingCandidates,
    parent: Summary,
    dep: Dependency,
    features: Rc<Vec<String>>,
    conflicting_activations: HashMap<PackageId, ConflictReason>,
}

#[derive(Clone)]
struct RemainingCandidates {
    remaining: RcVecIter<Candidate>,
    // note: change to RcList or something if clone is to expensive
    conflicting_prev_active: HashMap<PackageId, ConflictReason>,
    // This is a inlined peekable generator
    has_another: Option<Candidate>,
}

impl RemainingCandidates {
    fn new(candidates: &Rc<Vec<Candidate>>) -> RemainingCandidates {
        RemainingCandidates {
            remaining: RcVecIter::new(Rc::clone(candidates)),
            conflicting_prev_active: HashMap::new(),
            has_another: None,
        }
    }

    fn next(
        &mut self,
        prev_active: &[Summary],
        links: &HashMap<InternedString, PackageId>,
    ) -> Result<(Candidate, bool), HashMap<PackageId, ConflictReason>> {
        // Filter the set of candidates based on the previously activated
        // versions for this dependency. We can actually use a version if it
        // precisely matches an activated version or if it is otherwise
        // incompatible with all other activated versions. Note that we
        // define "compatible" here in terms of the semver sense where if
        // the left-most nonzero digit is the same they're considered
        // compatible unless we have a `*-sys` crate (defined by having a
        // linked attribute) then we can only have one version.
        //
        // When we are done we return the set of previously activated
        // that conflicted with the ones we tried. If any of these change
        // then we would have considered different candidates.
        use std::mem::replace;
        for (_, b) in self.remaining.by_ref() {
            if let Some(link) = b.summary.links() {
                if let Some(a) = links.get(&link) {
                    if a != b.summary.package_id() {
                        self.conflicting_prev_active
                            .entry(a.clone())
                            .or_insert_with(|| ConflictReason::Links(link.to_string()));
                        continue;
                    }
                }
            }
            if let Some(a) = prev_active
                .iter()
                .find(|a| compatible(a.version(), b.summary.version()))
            {
                if *a != b.summary {
                    self.conflicting_prev_active
                        .entry(a.package_id().clone())
                        .or_insert(ConflictReason::Semver);
                    continue;
                }
            }
            if let Some(r) = replace(&mut self.has_another, Some(b)) {
                return Ok((r, true));
            }
        }
        replace(&mut self.has_another, None)
            .map(|r| (r, false))
            .ok_or_else(|| self.conflicting_prev_active.clone())
    }
}
/// Recursively activates the dependencies for `top`, in depth-first order,
/// backtracking across possible candidates for each dependency as necessary.
///
/// If all dependencies can be activated and resolved to a version in the
/// dependency graph, cx.resolve is returned.
fn activate_deps_loop(
    mut cx: Context,
    registry: &mut RegistryQueryer,
    summaries: &[(Summary, Method)],
    config: Option<&Config>,
) -> CargoResult<Context> {
    // Note that a `BinaryHeap` is used for the remaining dependencies that need
    // activation. This heap is sorted such that the "largest value" is the most
    // constrained dependency, or the one with the least candidates.
    //
    // This helps us get through super constrained portions of the dependency
    // graph quickly and hopefully lock down what later larger dependencies can
    // use (those with more candidates).
    let mut backtrack_stack = Vec::new();
    let mut remaining_deps = BinaryHeap::new();
    // `past_conflicting_activations`is a cache of the reasons for each time we backtrack.
    // for example after several backtracks we may have:
    // past_conflicting_activations[`foo = "^1.0.2"`] = vac![map!{`foo=1.0.1`: Semver}, map!{`foo=1.0.1`: Semver}];
    // This can be read as "we cannot find a candidate for dep `foo = "^1.0.2"` if either
    // `foo=1.0.1` OR `foo=1.0.0` are activated"
    // for another example after several backtracks we may have:
    // past_conflicting_activations[`foo = ">=0.8.2, <=0.9.3"`] = vac![map!{`foo=0.8.1`: Semver, `foo=0.9.4`: Semver}];
    // This can be read as "we cannot find a candidate for dep `foo = ">=0.8.2, <=0.9.3"` if both
    // `foo=0.8.1` AND `foo=0.9.4` are activated" (better data structures are welcome but this works for now.)
    // This is used to make sure we don't queue work we know will fail.
    // See the discussion in https://github.com/rust-lang/cargo/pull/5168 for why this is so important
    let mut past_conflicting_activations: HashMap<
        Dependency,
        Vec<HashMap<PackageId, ConflictReason>>,
    > = HashMap::new();
    for &(ref summary, ref method) in summaries {
        debug!("initial activation: {}", summary.package_id());
        let candidate = Candidate {
            summary: summary.clone(),
            replace: None,
        };
        let res = activate(&mut cx, registry, None, candidate, method);
        match res {
            Ok(Some((frame, _))) => remaining_deps.push(frame),
            Ok(None) => (),
            Err(ActivateError::Error(e)) => return Err(e),
            Err(ActivateError::Conflict(_, _)) => panic!("bad error from activate"),
        }
    }

    let mut ticks = 0;
    let start = Instant::now();
    let time_to_print = Duration::from_millis(500);
    let mut printed = false;
    let mut deps_time = Duration::new(0, 0);

    // Main resolution loop, this is the workhorse of the resolution algorithm.
    //
    // You'll note that a few stacks are maintained on the side, which might
    // seem odd when this algorithm looks like it could be implemented
    // recursively. While correct, this is implemented iteratively to avoid
    // blowing the stack (the recursion depth is proportional to the size of the
    // input).
    //
    // The general sketch of this loop is to run until there are no dependencies
    // left to activate, and for each dependency to attempt to activate all of
    // its own dependencies in turn. The `backtrack_stack` is a side table of
    // backtracking states where if we hit an error we can return to in order to
    // attempt to continue resolving.
    while let Some(mut deps_frame) = remaining_deps.pop() {
        // If we spend a lot of time here (we shouldn't in most cases) then give
        // a bit of a visual indicator as to what we're doing. Only enable this
        // when stderr is a tty (a human is likely to be watching) to ensure we
        // get deterministic output otherwise when observed by tools.
        //
        // Also note that we hit this loop a lot, so it's fairly performance
        // sensitive. As a result try to defer a possibly expensive operation
        // like `Instant::now` by only checking every N iterations of this loop
        // to amortize the cost of the current time lookup.
        ticks += 1;
        if let Some(config) = config {
            if config.shell().is_err_tty() && !printed && ticks % 1000 == 0
                && start.elapsed() - deps_time > time_to_print
            {
                printed = true;
                config.shell().status("Resolving", "dependency graph...")?;
            }
        }

        let just_here_for_the_error_messages = deps_frame.just_for_error_messages;

        let frame = match deps_frame.remaining_siblings.next() {
            Some(sibling) => {
                let parent = Summary::clone(&deps_frame.parent);
                remaining_deps.push(deps_frame);
                (parent, sibling)
            }
            None => continue,
        };
        let (mut parent, (mut cur, (mut dep, candidates, mut features))) = frame;
        assert!(!remaining_deps.is_empty());

        trace!(
            "{}[{}]>{} {} candidates",
            parent.name(),
            cur,
            dep.name(),
            candidates.len()
        );
        trace!(
            "{}[{}]>{} {} prev activations",
            parent.name(),
            cur,
            dep.name(),
            cx.prev_active(&dep).len()
        );

        let just_here_for_the_error_messages = just_here_for_the_error_messages
            && past_conflicting_activations
                .get(&dep)
                .and_then(|past_bad| {
                    past_bad
                        .iter()
                        .find(|conflicting| cx.is_conflicting(None, conflicting))
                })
                .is_some();

        let mut remaining_candidates = RemainingCandidates::new(&candidates);
        let mut successfully_activated = false;
        // `conflicting_activations` stores all the reasons we were unable to activate candidates.
        // One of these reasons will have to go away for backtracking to find a place to restart.
        // It is also the list of things to explain in the error message if we fail to resolve.
        let mut conflicting_activations = HashMap::new();
        // When backtracking we don't fully update `conflicting_activations` especially for the
        // cases that we didn't make a backtrack frame in the first place.
        // This `backtracked` var stores whether we are continuing from a restored backtrack frame
        // so that we can skip caching `conflicting_activations` in `past_conflicting_activations`
        let mut backtracked = false;

        while !successfully_activated {
            let next = remaining_candidates.next(cx.prev_active(&dep), &cx.links);

            // Alright, for each candidate that's gotten this far, it meets the
            // following requirements:
            //
            // 1. The version matches the dependency requirement listed for this
            //    package
            // 2. There are no activated versions for this package which are
            //    semver/links-compatible, or there's an activated version which is
            //    precisely equal to `candidate`.
            //
            // This means that we're going to attempt to activate each candidate in
            // turn. We could possibly fail to activate each candidate, so we try
            // each one in turn.
            let (candidate, has_another) = next.or_else(|conflicting| {
                conflicting_activations.extend(conflicting);
                // This dependency has no valid candidate. Backtrack until we
                // find a dependency that does have a candidate to try, and try
                // to activate that one.
                trace!("{}[{}]>{} -- no candidates", parent.name(), cur, dep.name());

                if !just_here_for_the_error_messages && !backtracked {
                    // if `just_here_for_the_error_messages` then skip as it is already known to be bad.
                    // if `backtracked` then `conflicting_activations` may not be complete so skip.
                    let past = past_conflicting_activations
                        .entry(dep.clone())
                        .or_insert_with(Vec::new);
                    if !past.contains(&conflicting_activations) {
                        trace!(
                            "{}[{}]>{} adding a skip {:?}",
                            parent.name(),
                            cur,
                            dep.name(),
                            conflicting_activations
                        );
                        past.push(conflicting_activations.clone());
                    }
                }

                find_candidate(&mut backtrack_stack, &parent, &conflicting_activations)
                    .map(|(candidate, has_another, frame)| {
                        // This resets the `remaining_deps` to
                        // their state at the found level of the `backtrack_stack`.
                        cur = frame.cur;
                        cx = frame.context_backup;
                        remaining_deps = frame.deps_backup;
                        remaining_candidates = frame.remaining_candidates;
                        parent = frame.parent;
                        dep = frame.dep;
                        features = frame.features;
                        conflicting_activations = frame.conflicting_activations;
                        backtracked = true;
                        (candidate, has_another)
                    })
                    .ok_or_else(|| {
                        activation_error(
                            &cx,
                            registry.registry,
                            &parent,
                            &dep,
                            &conflicting_activations,
                            &candidates,
                            config,
                        )
                    })
            })?;

            if just_here_for_the_error_messages && !backtracked && has_another {
                continue;
            }

            // We have a candidate. Clone a `BacktrackFrame`
            // so we can add it to the `backtrack_stack` if activation succeeds.
            // We clone now in case `activate` changes `cx` and then fails.
            let backtrack = if has_another {
                Some(BacktrackFrame {
                    cur,
                    context_backup: Context::clone(&cx),
                    deps_backup: <BinaryHeap<DepsFrame>>::clone(&remaining_deps),
                    remaining_candidates: remaining_candidates.clone(),
                    parent: Summary::clone(&parent),
                    dep: Dependency::clone(&dep),
                    features: Rc::clone(&features),
                    conflicting_activations: conflicting_activations.clone(),
                })
            } else {
                None
            };

            let pid = candidate.summary.package_id().clone();
            let method = Method::Required {
                dev_deps: false,
                features: &features,
                all_features: false,
                uses_default_features: dep.uses_default_features(),
            };
            trace!(
                "{}[{}]>{} trying {}",
                parent.name(),
                cur,
                dep.name(),
                candidate.summary.version()
            );
            let res = activate(&mut cx, registry, Some(&parent), candidate, &method);
            successfully_activated = res.is_ok();

            match res {
                Ok(Some((mut frame, dur))) => {
                    // at this point we have technically already activated
                    // but we may want to scrap it if it is not going to end well
                    let mut has_past_conflicting_dep = just_here_for_the_error_messages;
                    if !has_past_conflicting_dep {
                        if let Some(conflicting) = frame
                            .remaining_siblings
                            .clone()
                            .filter_map(|(_, (new_dep, _, _))| {
                                past_conflicting_activations.get(&new_dep)
                            })
                            .flat_map(|x| x)
                            .find(|con| cx.is_conflicting(None, con))
                        {
                            // if any of them match than it will just backtrack to us
                            // so let's save the effort.
                            conflicting_activations.extend(conflicting.clone());
                            has_past_conflicting_dep = true;
                        }
                    }
                    // if not has_another we we activate for the better error messages
                    frame.just_for_error_messages = has_past_conflicting_dep;
                    if !has_past_conflicting_dep
                        || (!has_another && (just_here_for_the_error_messages || {
                            conflicting_activations
                                .extend(remaining_candidates.conflicting_prev_active.clone());
                            find_candidate(
                                &mut backtrack_stack.clone(),
                                &parent,
                                &conflicting_activations,
                            ).is_none()
                        })) {
                        remaining_deps.push(frame);
                    } else {
                        trace!(
                            "{}[{}]>{} skipping {} ",
                            parent.name(),
                            cur,
                            dep.name(),
                            pid.version()
                        );
                        successfully_activated = false;
                    }
                    deps_time += dur;
                }
                Ok(None) => (),
                Err(ActivateError::Error(e)) => return Err(e),
                Err(ActivateError::Conflict(id, reason)) => {
                    conflicting_activations.insert(id, reason);
                }
            }

            // Add an entry to the `backtrack_stack` so
            // we can try the next one if this one fails.
            if successfully_activated {
                if let Some(b) = backtrack {
                    backtrack_stack.push(b);
                }
            } else {
                if let Some(b) = backtrack {
                    // `activate` changed `cx` and then failed so put things back.
                    cx = b.context_backup;
                } // else we are just using the cx for the error message so we can live with a corrupted one
            }
        }
    }

    Ok(cx)
}

/// Looks through the states in `backtrack_stack` for dependencies with
/// remaining candidates. For each one, also checks if rolling back
/// could change the outcome of the failed resolution that caused backtracking
/// in the first place. Namely, if we've backtracked past the parent of the
/// failed dep, or any of the packages flagged as giving us trouble in `conflicting_activations`.
/// Read <https://github.com/rust-lang/cargo/pull/4834>
/// For several more detailed explanations of the logic here.
///
/// If the outcome could differ, resets `cx` and `remaining_deps` to that
/// level and returns the next candidate.
/// If all candidates have been exhausted, returns None.
fn find_candidate<'a>(
    backtrack_stack: &mut Vec<BacktrackFrame>,
    parent: &Summary,
    conflicting_activations: &HashMap<PackageId, ConflictReason>,
) -> Option<(Candidate, bool, BacktrackFrame)> {
    while let Some(mut frame) = backtrack_stack.pop() {
        let next = frame.remaining_candidates.next(
            frame.context_backup.prev_active(&frame.dep),
            &frame.context_backup.links,
        );
        if frame
            .context_backup
            .is_conflicting(Some(parent.package_id()), conflicting_activations)
        {
            continue;
        }
        if let Ok((candidate, has_another)) = next {
            return Some((candidate, has_another, frame));
        }
    }
    None
}

/// Returns String representation of dependency chain for a particular `pkgid`.
fn describe_path(graph: &Graph<PackageId>, pkgid: &PackageId) -> String {
    use std::fmt::Write;
    let dep_path = graph.path_to_top(pkgid);
    let mut dep_path_desc = format!("package `{}`", dep_path[0]);
    for dep in dep_path.iter().skip(1) {
        write!(dep_path_desc, "\n    ... which is depended on by `{}`", dep).unwrap();
    }
    dep_path_desc
}

fn activation_error(
    cx: &Context,
    registry: &mut Registry,
    parent: &Summary,
    dep: &Dependency,
    conflicting_activations: &HashMap<PackageId, ConflictReason>,
    candidates: &[Candidate],
    config: Option<&Config>,
) -> CargoError {
    let graph = cx.graph();
    if !candidates.is_empty() {
        let mut msg = format!("failed to select a version for `{}`.", dep.name());
        msg.push_str("\n    ... required by ");
        msg.push_str(&describe_path(&graph, parent.package_id()));

        msg.push_str("\nversions that meet the requirements `");
        msg.push_str(&dep.version_req().to_string());
        msg.push_str("` are: ");
        msg.push_str(&candidates
            .iter()
            .map(|v| v.summary.version())
            .map(|v| v.to_string())
            .collect::<Vec<_>>()
            .join(", "));

        let mut conflicting_activations: Vec<_> = conflicting_activations.iter().collect();
        conflicting_activations.sort_unstable();
        let (links_errors, mut other_errors): (Vec<_>, Vec<_>) = conflicting_activations
            .drain(..)
            .rev()
            .partition(|&(_, r)| r.is_links());

        for &(p, r) in links_errors.iter() {
            if let ConflictReason::Links(ref link) = *r {
                msg.push_str("\n\nthe package `");
                msg.push_str(&*dep.name());
                msg.push_str("` links to the native library `");
                msg.push_str(link);
                msg.push_str("`, but it conflicts with a previous package which links to `");
                msg.push_str(link);
                msg.push_str("` as well:\n");
            }
            msg.push_str(&describe_path(&graph, p));
        }

        let (features_errors, other_errors): (Vec<_>, Vec<_>) = other_errors
            .drain(..)
            .partition(|&(_, r)| r.is_missing_features());

        for &(p, r) in features_errors.iter() {
            if let ConflictReason::MissingFeatures(ref features) = *r {
                msg.push_str("\n\nthe package `");
                msg.push_str(&*p.name());
                msg.push_str("` depends on `");
                msg.push_str(&*dep.name());
                msg.push_str("`, with features: `");
                msg.push_str(features);
                msg.push_str("` but `");
                msg.push_str(&*dep.name());
                msg.push_str("` does not have these features.\n");
            }
            // p == parent so the full path is redundant.
        }

        if !other_errors.is_empty() {
            msg.push_str(
                "\n\nall possible versions conflict with \
                 previously selected packages.",
            );
        }

        for &(p, _) in other_errors.iter() {
            msg.push_str("\n\n  previously selected ");
            msg.push_str(&describe_path(&graph, p));
        }

        msg.push_str("\n\nfailed to select a version for `");
        msg.push_str(&*dep.name());
        msg.push_str("` which could resolve this conflict");

        return format_err!("{}", msg);
    }

    // Once we're all the way down here, we're definitely lost in the
    // weeds! We didn't actually find any candidates, so we need to
    // give an error message that nothing was found.
    //
    // Note that we re-query the registry with a new dependency that
    // allows any version so we can give some nicer error reporting
    // which indicates a few versions that were actually found.
    let all_req = semver::VersionReq::parse("*").unwrap();
    let mut new_dep = dep.clone();
    new_dep.set_version_req(all_req);
    let mut candidates = match registry.query_vec(&new_dep) {
        Ok(candidates) => candidates,
        Err(e) => return e,
    };
    candidates.sort_unstable_by(|a, b| b.version().cmp(a.version()));

    let mut msg = if !candidates.is_empty() {
        let versions = {
            let mut versions = candidates
                .iter()
                .take(3)
                .map(|cand| cand.version().to_string())
                .collect::<Vec<_>>();

            if candidates.len() > 3 {
                versions.push("...".into());
            }

            versions.join(", ")
        };

        let mut msg = format!(
            "no matching version `{}` found for package `{}`\n\
             location searched: {}\n\
             versions found: {}\n",
            dep.version_req(),
            dep.name(),
            dep.source_id(),
            versions
        );
        msg.push_str("required by ");
        msg.push_str(&describe_path(&graph, parent.package_id()));

        // If we have a path dependency with a locked version, then this may
        // indicate that we updated a sub-package and forgot to run `cargo
        // update`. In this case try to print a helpful error!
        if dep.source_id().is_path() && dep.version_req().to_string().starts_with('=') {
            msg.push_str(
                "\nconsider running `cargo update` to update \
                 a path dependency's locked version",
            );
        }

        msg
    } else {
        let mut msg = format!(
            "no matching package named `{}` found\n\
             location searched: {}\n",
            dep.name(),
            dep.source_id()
        );
        msg.push_str("required by ");
        msg.push_str(&describe_path(&graph, parent.package_id()));

        msg
    };

    if let Some(config) = config {
        if config.cli_unstable().offline {
            msg.push_str(
                "\nAs a reminder, you're using offline mode (-Z offline) \
                 which can sometimes cause surprising resolution failures, \
                 if this error is too confusing you may with to retry \
                 without the offline flag.",
            );
        }
    }

    format_err!("{}", msg)
}

// Returns if `a` and `b` are compatible in the semver sense. This is a
// commutative operation.
//
// Versions `a` and `b` are compatible if their left-most nonzero digit is the
// same.
fn compatible(a: &semver::Version, b: &semver::Version) -> bool {
    if a.major != b.major {
        return false;
    }
    if a.major != 0 {
        return true;
    }
    if a.minor != b.minor {
        return false;
    }
    if a.minor != 0 {
        return true;
    }
    a.patch == b.patch
}

struct Requirements<'a> {
    summary: &'a Summary,
    // The deps map is a mapping of package name to list of features enabled.
    // Each package should be enabled, and each package should have the
    // specified set of features enabled. The boolean indicates whether this
    // package was specifically requested (rather than just requesting features
    // *within* this package).
    deps: HashMap<&'a str, (bool, Vec<String>)>,
    // The used features set is the set of features which this local package had
    // enabled, which is later used when compiling to instruct the code what
    // features were enabled.
    used: HashSet<&'a str>,
    visited: HashSet<&'a str>,
}

impl<'r> Requirements<'r> {
    fn new<'a>(summary: &'a Summary) -> Requirements<'a> {
        Requirements {
            summary,
            deps: HashMap::new(),
            used: HashSet::new(),
            visited: HashSet::new(),
        }
    }

    fn require_crate_feature(&mut self, package: &'r str, feat: &'r str) {
        self.used.insert(package);
        self.deps
            .entry(package)
            .or_insert((false, Vec::new()))
            .1
            .push(feat.to_string());
    }

    fn seen(&mut self, feat: &'r str) -> bool {
        if self.visited.insert(feat) {
            self.used.insert(feat);
            false
        } else {
            true
        }
    }

    fn require_dependency(&mut self, pkg: &'r str) {
        if self.seen(pkg) {
            return;
        }
        self.deps.entry(pkg).or_insert((false, Vec::new())).0 = true;
    }

    fn require_feature(&mut self, feat: &'r str) -> CargoResult<()> {
        if self.seen(feat) {
            return Ok(());
        }
        for f in self.summary
            .features()
            .get(feat)
            .expect("must be a valid feature")
        {
            if f == feat {
                bail!(
                    "Cyclic feature dependency: feature `{}` depends on itself",
                    feat
                );
            }
            self.add_feature(f)?;
        }
        Ok(())
    }

    fn add_feature(&mut self, feat: &'r str) -> CargoResult<()> {
        if feat.is_empty() {
            return Ok(());
        }

        // If this feature is of the form `foo/bar`, then we just lookup package
        // `foo` and enable its feature `bar`. Otherwise this feature is of the
        // form `foo` and we need to recurse to enable the feature `foo` for our
        // own package, which may end up enabling more features or just enabling
        // a dependency.
        let mut parts = feat.splitn(2, '/');
        let feat_or_package = parts.next().unwrap();
        match parts.next() {
            Some(feat) => {
                self.require_crate_feature(feat_or_package, feat);
            }
            None => {
                if self.summary.features().contains_key(feat_or_package) {
                    self.require_feature(feat_or_package)?;
                } else {
                    self.require_dependency(feat_or_package);
                }
            }
        }
        Ok(())
    }
}

/// Takes requested features for a single package from the input Method and
/// recurses to find all requested features, dependencies and requested
/// dependency features in a Requirements object, returning it to the resolver.
fn build_requirements<'a, 'b: 'a>(
    s: &'a Summary,
    method: &'b Method,
) -> CargoResult<Requirements<'a>> {
    let mut reqs = Requirements::new(s);
    match *method {
        Method::Everything
        | Method::Required {
            all_features: true, ..
        } => {
            for key in s.features().keys() {
                reqs.require_feature(key)?;
            }
            for dep in s.dependencies().iter().filter(|d| d.is_optional()) {
                reqs.require_dependency(dep.name().to_inner());
            }
        }
        Method::Required {
            features: requested_features,
            ..
        } => for feat in requested_features.iter() {
            reqs.add_feature(feat)?;
        },
    }
    match *method {
        Method::Everything
        | Method::Required {
            uses_default_features: true,
            ..
        } => {
            if s.features().get("default").is_some() {
                reqs.require_feature("default")?;
            }
        }
        Method::Required {
            uses_default_features: false,
            ..
        } => {}
    }
    Ok(reqs)
}

impl Context {
    /// Activate this summary by inserting it into our list of known activations.
    ///
    /// Returns true if this summary with the given method is already activated.
    fn flag_activated(&mut self, summary: &Summary, method: &Method) -> CargoResult<bool> {
        let id = summary.package_id();
        let prev = self.activations
            .entry((id.name(), id.source_id().clone()))
            .or_insert_with(|| Rc::new(Vec::new()));
        if !prev.iter().any(|c| c == summary) {
            self.resolve_graph.push(GraphNode::Add(id.clone()));
            if let Some(link) = summary.links() {
                ensure!(
                    self.links.insert(link, id.clone()).is_none(),
                    "Attempting to resolve a with more then one crate with the links={}. \n\
                     This will not build as is. Consider rebuilding the .lock file.",
                    &*link
                );
            }
            let mut inner: Vec<_> = (**prev).clone();
            inner.push(summary.clone());
            *prev = Rc::new(inner);
            return Ok(false);
        }
        debug!("checking if {} is already activated", summary.package_id());
        let (features, use_default) = match *method {
            Method::Everything
            | Method::Required {
                all_features: true, ..
            } => return Ok(false),
            Method::Required {
                features,
                uses_default_features,
                ..
            } => (features, uses_default_features),
        };

        let has_default_feature = summary.features().contains_key("default");
        Ok(match self.resolve_features.get(id) {
            Some(prev) => {
                features
                    .iter()
                    .all(|f| prev.contains(&InternedString::new(f)))
                    && (!use_default || prev.contains(&InternedString::new("default"))
                        || !has_default_feature)
            }
            None => features.is_empty() && (!use_default || !has_default_feature),
        })
    }

    fn build_deps(
        &mut self,
        registry: &mut RegistryQueryer,
        parent: Option<&Summary>,
        candidate: &Summary,
        method: &Method,
    ) -> ActivateResult<Vec<DepInfo>> {
        // First, figure out our set of dependencies based on the requested set
        // of features. This also calculates what features we're going to enable
        // for our own dependencies.
        let deps = self.resolve_features(parent, candidate, method)?;

        // Next, transform all dependencies into a list of possible candidates
        // which can satisfy that dependency.
        let mut deps = deps.into_iter()
            .map(|(dep, features)| {
                let candidates = registry.query(&dep)?;
                Ok((dep, candidates, Rc::new(features)))
            })
            .collect::<CargoResult<Vec<DepInfo>>>()?;

        // Attempt to resolve dependencies with fewer candidates before trying
        // dependencies with more candidates.  This way if the dependency with
        // only one candidate can't be resolved we don't have to do a bunch of
        // work before we figure that out.
        deps.sort_by_key(|&(_, ref a, _)| a.len());

        Ok(deps)
    }

    fn prev_active(&self, dep: &Dependency) -> &[Summary] {
        self.activations
            .get(&(dep.name(), dep.source_id().clone()))
            .map(|v| &v[..])
            .unwrap_or(&[])
    }

    fn is_active(&self, id: &PackageId) -> bool {
        self.activations
            .get(&(id.name(), id.source_id().clone()))
            .map(|v| v.iter().any(|s| s.package_id() == id))
            .unwrap_or(false)
    }

    /// checks whether all of `parent` and the keys of `conflicting activations`
    /// are still active
    fn is_conflicting(
        &self,
        parent: Option<&PackageId>,
        conflicting_activations: &HashMap<PackageId, ConflictReason>,
    ) -> bool {
        conflicting_activations
            .keys()
            .chain(parent)
            .all(|id| self.is_active(id))
    }

    /// Return all dependencies and the features we want from them.
    fn resolve_features<'b>(
        &mut self,
        parent: Option<&Summary>,
        s: &'b Summary,
        method: &'b Method,
    ) -> ActivateResult<Vec<(Dependency, Vec<String>)>> {
        let dev_deps = match *method {
            Method::Everything => true,
            Method::Required { dev_deps, .. } => dev_deps,
        };

        // First, filter by dev-dependencies
        let deps = s.dependencies();
        let deps = deps.iter().filter(|d| d.is_transitive() || dev_deps);

        let mut reqs = build_requirements(s, method)?;
        let mut ret = Vec::new();

        // Next, collect all actually enabled dependencies and their features.
        for dep in deps {
            // Skip optional dependencies, but not those enabled through a feature
            if dep.is_optional() && !reqs.deps.contains_key(&*dep.name()) {
                continue;
            }
            // So we want this dependency.  Move the features we want from `feature_deps`
            // to `ret`.
            let base = reqs.deps.remove(&*dep.name()).unwrap_or((false, vec![]));
            if !dep.is_optional() && base.0 {
                self.warnings.push(format!(
                    "Package `{}` does not have feature `{}`. It has a required dependency \
                     with that name, but only optional dependencies can be used as features. \
                     This is currently a warning to ease the transition, but it will become an \
                     error in the future.",
                    s.package_id(),
                    dep.name()
                ));
            }
            let mut base = base.1;
            base.extend(dep.features().iter().cloned());
            for feature in base.iter() {
                if feature.contains('/') {
                    return Err(
                        format_err!("feature names may not contain slashes: `{}`", feature).into(),
                    );
                }
            }
            ret.push((dep.clone(), base));
        }

        // Any remaining entries in feature_deps are bugs in that the package does not actually
        // have those dependencies.  We classified them as dependencies in the first place
        // because there is no such feature, either.
        if !reqs.deps.is_empty() {
            let unknown = reqs.deps.keys().map(|s| &s[..]).collect::<Vec<&str>>();
            let features = unknown.join(", ");
            return Err(match parent {
                None => format_err!(
                    "Package `{}` does not have these features: `{}`",
                    s.package_id(),
                    features
                ).into(),
                Some(p) => (
                    p.package_id().clone(),
                    ConflictReason::MissingFeatures(features),
                ).into(),
            });
        }

        // Record what list of features is active for this package.
        if !reqs.used.is_empty() {
            let pkgid = s.package_id();

            let set = self.resolve_features
                .entry(pkgid.clone())
                .or_insert_with(HashSet::new);
            for feature in reqs.used {
                set.insert(InternedString::new(feature));
            }
        }

        Ok(ret)
    }

    fn resolve_replacements(&self) -> HashMap<PackageId, PackageId> {
        let mut replacements = HashMap::new();
        let mut cur = &self.resolve_replacements;
        while let Some(ref node) = cur.head {
            let (k, v) = node.0.clone();
            replacements.insert(k, v);
            cur = &node.1;
        }
        replacements
    }

    fn graph(&self) -> Graph<PackageId> {
        let mut graph = Graph::new();
        let mut cur = &self.resolve_graph;
        while let Some(ref node) = cur.head {
            match node.0 {
                GraphNode::Add(ref p) => graph.add(p.clone(), &[]),
                GraphNode::Link(ref a, ref b) => graph.link(a.clone(), b.clone()),
            }
            cur = &node.1;
        }
        graph
    }
}

fn check_cycles(resolve: &Resolve, activations: &Activations) -> CargoResult<()> {
    let summaries: HashMap<&PackageId, &Summary> = activations
        .values()
        .flat_map(|v| v.iter())
        .map(|s| (s.package_id(), s))
        .collect();

    // Sort packages to produce user friendly deterministic errors.
    let all_packages = resolve.iter().collect::<BinaryHeap<_>>().into_sorted_vec();
    let mut checked = HashSet::new();
    for pkg in all_packages {
        if !checked.contains(pkg) {
            visit(resolve, pkg, &summaries, &mut HashSet::new(), &mut checked)?
        }
    }
    return Ok(());

    fn visit<'a>(
        resolve: &'a Resolve,
        id: &'a PackageId,
        summaries: &HashMap<&'a PackageId, &Summary>,
        visited: &mut HashSet<&'a PackageId>,
        checked: &mut HashSet<&'a PackageId>,
    ) -> CargoResult<()> {
        // See if we visited ourselves
        if !visited.insert(id) {
            bail!(
                "cyclic package dependency: package `{}` depends on itself. Cycle:\n{}",
                id,
                describe_path(&resolve.graph, id)
            );
        }

        // If we've already checked this node no need to recurse again as we'll
        // just conclude the same thing as last time, so we only execute the
        // recursive step if we successfully insert into `checked`.
        //
        // Note that if we hit an intransitive dependency then we clear out the
        // visitation list as we can't induce a cycle through transitive
        // dependencies.
        if checked.insert(id) {
            let summary = summaries[id];
            for dep in resolve.deps_not_replaced(id) {
                let is_transitive = summary
                    .dependencies()
                    .iter()
                    .any(|d| d.matches_id(dep) && d.is_transitive());
                let mut empty = HashSet::new();
                let visited = if is_transitive {
                    &mut *visited
                } else {
                    &mut empty
                };
                visit(resolve, dep, summaries, visited, checked)?;

                if let Some(id) = resolve.replacement(dep) {
                    visit(resolve, id, summaries, visited, checked)?;
                }
            }
        }

        // Ok, we're done, no longer visiting our node any more
        visited.remove(id);
        Ok(())
    }
}
