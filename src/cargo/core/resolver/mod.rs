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
//!   activated (as we're only allowed to have one).
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
use std::collections::{HashSet, HashMap, BinaryHeap, BTreeMap};
use std::iter::FromIterator;
use std::fmt;
use std::ops::Range;
use std::rc::Rc;

use semver;
use url::Url;

use core::{PackageId, Registry, SourceId, Summary, Dependency};
use core::PackageIdSpec;
use util::config::Config;
use util::Graph;
use util::errors::{CargoResult, CargoError};
use util::profile;
use util::graph::{Nodes, Edges};

pub use self::encode::{EncodableResolve, EncodableDependency, EncodablePackageId};
pub use self::encode::{Metadata, WorkspaceResolve};

mod encode;

/// Represents a fully resolved package dependency graph. Each node in the graph
/// is a package and edges represent dependencies between packages.
///
/// Each instance of `Resolve` also understands the full set of features used
/// for each package.
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
    Everything,
    Required {
        dev_deps: bool,
        features: &'a [String],
        uses_default_features: bool,
    },
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
    pub fn register_used_patches(&mut self,
                                 patches: &HashMap<Url, Vec<Summary>>) {
        for summary in patches.values().flat_map(|v| v) {
            if self.iter().any(|id| id == summary.package_id()) {
                continue
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
                    continue
                }

                // If the previous checksum wasn't calculated, the current
                // checksum is `Some`. This may indicate that a source was
                // erroneously replaced or was replaced with something that
                // desires stronger checksum guarantees than can be afforded
                // elsewhere.
                if cksum.is_none() {
                    bail!("\
checksum for `{}` was not previously calculated, but a checksum could now \
be calculated

this could be indicative of a few possible situations:

    * the source `{}` did not previously support checksums,
      but was replaced with one that does
    * newer Cargo implementations know how to checksum this source, but this
      older implementation does not
    * the lock file is corrupt
", id, id.source_id())

                // If our checksum hasn't been calculated, then it could mean
                // that future Cargo figured out how to checksum something or
                // more realistically we were overridden with a source that does
                // not have checksums.
                } else if mine.is_none() {
                    bail!("\
checksum for `{}` could not be calculated, but a checksum is listed in \
the existing lock file

this could be indicative of a few possible situations:

    * the source `{}` supports checksums,
      but was replaced with one that doesn't
    * the lock file is corrupt

unable to verify that `{0}` is the same as when the lockfile was generated
", id, id.source_id())

                // If the checksums aren't equal, and neither is None, then they
                // must both be Some, in which case the checksum now differs.
                // That's quite bad!
                } else {
                    bail!("\
checksum for `{}` changed between lock files

this could be indicative of a few possible errors:

    * the lock file is corrupt
    * a replacement source in use (e.g. a mirror) returned a different checksum
    * the source itself may be corrupt in one way or another

unable to verify that `{0}` is the same as when the lockfile was generated
", id);
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
        Deps { edges: self.graph.edges(pkg), resolve: self }
    }

    pub fn deps_not_replaced(&self, pkg: &PackageId) -> DepsNotReplaced {
        DepsNotReplaced { edges: self.graph.edges(pkg) }
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
        self.edges.as_mut()
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
    head: Option<Rc<(T, RcList<T>)>>
}

impl<T> RcList<T> {
    fn new() -> RcList<T> {
        RcList { head: None }
    }

    fn push(&mut self, data: T) {
        let node = Rc::new((data, RcList { head: self.head.take() }));
        self.head = Some(node);
    }
}

// Not derived to avoid `T: Clone`
impl<T> Clone for RcList<T> {
    fn clone(&self) -> RcList<T> {
        RcList { head: self.head.clone() }
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
struct Context<'a> {
    // TODO: Both this and the map below are super expensive to clone. We should
    //       switch to persistent hash maps if we can at some point or otherwise
    //       make these much cheaper to clone in general.
    activations: Activations,
    resolve_features: HashMap<PackageId, HashSet<String>>,

    // These are two cheaply-cloneable lists (O(1) clone) which are effectively
    // hash maps but are built up as "construction lists". We'll iterate these
    // at the very end and actually construct the map that we're making.
    resolve_graph: RcList<GraphNode>,
    resolve_replacements: RcList<(PackageId, PackageId)>,

    replacements: &'a [(PackageIdSpec, Dependency)],

    // These warnings are printed after resolution.
    warnings: RcList<String>,
}

type Activations = HashMap<String, HashMap<SourceId, Vec<Summary>>>;

/// Builds the list of all packages required to build the first argument.
pub fn resolve(summaries: &[(Summary, Method)],
               replacements: &[(PackageIdSpec, Dependency)],
               registry: &mut Registry,
               config: Option<&Config>) -> CargoResult<Resolve> {
    let cx = Context {
        resolve_graph: RcList::new(),
        resolve_features: HashMap::new(),
        resolve_replacements: RcList::new(),
        activations: HashMap::new(),
        replacements: replacements,
        warnings: RcList::new(),
    };
    let _p = profile::start(format!("resolving"));
    let cx = activate_deps_loop(cx, registry, summaries)?;

    let mut resolve = Resolve {
        graph: cx.graph(),
        empty_features: HashSet::new(),
        checksums: HashMap::new(),
        metadata: BTreeMap::new(),
        replacements: cx.resolve_replacements(),
        features: cx.resolve_features.iter().map(|(k, v)| {
            (k.clone(), v.clone())
        }).collect(),
        unused_patches: Vec::new(),
    };

    for summary in cx.activations.values()
                                 .flat_map(|v| v.values())
                                 .flat_map(|v| v.iter()) {
        let cksum = summary.checksum().map(|s| s.to_string());
        resolve.checksums.insert(summary.package_id().clone(), cksum);
    }

    check_cycles(&resolve, &cx.activations)?;
    trace!("resolved: {:?}", resolve);

    // If we have a shell, emit warnings about required deps used as feature.
    if let Some(config) = config {
        let mut shell = config.shell();
        let mut warnings = &cx.warnings;
        while let Some(ref head) = warnings.head {
            shell.warn(&head.0)?;
            warnings = &head.1;
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
fn activate(cx: &mut Context,
            registry: &mut Registry,
            parent: Option<&Summary>,
            candidate: Candidate,
            method: &Method)
            -> CargoResult<Option<DepsFrame>> {
    if let Some(parent) = parent {
        cx.resolve_graph.push(GraphNode::Link(parent.package_id().clone(),
                                           candidate.summary.package_id().clone()));
    }

    let activated = cx.flag_activated(&candidate.summary, method);

    let candidate = match candidate.replace {
        Some(replace) => {
            cx.resolve_replacements.push((candidate.summary.package_id().clone(),
                                          replace.package_id().clone()));
            if cx.flag_activated(&replace, method) && activated {
                return Ok(None);
            }
            trace!("activating {} (replacing {})", replace.package_id(),
                   candidate.summary.package_id());
            replace
        }
        None => {
            if activated {
                return Ok(None)
            }
            trace!("activating {}", candidate.summary.package_id());
            candidate.summary
        }
    };

    let deps = cx.build_deps(registry, &candidate, method)?;

    Ok(Some(DepsFrame {
        parent: candidate,
        remaining_siblings: RcVecIter::new(Rc::new(deps)),
    }))
}

struct RcVecIter<T> {
    vec: Rc<Vec<T>>,
    rest: Range<usize>,
}

impl<T> RcVecIter<T> {
    fn new(vec: Rc<Vec<T>>) -> RcVecIter<T> {
        RcVecIter {
            rest: 0..vec.len(),
            vec: vec,
        }
    }

    fn cur_index(&self) -> usize {
        self.rest.start - 1
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

impl<T> Iterator for RcVecIter<T> where T: Clone {
    type Item = (usize, T);

    fn next(&mut self) -> Option<(usize, T)> {
        self.rest.next().and_then(|i| {
            self.vec.get(i).map(|val| (i, val.clone()))
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.rest.size_hint()
    }
}

#[derive(Clone)]
struct DepsFrame {
    parent: Summary,
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
        self.remaining_siblings.clone().next().map(|(_, (_, candidates, _))| {
            candidates.len()
        }).unwrap_or(0)
    }
}

impl PartialEq for DepsFrame {
    fn eq(&self, other: &DepsFrame) -> bool {
        self.min_candidates() == other.min_candidates()
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
        // the frame with the sibling that has the least number of candidates
        // needs to get the bubbled up to the top of the heap we use below, so
        // reverse the order of the comparison here.
        other.min_candidates().cmp(&self.min_candidates())
    }
}

struct BacktrackFrame<'a> {
    context_backup: Context<'a>,
    deps_backup: BinaryHeap<DepsFrame>,
    remaining_candidates: RemainingCandidates,
    parent: Summary,
    dep: Dependency,
    features: Rc<Vec<String>>,
}

#[derive(Clone)]
struct RemainingCandidates {
    remaining: RcVecIter<Candidate>,
}

impl RemainingCandidates {
    fn next(&mut self, prev_active: &[Summary]) -> Option<Candidate> {
        // Filter the set of candidates based on the previously activated
        // versions for this dependency. We can actually use a version if it
        // precisely matches an activated version or if it is otherwise
        // incompatible with all other activated versions. Note that we
        // define "compatible" here in terms of the semver sense where if
        // the left-most nonzero digit is the same they're considered
        // compatible.
        self.remaining.by_ref().map(|p| p.1).filter(|b| {
            prev_active.iter().any(|a| *a == b.summary) ||
                prev_active.iter().all(|a| {
                    !compatible(a.version(), b.summary.version())
                })
        }).next()
    }
}

/// Recursively activates the dependencies for `top`, in depth-first order,
/// backtracking across possible candidates for each dependency as necessary.
///
/// If all dependencies can be activated and resolved to a version in the
/// dependency graph, cx.resolve is returned.
fn activate_deps_loop<'a>(mut cx: Context<'a>,
                          registry: &mut Registry,
                          summaries: &[(Summary, Method)])
                          -> CargoResult<Context<'a>> {
    // Note that a `BinaryHeap` is used for the remaining dependencies that need
    // activation. This heap is sorted such that the "largest value" is the most
    // constrained dependency, or the one with the least candidates.
    //
    // This helps us get through super constrained portions of the dependency
    // graph quickly and hopefully lock down what later larger dependencies can
    // use (those with more candidates).
    let mut backtrack_stack = Vec::new();
    let mut remaining_deps = BinaryHeap::new();
    for &(ref summary, ref method) in summaries {
        debug!("initial activation: {}", summary.package_id());
        let candidate = Candidate { summary: summary.clone(), replace: None };
        remaining_deps.extend(activate(&mut cx, registry, None, candidate,
                                       method)?);
    }

    // Main resolution loop, this is the workhorse of the resolution algorithm.
    //
    // You'll note that a few stacks are maintained on the side, which might
    // seem odd when this algorithm looks like it could be implemented
    // recursively. While correct, this is implemented iteratively to avoid
    // blowing the stack (the recusion depth is proportional to the size of the
    // input).
    //
    // The general sketch of this loop is to run until there are no dependencies
    // left to activate, and for each dependency to attempt to activate all of
    // its own dependencies in turn. The `backtrack_stack` is a side table of
    // backtracking states where if we hit an error we can return to in order to
    // attempt to continue resolving.
    while let Some(mut deps_frame) = remaining_deps.pop() {
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

        let (next, has_another, remaining_candidates) = {
            let prev_active = cx.prev_active(&dep);
            trace!("{}[{}]>{} {} candidates", parent.name(), cur, dep.name(),
                   candidates.len());
            trace!("{}[{}]>{} {} prev activations", parent.name(), cur,
                   dep.name(), prev_active.len());
            let mut candidates = RemainingCandidates {
                remaining: RcVecIter::new(Rc::clone(&candidates)),
            };
            (candidates.next(prev_active),
             candidates.clone().next(prev_active).is_some(),
             candidates)
        };

        // Alright, for each candidate that's gotten this far, it meets the
        // following requirements:
        //
        // 1. The version matches the dependency requirement listed for this
        //    package
        // 2. There are no activated versions for this package which are
        //    semver-compatible, or there's an activated version which is
        //    precisely equal to `candidate`.
        //
        // This means that we're going to attempt to activate each candidate in
        // turn. We could possibly fail to activate each candidate, so we try
        // each one in turn.
        let candidate = match next {
            Some(candidate) => {
                // We have a candidate. Add an entry to the `backtrack_stack` so
                // we can try the next one if this one fails.
                if has_another {
                    backtrack_stack.push(BacktrackFrame {
                        context_backup: Context::clone(&cx),
                        deps_backup: <BinaryHeap<DepsFrame>>::clone(&remaining_deps),
                        remaining_candidates: remaining_candidates,
                        parent: Summary::clone(&parent),
                        dep: Dependency::clone(&dep),
                        features: Rc::clone(&features),
                    });
                }
                candidate
            }
            None => {
                // This dependency has no valid candidate. Backtrack until we
                // find a dependency that does have a candidate to try, and try
                // to activate that one.  This resets the `remaining_deps` to
                // their state at the found level of the `backtrack_stack`.
                trace!("{}[{}]>{} -- no candidates", parent.name(), cur,
                       dep.name());
                match find_candidate(&mut backtrack_stack,
                                     &mut cx,
                                     &mut remaining_deps,
                                     &mut parent,
                                     &mut cur,
                                     &mut dep,
                                     &mut features) {
                    None => return Err(activation_error(&cx, registry, &parent,
                                                        &dep,
                                                        &cx.prev_active(&dep),
                                                        &candidates)),
                    Some(candidate) => candidate,
                }
            }
        };

        let method = Method::Required {
            dev_deps: false,
            features: &features,
            uses_default_features: dep.uses_default_features(),
        };
        trace!("{}[{}]>{} trying {}", parent.name(), cur, dep.name(),
               candidate.summary.version());
        remaining_deps.extend(activate(&mut cx, registry, Some(&parent),
                              candidate, &method)?);
    }

    Ok(cx)
}

// Searches up `backtrack_stack` until it finds a dependency with remaining
// candidates. Resets `cx` and `remaining_deps` to that level and returns the
// next candidate. If all candidates have been exhausted, returns None.
fn find_candidate<'a>(backtrack_stack: &mut Vec<BacktrackFrame<'a>>,
                      cx: &mut Context<'a>,
                      remaining_deps: &mut BinaryHeap<DepsFrame>,
                      parent: &mut Summary,
                      cur: &mut usize,
                      dep: &mut Dependency,
                      features: &mut Rc<Vec<String>>) -> Option<Candidate> {
    while let Some(mut frame) = backtrack_stack.pop() {
        let (next, has_another) = {
            let prev_active = frame.context_backup.prev_active(&frame.dep);
            (frame.remaining_candidates.next(prev_active),
             frame.remaining_candidates.clone().next(prev_active).is_some())
        };
        if let Some(candidate) = next {
            if has_another {
                *cx = frame.context_backup.clone();
                *remaining_deps = frame.deps_backup.clone();
                *parent = frame.parent.clone();
                *dep = frame.dep.clone();
                *features = frame.features.clone();
                backtrack_stack.push(frame);
            } else {
                *cx = frame.context_backup;
                *remaining_deps = frame.deps_backup;
                *parent = frame.parent;
                *dep = frame.dep;
                *features = frame.features;
            }
            *cur = remaining_deps.peek().unwrap().remaining_siblings.cur_index();
            return Some(candidate)
        }
    }
    None
}

fn activation_error(cx: &Context,
                    registry: &mut Registry,
                    parent: &Summary,
                    dep: &Dependency,
                    prev_active: &[Summary],
                    candidates: &[Candidate]) -> CargoError {
    if candidates.len() > 0 {
        let mut msg = format!("failed to select a version for `{}` \
                               (required by `{}`):\n\
                               all possible versions conflict with \
                               previously selected versions of `{}`",
                              dep.name(), parent.name(),
                              dep.name());
        let graph = cx.graph();
        'outer: for v in prev_active.iter() {
            for node in graph.iter() {
                let edges = match graph.edges(node) {
                    Some(edges) => edges,
                    None => continue,
                };
                for edge in edges {
                    if edge != v.package_id() { continue }

                    msg.push_str(&format!("\n  version {} in use by {}",
                                          v.version(), edge));
                    continue 'outer;
                }
            }
            msg.push_str(&format!("\n  version {} in use by ??",
                                  v.version()));
        }

        msg.push_str(&format!("\n  possible versions to select: {}",
                              candidates.iter()
                                        .map(|v| v.summary.version())
                                        .map(|v| v.to_string())
                                        .collect::<Vec<_>>()
                                        .join(", ")));

        return msg.into()
    }

    // Once we're all the way down here, we're definitely lost in the
    // weeds! We didn't actually use any candidates above, so we need to
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
    candidates.sort_by(|a, b| {
        b.version().cmp(a.version())
    });

    let msg = if !candidates.is_empty() {
        let versions = {
            let mut versions = candidates.iter().take(3).map(|cand| {
                cand.version().to_string()
            }).collect::<Vec<_>>();

            if candidates.len() > 3 {
                versions.push("...".into());
            }

            versions.join(", ")
        };

        let mut msg = format!("no matching version `{}` found for package `{}` \
                               (required by `{}`)\n\
                               location searched: {}\n\
                               versions found: {}",
                              dep.version_req(),
                              dep.name(),
                              parent.name(),
                              dep.source_id(),
                              versions);

        // If we have a path dependency with a locked version, then this may
        // indicate that we updated a sub-package and forgot to run `cargo
        // update`. In this case try to print a helpful error!
        if dep.source_id().is_path()
           && dep.version_req().to_string().starts_with("=") {
            msg.push_str("\nconsider running `cargo update` to update \
                          a path dependency's locked version");
        }

        msg
    } else {
        format!("no matching package named `{}` found \
                 (required by `{}`)\n\
                 location searched: {}\n\
                 version required: {}",
                dep.name(), parent.name(),
                dep.source_id(),
                dep.version_req())
    };

    msg.into()
}

// Returns if `a` and `b` are compatible in the semver sense. This is a
// commutative operation.
//
// Versions `a` and `b` are compatible if their left-most nonzero digit is the
// same.
fn compatible(a: &semver::Version, b: &semver::Version) -> bool {
    if a.major != b.major { return false }
    if a.major != 0 { return true }
    if a.minor != b.minor { return false }
    if a.minor != 0 { return true }
    a.patch == b.patch
}

// Returns a pair of (feature dependencies, all used features)
//
// The feature dependencies map is a mapping of package name to list of features
// enabled. Each package should be enabled, and each package should have the
// specified set of features enabled.  The boolean indicates whether this
// package was specifically requested (rather than just requesting features
// *within* this package).
//
// The all used features set is the set of features which this local package had
// enabled, which is later used when compiling to instruct the code what
// features were enabled.
fn build_features<'a>(s: &'a Summary, method: &'a Method)
                      -> CargoResult<(HashMap<&'a str, (bool, Vec<String>)>, HashSet<&'a str>)> {
    let mut deps = HashMap::new();
    let mut used = HashSet::new();
    let mut visited = HashSet::new();
    match *method {
        Method::Everything => {
            for key in s.features().keys() {
                add_feature(s, key, &mut deps, &mut used, &mut visited)?;
            }
            for dep in s.dependencies().iter().filter(|d| d.is_optional()) {
                add_feature(s, dep.name(), &mut deps, &mut used,
                            &mut visited)?;
            }
        }
        Method::Required { features: requested_features, .. } =>  {
            for feat in requested_features.iter() {
                add_feature(s, feat, &mut deps, &mut used, &mut visited)?;
            }
        }
    }
    match *method {
        Method::Everything |
        Method::Required { uses_default_features: true, .. } => {
            if s.features().get("default").is_some() {
                add_feature(s, "default", &mut deps, &mut used,
                            &mut visited)?;
            }
        }
        Method::Required { uses_default_features: false, .. } => {}
    }
    return Ok((deps, used));

    fn add_feature<'a>(s: &'a Summary,
                       feat: &'a str,
                       deps: &mut HashMap<&'a str, (bool, Vec<String>)>,
                       used: &mut HashSet<&'a str>,
                       visited: &mut HashSet<&'a str>) -> CargoResult<()> {
        if feat.is_empty() { return Ok(()) }

        // If this feature is of the form `foo/bar`, then we just lookup package
        // `foo` and enable its feature `bar`. Otherwise this feature is of the
        // form `foo` and we need to recurse to enable the feature `foo` for our
        // own package, which may end up enabling more features or just enabling
        // a dependency.
        let mut parts = feat.splitn(2, '/');
        let feat_or_package = parts.next().unwrap();
        match parts.next() {
            Some(feat) => {
                let package = feat_or_package;
                used.insert(package);
                deps.entry(package)
                    .or_insert((false, Vec::new()))
                    .1.push(feat.to_string());
            }
            None => {
                let feat = feat_or_package;

                //if this feature has already been added, then just return Ok
                if !visited.insert(feat) {
                    return Ok(());
                }

                used.insert(feat);
                match s.features().get(feat) {
                    Some(recursive) => {
                        // This is a feature, add it recursively.
                        for f in recursive {
                            if f == feat {
                                bail!("Cyclic feature dependency: feature `{}` depends \
                                        on itself", feat);
                            }

                            add_feature(s, f, deps, used, visited)?;
                        }
                    }
                    None => {
                        // This is a dependency, mark it as explicitly requested.
                        deps.entry(feat).or_insert((false, Vec::new())).0 = true;
                    }
                }
            }
        }
        Ok(())
    }
}

impl<'a> Context<'a> {
    // Activate this summary by inserting it into our list of known activations.
    //
    // Returns if this summary with the given method is already activated.
    fn flag_activated(&mut self,
                      summary: &Summary,
                      method: &Method) -> bool {
        let id = summary.package_id();
        let prev = self.activations
                       .entry(id.name().to_string())
                       .or_insert_with(HashMap::new)
                       .entry(id.source_id().clone())
                       .or_insert(Vec::new());
        if !prev.iter().any(|c| c == summary) {
            self.resolve_graph.push(GraphNode::Add(id.clone()));
            prev.push(summary.clone());
            return false
        }
        debug!("checking if {} is already activated", summary.package_id());
        let (features, use_default) = match *method {
            Method::Required { features, uses_default_features, .. } => {
                (features, uses_default_features)
            }
            Method::Everything => return false,
        };

        let has_default_feature = summary.features().contains_key("default");
        match self.resolve_features.get(id) {
            Some(prev) => {
                features.iter().all(|f| prev.contains(f)) &&
                    (!use_default || prev.contains("default") ||
                     !has_default_feature)
            }
            None => features.is_empty() && (!use_default || !has_default_feature)
        }
    }

    fn build_deps(&mut self,
                  registry: &mut Registry,
                  candidate: &Summary,
                  method: &Method) -> CargoResult<Vec<DepInfo>> {
        // First, figure out our set of dependencies based on the requsted set
        // of features. This also calculates what features we're going to enable
        // for our own dependencies.
        let deps = self.resolve_features(candidate, method)?;

        // Next, transform all dependencies into a list of possible candidates
        // which can satisfy that dependency.
        let mut deps = deps.into_iter().map(|(dep, features)| {
            let mut candidates = self.query(registry, &dep)?;
            // When we attempt versions for a package, we'll want to start at
            // the maximum version and work our way down.
            candidates.sort_by(|a, b| {
                b.summary.version().cmp(a.summary.version())
            });
            Ok((dep, Rc::new(candidates), Rc::new(features)))
        }).collect::<CargoResult<Vec<DepInfo>>>()?;

        // Attempt to resolve dependencies with fewer candidates before trying
        // dependencies with more candidates.  This way if the dependency with
        // only one candidate can't be resolved we don't have to do a bunch of
        // work before we figure that out.
        deps.sort_by_key(|&(_, ref a, _)| a.len());

        Ok(deps)
    }

    /// Queries the `registry` to return a list of candidates for `dep`.
    ///
    /// This method is the location where overrides are taken into account. If
    /// any candidates are returned which match an override then the override is
    /// applied by performing a second query for what the override should
    /// return.
    fn query(&self,
             registry: &mut Registry,
             dep: &Dependency) -> CargoResult<Vec<Candidate>> {
        let mut ret = Vec::new();
        registry.query(dep, &mut |s| {
            ret.push(Candidate { summary: s, replace: None });
        })?;
        for candidate in ret.iter_mut() {
            let summary = &candidate.summary;

            let mut potential_matches = self.replacements.iter()
                .filter(|&&(ref spec, _)| spec.matches(summary.package_id()));

            let &(ref spec, ref dep) = match potential_matches.next() {
                None => continue,
                Some(replacement) => replacement,
            };
            debug!("found an override for {} {}", dep.name(), dep.version_req());

            let mut summaries = registry.query_vec(dep)?.into_iter();
            let s = summaries.next().ok_or_else(|| {
                format!("no matching package for override `{}` found\n\
                         location searched: {}\n\
                         version required: {}",
                         spec, dep.source_id(), dep.version_req())
            })?;
            let summaries = summaries.collect::<Vec<_>>();
            if summaries.len() > 0 {
                let bullets = summaries.iter().map(|s| {
                    format!("  * {}", s.package_id())
                }).collect::<Vec<_>>();
                bail!("the replacement specification `{}` matched \
                       multiple packages:\n  * {}\n{}", spec, s.package_id(),
                      bullets.join("\n"));
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
                bail!("overlapping replacement specifications found:\n\n  \
                       * {}\n  * {}\n\nboth specifications match: {}",
                      matched_spec, spec, summary.package_id());
            }

            for dep in summary.dependencies() {
                debug!("\t{} => {}", dep.name(), dep.version_req());
            }

            candidate.replace = replace;
        }
        Ok(ret)
    }

    fn prev_active(&self, dep: &Dependency) -> &[Summary] {
        self.activations.get(dep.name())
            .and_then(|v| v.get(dep.source_id()))
            .map(|v| &v[..])
            .unwrap_or(&[])
    }

    /// Return all dependencies and the features we want from them.
    fn resolve_features<'b>(&mut self,
                            s: &'b Summary,
                            method: &'b Method)
                            -> CargoResult<Vec<(Dependency, Vec<String>)>> {
        let dev_deps = match *method {
            Method::Everything => true,
            Method::Required { dev_deps, .. } => dev_deps,
        };

        // First, filter by dev-dependencies
        let deps = s.dependencies();
        let deps = deps.iter().filter(|d| d.is_transitive() || dev_deps);

        let (mut feature_deps, used_features) = build_features(s, method)?;
        let mut ret = Vec::new();

        // Next, collect all actually enabled dependencies and their features.
        for dep in deps {
            // Skip optional dependencies, but not those enabled through a feature
            if dep.is_optional() && !feature_deps.contains_key(dep.name()) {
                continue
            }
            // So we want this dependency.  Move the features we want from `feature_deps`
            // to `ret`.
            let base = feature_deps.remove(dep.name()).unwrap_or((false, vec![]));
            if !dep.is_optional() && base.0 {
                self.warnings.push(
                    format!("Package `{}` does not have feature `{}`. It has a required dependency \
                       with that name, but only optional dependencies can be used as features. \
                       This is currently a warning to ease the transition, but it will become an \
                       error in the future.",
                       s.package_id(), dep.name())
                );
            }
            let mut base = base.1;
            base.extend(dep.features().iter().cloned());
            for feature in base.iter() {
                if feature.contains("/") {
                    bail!("feature names may not contain slashes: `{}`", feature);
                }
            }
            ret.push((dep.clone(), base));
        }

        // Any remaining entries in feature_deps are bugs in that the package does not actually
        // have those dependencies.  We classified them as dependencies in the first place
        // because there is no such feature, either.
        if !feature_deps.is_empty() {
            let unknown = feature_deps.keys().map(|s| &s[..])
                                      .collect::<Vec<&str>>();
            let features = unknown.join(", ");
            bail!("Package `{}` does not have these features: `{}`",
                    s.package_id(), features)
        }

        // Record what list of features is active for this package.
        if !used_features.is_empty() {
            let pkgid = s.package_id();

            let set = self.resolve_features.entry(pkgid.clone())
                              .or_insert_with(HashSet::new);
            for feature in used_features {
                if !set.contains(feature) {
                    set.insert(feature.to_string());
                }
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
        return replacements
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
        return graph
    }
}

fn check_cycles(resolve: &Resolve, activations: &Activations)
                -> CargoResult<()> {
    let summaries: HashMap<&PackageId, &Summary> = activations.values()
        .flat_map(|v| v.values())
        .flat_map(|v| v)
        .map(|s| (s.package_id(), s))
        .collect();

    // Sort packages to produce user friendly deterministic errors.
    let all_packages = resolve.iter().collect::<BinaryHeap<_>>().into_sorted_vec();
    let mut checked = HashSet::new();
    for pkg in all_packages {
        if !checked.contains(pkg) {
            visit(resolve,
                  pkg,
                  &summaries,
                  &mut HashSet::new(),
                  &mut checked)?
        }
    }
    return Ok(());

    fn visit<'a>(resolve: &'a Resolve,
                 id: &'a PackageId,
                 summaries: &HashMap<&'a PackageId, &Summary>,
                 visited: &mut HashSet<&'a PackageId>,
                 checked: &mut HashSet<&'a PackageId>)
                 -> CargoResult<()> {
        // See if we visited ourselves
        if !visited.insert(id) {
            bail!("cyclic package dependency: package `{}` depends on itself",
                  id);
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
                let is_transitive = summary.dependencies().iter().any(|d| {
                    d.matches_id(dep) && d.is_transitive()
                });
                let mut empty = HashSet::new();
                let visited = if is_transitive {&mut *visited} else {&mut empty};
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
