use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::ops::Range;
use std::rc::Rc;
use std::time::{Duration, Instant};

use log::debug;

use crate::core::interning::InternedString;
use crate::core::{Dependency, PackageId, PackageIdSpec, Registry, Summary};
use crate::util::errors::CargoResult;
use crate::util::Config;

use im_rc;

pub struct ResolverProgress {
    ticks: u16,
    start: Instant,
    time_to_print: Duration,
    printed: bool,
    deps_time: Duration,
    #[cfg(debug_assertions)]
    slow_cpu_multiplier: u64,
}

impl ResolverProgress {
    pub fn new() -> ResolverProgress {
        ResolverProgress {
            ticks: 0,
            start: Instant::now(),
            time_to_print: Duration::from_millis(500),
            printed: false,
            deps_time: Duration::new(0, 0),
            // Some CI setups are much slower then the equipment used by Cargo itself.
            // Architectures that do not have a modern processor, hardware emulation, ect.
            // In the test code we have `slow_cpu_multiplier`, but that is not accessible here.
            #[cfg(debug_assertions)]
            slow_cpu_multiplier: std::env::var("CARGO_TEST_SLOW_CPU_MULTIPLIER")
                .ok()
                .and_then(|m| m.parse().ok())
                .unwrap_or(1),
        }
    }
    pub fn shell_status(&mut self, config: Option<&Config>) -> CargoResult<()> {
        // If we spend a lot of time here (we shouldn't in most cases) then give
        // a bit of a visual indicator as to what we're doing. Only enable this
        // when stderr is a tty (a human is likely to be watching) to ensure we
        // get deterministic output otherwise when observed by tools.
        //
        // Also note that we hit this loop a lot, so it's fairly performance
        // sensitive. As a result try to defer a possibly expensive operation
        // like `Instant::now` by only checking every N iterations of this loop
        // to amortize the cost of the current time lookup.
        self.ticks += 1;
        if let Some(config) = config {
            if config.shell().is_err_tty()
                && !self.printed
                && self.ticks % 1000 == 0
                && self.start.elapsed() - self.deps_time > self.time_to_print
            {
                self.printed = true;
                config.shell().status("Resolving", "dependency graph...")?;
            }
        }
        #[cfg(debug_assertions)]
        {
            // The largest test in our suite takes less then 5000 ticks
            // with all the algorithm improvements.
            // If any of them are removed then it takes more than I am willing to measure.
            // So lets fail the test fast if we have ben running for two long.
            assert!(
                self.ticks < 50_000,
                "got to 50_000 ticks in {:?}",
                self.start.elapsed()
            );
            // The largest test in our suite takes less then 30 sec
            // with all the improvements to how fast a tick can go.
            // If any of them are removed then it takes more than I am willing to measure.
            // So lets fail the test fast if we have ben running for two long.
            if self.ticks % 1000 == 0 {
                assert!(
                    self.start.elapsed() - self.deps_time
                        < Duration::from_secs(self.slow_cpu_multiplier * 90)
                );
            }
        }
        Ok(())
    }
    pub fn elapsed(&mut self, dur: Duration) {
        self.deps_time += dur;
    }
}

pub struct RegistryQueryer<'a> {
    pub registry: &'a mut (dyn Registry + 'a),
    replacements: &'a [(PackageIdSpec, Dependency)],
    try_to_use: &'a HashSet<PackageId>,
    cache: HashMap<Dependency, Rc<Vec<Candidate>>>,
    // If set the list of dependency candidates will be sorted by minimal
    // versions first. That allows `cargo update -Z minimal-versions` which will
    // specify minimum dependency versions to be used.
    minimal_versions: bool,
}

impl<'a> RegistryQueryer<'a> {
    pub fn new(
        registry: &'a mut dyn Registry,
        replacements: &'a [(PackageIdSpec, Dependency)],
        try_to_use: &'a HashSet<PackageId>,
        minimal_versions: bool,
    ) -> Self {
        RegistryQueryer {
            registry,
            replacements,
            cache: HashMap::new(),
            try_to_use,
            minimal_versions,
        }
    }

    /// Queries the `registry` to return a list of candidates for `dep`.
    ///
    /// This method is the location where overrides are taken into account. If
    /// any candidates are returned which match an override then the override is
    /// applied by performing a second query for what the override should
    /// return.
    pub fn query(&mut self, dep: &Dependency) -> CargoResult<Rc<Vec<Candidate>>> {
        if let Some(out) = self.cache.get(dep).cloned() {
            return Ok(out);
        }

        let mut ret = Vec::new();
        self.registry.query(
            dep,
            &mut |s| {
                ret.push(Candidate {
                    summary: s,
                    replace: None,
                });
            },
            false,
        )?;
        for candidate in ret.iter_mut() {
            let summary = &candidate.summary;

            let mut potential_matches = self
                .replacements
                .iter()
                .filter(|&&(ref spec, _)| spec.matches(summary.package_id()));

            let &(ref spec, ref dep) = match potential_matches.next() {
                None => continue,
                Some(replacement) => replacement,
            };
            debug!(
                "found an override for {} {}",
                dep.package_name(),
                dep.version_req()
            );

            let mut summaries = self.registry.query_vec(dep, false)?.into_iter();
            let s = summaries.next().ok_or_else(|| {
                failure::format_err!(
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
                failure::bail!(
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
                failure::bail!(
                    "overlapping replacement specifications found:\n\n  \
                     * {}\n  * {}\n\nboth specifications match: {}",
                    matched_spec,
                    spec,
                    summary.package_id()
                );
            }

            for dep in summary.dependencies() {
                debug!("\t{} => {}", dep.package_name(), dep.version_req());
            }

            candidate.replace = replace;
        }

        // When we attempt versions for a package we'll want to do so in a
        // sorted fashion to pick the "best candidates" first. Currently we try
        // prioritized summaries (those in `try_to_use`) and failing that we
        // list everything from the maximum version to the lowest version.
        ret.sort_unstable_by(|a, b| {
            let a_in_previous = self.try_to_use.contains(&a.summary.package_id());
            let b_in_previous = self.try_to_use.contains(&b.summary.package_id());
            let previous_cmp = a_in_previous.cmp(&b_in_previous).reverse();
            match previous_cmp {
                Ordering::Equal => {
                    let cmp = a.summary.version().cmp(b.summary.version());
                    if self.minimal_versions {
                        // Lower version ordered first.
                        cmp
                    } else {
                        // Higher version ordered first.
                        cmp.reverse()
                    }
                }
                _ => previous_cmp,
            }
        });

        let out = Rc::new(ret);

        self.cache.insert(dep.clone(), out.clone());

        Ok(out)
    }
}

#[derive(Clone, Copy)]
pub enum Method<'a> {
    Everything, // equivalent to Required { dev_deps: true, all_features: true, .. }
    Required {
        dev_deps: bool,
        features: &'a [InternedString],
        all_features: bool,
        uses_default_features: bool,
    },
}

impl<'r> Method<'r> {
    pub fn split_features(features: &[String]) -> Vec<InternedString> {
        features
            .iter()
            .flat_map(|s| s.split_whitespace())
            .flat_map(|s| s.split(','))
            .filter(|s| !s.is_empty())
            .map(|s| InternedString::new(s))
            .collect::<Vec<InternedString>>()
    }
}

#[derive(Clone)]
pub struct Candidate {
    pub summary: Summary,
    pub replace: Option<Summary>,
}

#[derive(Clone)]
pub struct DepsFrame {
    pub parent: Summary,
    pub just_for_error_messages: bool,
    pub remaining_siblings: RcVecIter<DepInfo>,
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
            .peek()
            .map(|(_, (_, candidates, _))| candidates.len())
            .unwrap_or(0)
    }

    pub fn flatten<'a>(&'a self) -> impl Iterator<Item = (PackageId, Dependency)> + 'a {
        self.remaining_siblings
            .clone()
            .map(move |(_, (d, _, _))| (self.parent.package_id(), d))
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
            .reverse()
            .then_with(|| self.min_candidates().cmp(&other.min_candidates()))
    }
}

/// Note that a `OrdSet` is used for the remaining dependencies that need
/// activation. This set is sorted by how many candidates each dependency has.
///
/// This helps us get through super constrained portions of the dependency
/// graph quickly and hopefully lock down what later larger dependencies can
/// use (those with more candidates).
#[derive(Clone)]
pub struct RemainingDeps {
    /// a monotonic counter, increased for each new insertion.
    time: u32,
    /// the data is augmented by the insertion time.
    /// This insures that no two items will cmp eq.
    /// Forcing the OrdSet into a multi set.
    data: im_rc::OrdSet<(DepsFrame, u32)>,
}

impl RemainingDeps {
    pub fn new() -> RemainingDeps {
        RemainingDeps {
            time: 0,
            data: im_rc::OrdSet::new(),
        }
    }
    pub fn push(&mut self, x: DepsFrame) {
        let insertion_time = self.time;
        self.data.insert((x, insertion_time));
        self.time += 1;
    }
    pub fn pop_most_constrained(&mut self) -> Option<(bool, (Summary, (usize, DepInfo)))> {
        while let Some((mut deps_frame, insertion_time)) = self.data.remove_min() {
            let just_here_for_the_error_messages = deps_frame.just_for_error_messages;

            // Figure out what our next dependency to activate is, and if nothing is
            // listed then we're entirely done with this frame (yay!) and we can
            // move on to the next frame.
            if let Some(sibling) = deps_frame.remaining_siblings.next() {
                let parent = Summary::clone(&deps_frame.parent);
                self.data.insert((deps_frame, insertion_time));
                return Some((just_here_for_the_error_messages, (parent, sibling)));
            }
        }
        None
    }
    pub fn iter<'a>(&'a mut self) -> impl Iterator<Item = (PackageId, Dependency)> + 'a {
        self.data.iter().flat_map(|(other, _)| other.flatten())
    }
}

// Information about the dependencies for a crate, a tuple of:
//
// (dependency info, candidates, features activated)
pub type DepInfo = (Dependency, Rc<Vec<Candidate>>, Rc<Vec<InternedString>>);

/// All possible reasons that a package might fail to activate.
///
/// We maintain a list of conflicts for error reporting as well as backtracking
/// purposes. Each reason here is why candidates may be rejected or why we may
/// fail to resolve a dependency.
#[derive(Debug, Clone, PartialOrd, Ord, PartialEq, Eq)]
pub enum ConflictReason {
    /// There was a semver conflict, for example we tried to activate a package
    /// 1.0.2 but 1.1.0 was already activated (aka a compatible semver version
    /// is already activated)
    Semver,

    /// The `links` key is being violated. For example one crate in the
    /// dependency graph has `links = "foo"` but this crate also had that, and
    /// we're only allowed one per dependency graph.
    Links(InternedString),

    /// A dependency listed features that weren't actually available on the
    /// candidate. For example we tried to activate feature `foo` but the
    /// candidate we're activating didn't actually have the feature `foo`.
    MissingFeatures(String),

    // TODO: needs more info for `activation_error`
    // TODO: needs more info for `find_candidate`
    /// pub dep error
    PublicDependency,
}

impl ConflictReason {
    pub fn is_links(&self) -> bool {
        if let ConflictReason::Links(_) = *self {
            return true;
        }
        false
    }

    pub fn is_missing_features(&self) -> bool {
        if let ConflictReason::MissingFeatures(_) = *self {
            return true;
        }
        false
    }
}

/// A list of packages that have gotten in the way of resolving a dependency.
/// If resolving a dependency fails then this represents an incompatibility,
/// that dependency will never be resolve while all of these packages are active.
/// This is useless if the packages can't be simultaneously activated for other reasons.
pub type ConflictMap = BTreeMap<PackageId, ConflictReason>;

pub struct RcVecIter<T> {
    vec: Rc<Vec<T>>,
    rest: Range<usize>,
}

impl<T> RcVecIter<T> {
    pub fn new(vec: Rc<Vec<T>>) -> RcVecIter<T> {
        RcVecIter {
            rest: 0..vec.len(),
            vec,
        }
    }

    fn peek(&self) -> Option<(usize, &T)> {
        self.rest
            .clone()
            .next()
            .and_then(|i| self.vec.get(i).map(|val| (i, &*val)))
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

    fn next(&mut self) -> Option<Self::Item> {
        self.rest
            .next()
            .and_then(|i| self.vec.get(i).map(|val| (i, val.clone())))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        // rest is a std::ops::Range, which is an ExactSizeIterator.
        self.rest.size_hint()
    }
}

impl<T: Clone> ExactSizeIterator for RcVecIter<T> {}

pub struct RcList<T> {
    pub head: Option<Rc<(T, RcList<T>)>>,
}

impl<T> RcList<T> {
    pub fn new() -> RcList<T> {
        RcList { head: None }
    }

    pub fn push(&mut self, data: T) {
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

pub enum GraphNode {
    Add(PackageId),
    Link(PackageId, PackageId, Dependency),
}
