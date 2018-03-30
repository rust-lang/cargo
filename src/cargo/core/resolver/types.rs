use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::rc::Rc;

use core::{Dependency, PackageId, PackageIdSpec, Registry, Summary};
use util::{CargoError, CargoResult};

pub struct RegistryQueryer<'a> {
    pub registry: &'a mut (Registry + 'a),
    replacements: &'a [(PackageIdSpec, Dependency)],
    try_to_use: &'a HashSet<&'a PackageId>,
    // TODO: with nll the Rc can be removed
    cache: HashMap<Dependency, Rc<Vec<Candidate>>>,
    // If set the list of dependency candidates will be sorted by minimal
    // versions first. That allows `cargo update -Z minimal-versions` which will
    // specify minimum depedency versions to be used.
    minimal_versions: bool,
}

impl<'a> RegistryQueryer<'a> {
    pub fn new(
        registry: &'a mut Registry,
        replacements: &'a [(PackageIdSpec, Dependency)],
        try_to_use: &'a HashSet<&'a PackageId>,
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

        // When we attempt versions for a package we'll want to do so in a
        // sorted fashion to pick the "best candidates" first. Currently we try
        // prioritized summaries (those in `try_to_use`) and failing that we
        // list everything from the maximum version to the lowest version.
        ret.sort_unstable_by(|a, b| {
            let a_in_previous = self.try_to_use.contains(a.summary.package_id());
            let b_in_previous = self.try_to_use.contains(b.summary.package_id());
            let previous_cmp = a_in_previous.cmp(&b_in_previous).reverse();
            match previous_cmp {
                Ordering::Equal => {
                    let cmp = a.summary.version().cmp(&b.summary.version());
                    if self.minimal_versions == true {
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
            .clone()
            .next()
            .map(|(_, (_, candidates, _))| candidates.len())
            .unwrap_or(0)
    }

    pub fn flatten<'s>(&'s self) -> Box<Iterator<Item = (&PackageId, Dependency)> + 's> {
        // TODO: with impl Trait the Box can be removed
        Box::new(
            self.remaining_siblings
                .clone()
                .map(move |(_, (d, _, _))| (self.parent.package_id(), d)),
        )
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

// Information about the dependencies for a crate, a tuple of:
//
// (dependency info, candidates, features activated)
pub type DepInfo = (Dependency, Rc<Vec<Candidate>>, Rc<Vec<String>>);

pub type ActivateResult<T> = Result<T, ActivateError>;

pub enum ActivateError {
    Fatal(CargoError),
    Conflict(PackageId, ConflictReason),
}

impl From<::failure::Error> for ActivateError {
    fn from(t: ::failure::Error) -> Self {
        ActivateError::Fatal(t)
    }
}

impl From<(PackageId, ConflictReason)> for ActivateError {
    fn from(t: (PackageId, ConflictReason)) -> Self {
        ActivateError::Conflict(t.0, t.1)
    }
}

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
    Links(String),

    /// A dependency listed features that weren't actually available on the
    /// candidate. For example we tried to activate feature `foo` but the
    /// candidiate we're activating didn't actually have the feature `foo`.
    MissingFeatures(String),
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
    Link(PackageId, PackageId),
}
