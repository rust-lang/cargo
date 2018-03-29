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

use std::collections::{BTreeMap, BinaryHeap, HashMap, HashSet};
use std::mem;
use std::rc::Rc;
use std::time::{Duration, Instant};

use semver;

use core::{Dependency, PackageId, Registry, SourceId, Summary};
use core::PackageIdSpec;
use core::interning::InternedString;
use util::config::Config;
use util::Graph;
use util::errors::{CargoError, CargoResult};
use util::profile;

use self::types::{ActivateError, ActivateResult, Candidate, ConflictReason, DepInfo, DepsFrame};
use self::types::{GraphNode, RcList, RcVecIter, RegistryQueryer};

pub use self::encode::{EncodableDependency, EncodablePackageId, EncodableResolve};
pub use self::encode::{Metadata, WorkspaceResolve};
pub use self::resolve::Resolve;
pub use self::types::Method;

mod encode;
mod conflict_cache;
mod resolve;
mod types;

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
///
/// * `summaries` - the list of package summaries along with how to resolve
///   their features. This is a list of all top-level packages that are intended
///   to be part of the lock file (resolve output). These typically are a list
///   of all workspace members.
///
/// * `replacements` - this is a list of `[replace]` directives found in the
///   root of the workspace. The list here is a `PackageIdSpec` of what to
///   replace and a `Dependency` to replace that with. In general it's not
///   recommended to use `[replace]` any more and use `[patch]` instead, which
///   is supported elsewhere.
///
/// * `registry` - this is the source from which all package summaries are
///   loaded. It's expected that this is extensively configured ahead of time
///   and is idempotent with our requests to it (aka returns the same results
///   for the same query every time). Typically this is an instance of a
///   `PackageRegistry`.
///
/// * `try_to_use` - this is a list of package ids which were previously found
///   in the lock file. We heuristically prefer the ids listed in `try_to_use`
///   when sorting candidates to activate, but otherwise this isn't used
///   anywhere else.
///
/// * `config` - a location to print warnings and such, or `None` if no warnings
///   should be printed
///
/// * `print_warnings` - whether or not to print backwards-compatibility
///   warnings and such
pub fn resolve(
    summaries: &[(Summary, Method)],
    replacements: &[(PackageIdSpec, Dependency)],
    registry: &mut Registry,
    try_to_use: &HashSet<&PackageId>,
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
    let minimal_versions = match config {
        Some(config) => config.cli_unstable().minimal_versions,
        None => false,
    };
    let mut registry = RegistryQueryer::new(registry, replacements, try_to_use, minimal_versions);
    let cx = activate_deps_loop(cx, &mut registry, summaries, config)?;

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

/// A helper "iterator" used to extract candidates within a current `Context` of
/// a dependency graph.
///
/// This struct doesn't literally implement the `Iterator` trait (requires a few
/// more inputs) but in general acts like one. Each `RemainingCandidates` is
/// created with a list of candidates to choose from. When attempting to iterate
/// over the list of candidates only *valid* candidates are returned. Validity
/// is defined within a `Context`.
///
/// Candidates passed to `new` may not be returned from `next` as they could be
/// filtered out, and if iteration stops a map of all packages which caused
/// filtered out candidates to be filtered out will be returned.
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

    /// Attempts to find another candidate to check from this list.
    ///
    /// This method will attempt to move this iterator forward, returning a
    /// candidate that's possible to activate. The `cx` argument is the current
    /// context which determines validity for candidates returned, and the `dep`
    /// is the dependency listing that we're activating for.
    ///
    /// If successful a `(Candidate, bool)` pair will be returned. The
    /// `Candidate` is the candidate to attempt to activate, and the `bool` is
    /// an indicator of whether there are remaining candidates to try of if
    /// we've reached the end of iteration.
    ///
    /// If we've reached the end of the iterator here then `Err` will be
    /// returned. The error will contain a map of package id to conflict reason,
    /// where each package id caused a candidate to be filtered out from the
    /// original list for the reason listed.
    fn next(
        &mut self,
        cx: &Context,
        dep: &Dependency,
    ) -> Result<(Candidate, bool), HashMap<PackageId, ConflictReason>> {
        let prev_active = cx.prev_active(dep);

        for (_, b) in self.remaining.by_ref() {
            // The `links` key in the manifest dictates that there's only one
            // package in a dependency graph, globally, with that particular
            // `links` key. If this candidate links to something that's already
            // linked to by a different package then we've gotta skip this.
            if let Some(link) = b.summary.links() {
                if let Some(a) = cx.links.get(&link) {
                    if a != b.summary.package_id() {
                        self.conflicting_prev_active
                            .entry(a.clone())
                            .or_insert_with(|| ConflictReason::Links(link.to_string()));
                        continue;
                    }
                }
            }

            // Otherwise the condition for being a valid candidate relies on
            // semver. Cargo dictates that you can't duplicate multiple
            // semver-compatible versions of a crate. For example we can't
            // simultaneously activate `foo 1.0.2` and `foo 1.2.0`. We can,
            // however, activate `1.0.2` and `2.0.0`.
            //
            // Here we throw out our candidate if it's *compatible*, yet not
            // equal, to all previously activated versions.
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

            // Well if we made it this far then we've got a valid dependency. We
            // want this iterator to be inherently "peekable" so we don't
            // necessarily return the item just yet. Instead we stash it away to
            // get returned later, and if we replaced something then that was
            // actually the candidate to try first so we return that.
            if let Some(r) = mem::replace(&mut self.has_another, Some(b)) {
                return Ok((r, true));
            }
        }

        // Alright we've entirely exhausted our list of candidates. If we've got
        // something stashed away return that here (also indicating that there's
        // nothign else). If nothing is stashed away we return the list of all
        // conflicting activations, if any.
        //
        // TODO: can the `conflicting_prev_active` clone be avoided here? should
        //       panic if this is called twice and an error is already returned
        self.has_another
            .take()
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

    // `past_conflicting_activations` is a cache of the reasons for each time we
    // backtrack.
    let mut past_conflicting_activations = conflict_cache::ConflictCache::new();

    // Activate all the initial summaries to kick off some work.
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
            Err(ActivateError::Fatal(e)) => return Err(e),
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

        // Figure out what our next dependency to activate is, and if nothing is
        // listed then we're entirely done with this frame (yay!) and we can
        // move on to the next frame.
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
                .conflicting(&cx, &dep)
                .is_some();

        let mut remaining_candidates = RemainingCandidates::new(&candidates);

        // `conflicting_activations` stores all the reasons we were unable to
        // activate candidates. One of these reasons will have to go away for
        // backtracking to find a place to restart. It is also the list of
        // things to explain in the error message if we fail to resolve.
        //
        // This is a map of package id to a reason why that packaged caused a
        // conflict for us.
        let mut conflicting_activations = HashMap::new();

        // When backtracking we don't fully update `conflicting_activations`
        // especially for the cases that we didn't make a backtrack frame in the
        // first place.  This `backtracked` var stores whether we are continuing
        // from a restored backtrack frame so that we can skip caching
        // `conflicting_activations` in `past_conflicting_activations`
        let mut backtracked = false;

        loop {
            let next = remaining_candidates.next(&cx, &dep);

            let (candidate, has_another) = next.or_else(|conflicting| {
                // If we get here then our `remaining_candidates` was just
                // exhausted, so `dep` failed to activate.
                //
                // It's our job here to backtrack, if possible, and find a
                // different candidate to activate. If we can't find any
                // candidates whatsoever then it's time to bail entirely.
                trace!("{}[{}]>{} -- no candidates", parent.name(), cur, dep.name());

                // Add all the reasons to our frame's list of conflicting
                // activations, as we may use this to start backtracking later.
                conflicting_activations.extend(conflicting);

                // Use our list of `conflicting_activations` to add to our
                // global list of past conflicting activations, effectively
                // globally poisoning `dep` if `conflicting_activations` ever
                // shows up again. We'll use the `past_conflicting_activations`
                // below to determine if a dependency is poisoned and skip as
                // much work as possible.
                //
                // If we're only here for the error messages then there's no
                // need to try this as this dependency is already known to be
                // bad.
                //
                // As we mentioned above with the `backtracked` variable if this
                // local is set to `true` then our `conflicting_activations` may
                // not be right, so we can't push into our global cache.
                if !just_here_for_the_error_messages && !backtracked {
                    past_conflicting_activations.insert(&dep, &conflicting_activations);
                }

                match find_candidate(&mut backtrack_stack, &parent, &conflicting_activations) {
                    Some((candidate, has_another, frame)) => {
                        // Reset all of our local variables used with the
                        // contents of `frame` to complete our backtrack.
                        cur = frame.cur;
                        cx = frame.context_backup;
                        remaining_deps = frame.deps_backup;
                        remaining_candidates = frame.remaining_candidates;
                        parent = frame.parent;
                        dep = frame.dep;
                        features = frame.features;
                        conflicting_activations = frame.conflicting_activations;
                        backtracked = true;
                        Ok((candidate, has_another))
                    }
                    None => Err(activation_error(
                        &cx,
                        registry.registry,
                        &parent,
                        &dep,
                        &conflicting_activations,
                        &candidates,
                        config,
                    )),
                }
            })?;

            // If we're only here for the error messages then we know that this
            // activation will fail one way or another. To that end if we've got
            // more candidates we want to fast-forward to the last one as
            // otherwise we'll just backtrack here anyway (helping us to skip
            // some work).
            if just_here_for_the_error_messages && !backtracked && has_another {
                continue;
            }

            // We have a `candidate`. Create a `BacktrackFrame` so we can add it
            // to the `backtrack_stack` later if activation succeeds.
            //
            // Note that if we don't actually have another candidate then there
            // will be nothing to backtrack to so we skip construction of the
            // frame. This is a relatively important optimization as a number of
            // the `clone` calls below can be quite expensive, so we avoid them
            // if we can.
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

            let successfully_activated = match res {
                // Success! We've now activated our `candidate` in our context
                // and we're almost ready to move on. We may want to scrap this
                // frame in the end if it looks like it's not going to end well,
                // so figure that out here.
                Ok(Some((mut frame, dur))) => {
                    deps_time += dur;

                    // Our `frame` here is a new package with its own list of
                    // dependencies. Do a sanity check here of all those
                    // dependencies by cross-referencing our global
                    // `past_conflicting_activations`. Recall that map is a
                    // global cache which lists sets of packages where, when
                    // activated, the dependency is unresolvable.
                    //
                    // If any our our frame's dependencies fit in that bucket,
                    // aka known unresolvable, then we extend our own set of
                    // conflicting activations with theirs. We can do this
                    // because the set of conflicts we found implies the
                    // dependency can't be activated which implies that we
                    // ourselves can't be activated, so we know that they
                    // conflict with us.
                    let mut has_past_conflicting_dep = just_here_for_the_error_messages;
                    if !has_past_conflicting_dep {
                        if let Some(conflicting) = frame
                            .remaining_siblings
                            .clone()
                            .filter_map(|(_, (ref new_dep, _, _))| {
                                past_conflicting_activations.conflicting(&cx, &new_dep)
                            })
                            .next()
                        {
                            // If one of our deps is known unresolvable
                            // then we will not succeed.
                            // How ever if we are part of the reason that
                            // one of our deps conflicts then
                            // we can make a stronger statement
                            // because we will definitely be activated when
                            // we try our dep.
                            conflicting_activations.extend(
                                conflicting
                                    .iter()
                                    .filter(|&(p, _)| p != &pid)
                                    .map(|(p, r)| (p.clone(), r.clone())),
                            );

                            has_past_conflicting_dep = true;
                        }
                    }
                    // If any of `remaining_deps` are known unresolvable with
                    // us activated, then we extend our own set of
                    // conflicting activations with theirs and its parent. We can do this
                    // because the set of conflicts we found implies the
                    // dependency can't be activated which implies that we
                    // ourselves are incompatible with that dep, so we know that deps
                    // parent conflict with us.
                    if !has_past_conflicting_dep {
                        if let Some(known_related_bad_deps) =
                            past_conflicting_activations.dependencies_conflicting_with(&pid)
                        {
                            if let Some((other_parent, conflict)) = remaining_deps
                                .iter()
                                .flat_map(|other| other.flatten())
                                // for deps related to us
                                .filter(|&(_, ref other_dep)|
                                        known_related_bad_deps.contains(other_dep))
                                .filter_map(|(other_parent, other_dep)| {
                                    past_conflicting_activations
                                        .find_conflicting(
                                            &cx,
                                            &other_dep,
                                            |con| con.contains_key(&pid)
                                        )
                                        .map(|con| (other_parent, con))
                                })
                                .next()
                            {
                                let rel = conflict.get(&pid).unwrap().clone();

                                // The conflict we found is
                                // "other dep will not succeed if we are activated."
                                // We want to add
                                // "our dep will not succeed if other dep is in remaining_deps"
                                // but that is not how the cache is set up.
                                // So we add the less general but much faster,
                                // "our dep will not succeed if other dep's parent is activated".
                                conflicting_activations.extend(
                                    conflict
                                        .iter()
                                        .filter(|&(p, _)| p != &pid)
                                        .map(|(p, r)| (p.clone(), r.clone())),
                                );
                                conflicting_activations.insert(other_parent.clone(), rel);
                                has_past_conflicting_dep = true;
                            }
                        }
                    }

                    // Ok if we're in a "known failure" state for this frame we
                    // may want to skip it altogether though. We don't want to
                    // skip it though in the case that we're displaying error
                    // messages to the user!
                    //
                    // Here we need to figure out if the user will see if we
                    // skipped this candidate (if it's known to fail, aka has a
                    // conflicting dep and we're the last candidate). If we're
                    // here for the error messages, we can't skip it (but we can
                    // prune extra work). If we don't have any candidates in our
                    // backtrack stack then we're the last line of defense, so
                    // we'll want to present an error message for sure.
                    let activate_for_error_message = has_past_conflicting_dep && !has_another && {
                        just_here_for_the_error_messages || {
                            conflicting_activations
                                .extend(remaining_candidates.conflicting_prev_active.clone());
                            find_candidate(
                                &mut backtrack_stack.clone(),
                                &parent,
                                &conflicting_activations,
                            ).is_none()
                        }
                    };

                    // If we're only here for the error messages then we know
                    // one of our candidate deps will fail, meaning we will
                    // fail and that none of the backtrack frames will find a
                    // candidate that will help. Consequently let's clean up the
                    // no longer needed backtrack frames.
                    if activate_for_error_message {
                        backtrack_stack.clear();
                    }

                    // If we don't know for a fact that we'll fail or if we're
                    // just here for the error message then we push this frame
                    // onto our list of to-be-resolve, which will generate more
                    // work for us later on.
                    //
                    // Otherwise we're guaranteed to fail and were not here for
                    // error messages, so we skip work and don't push anything
                    // onto our stack.
                    frame.just_for_error_messages = has_past_conflicting_dep;
                    if !has_past_conflicting_dep || activate_for_error_message {
                        remaining_deps.push(frame);
                        true
                    } else {
                        trace!(
                            "{}[{}]>{} skipping {} ",
                            parent.name(),
                            cur,
                            dep.name(),
                            pid.version()
                        );
                        false
                    }
                }

                // This candidate's already activated, so there's no extra work
                // for us to do. Let's keep going.
                Ok(None) => true,

                // We failed with a super fatal error (like a network error), so
                // bail out as quickly as possible as we can't reliably
                // backtrack from errors like these
                Err(ActivateError::Fatal(e)) => return Err(e),

                // We failed due to a bland conflict, bah! Record this in our
                // frame's list of conflicting activations as to why this
                // candidate failed, and then move on.
                Err(ActivateError::Conflict(id, reason)) => {
                    conflicting_activations.insert(id, reason);
                    false
                }
            };

            // If we've successfully activated then save off the backtrack frame
            // if one was created, and otherwise break out of the inner
            // activation loop as we're ready to move to the next dependency
            if successfully_activated {
                backtrack_stack.extend(backtrack);
                break;
            }

            // We've failed to activate this dependency, oh dear! Our call to
            // `activate` above may have altered our `cx` local variable, so
            // restore it back if we've got a backtrack frame.
            //
            // If we don't have a backtrack frame then we're just using the `cx`
            // for error messages anyway so we can live with a little
            // imprecision.
            if let Some(b) = backtrack {
                cx = b.context_backup;
            }
        }

        // Ok phew, that loop was a big one! If we've broken out then we've
        // successfully activated a candidate. Our stacks are all in place that
        // we're ready to move on to the next dependency that needs activation,
        // so loop back to the top of the function here.
    }

    Ok(cx)
}

/// Looks through the states in `backtrack_stack` for dependencies with
/// remaining candidates. For each one, also checks if rolling back
/// could change the outcome of the failed resolution that caused backtracking
/// in the first place. Namely, if we've backtracked past the parent of the
/// failed dep, or any of the packages flagged as giving us trouble in
/// `conflicting_activations`.
///
/// Read <https://github.com/rust-lang/cargo/pull/4834>
/// For several more detailed explanations of the logic here.
fn find_candidate<'a>(
    backtrack_stack: &mut Vec<BacktrackFrame>,
    parent: &Summary,
    conflicting_activations: &HashMap<PackageId, ConflictReason>,
) -> Option<(Candidate, bool, BacktrackFrame)> {
    while let Some(mut frame) = backtrack_stack.pop() {
        let next = frame
            .remaining_candidates
            .next(&frame.context_backup, &frame.dep);
        let (candidate, has_another) = match next {
            Ok(pair) => pair,
            Err(_) => continue,
        };
        // When we're calling this method we know that `parent` failed to
        // activate. That means that some dependency failed to get resolved for
        // whatever reason, and all of those reasons (plus maybe some extras)
        // are listed in `conflicting_activations`.
        //
        // This means that if all members of `conflicting_activations` are still
        // active in this back up we know that we're guaranteed to not actually
        // make any progress. As a result if we hit this condition we can
        // completely skip this backtrack frame and move on to the next.
        if frame
            .context_backup
            .is_conflicting(Some(parent.package_id()), conflicting_activations)
        {
            continue;
        }

        return Some((candidate, has_another, frame));
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
