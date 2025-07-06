use super::features::{CliFeatures, RequestedFeatures};
use crate::core::{Dependency, PackageId, SourceId, Summary};
use crate::util::GlobalContext;
use crate::util::errors::CargoResult;
use crate::util::interning::InternedString;
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroU64;
use std::rc::Rc;
use std::time::{Duration, Instant};

pub struct ResolverProgress {
    ticks: u16,
    start: Instant,
    time_to_print: Duration,
    printed: bool,
    deps_time: Duration,
    /// Provides an escape hatch for machine with slow CPU for debugging and
    /// testing Cargo itself.
    /// See [rust-lang/cargo#6596](https://github.com/rust-lang/cargo/pull/6596) for more.
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
            // Architectures that do not have a modern processor, hardware emulation, etc.
            // In the test code we have `slow_cpu_multiplier`, but that is not accessible here.
            #[cfg(debug_assertions)]
            // ALLOWED: For testing cargo itself only. However, it was communicated as an public
            // interface to other developers, so keep it as-is, shouldn't add `__CARGO` prefix.
            #[allow(clippy::disallowed_methods)]
            slow_cpu_multiplier: std::env::var("CARGO_TEST_SLOW_CPU_MULTIPLIER")
                .ok()
                .and_then(|m| m.parse().ok())
                .unwrap_or(1),
        }
    }
    pub fn shell_status(&mut self, gctx: Option<&GlobalContext>) -> CargoResult<()> {
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
        if let Some(config) = gctx {
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
            // So lets fail the test fast if we have been running for too long.
            assert!(
                self.ticks < 50_000,
                "got to 50_000 ticks in {:?}",
                self.start.elapsed()
            );
            // The largest test in our suite takes less then 30 sec
            // with all the improvements to how fast a tick can go.
            // If any of them are removed then it takes more than I am willing to measure.
            // So lets fail the test fast if we have been running for too long.
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

/// The preferred way to store the set of activated features for a package.
/// This is sorted so that it impls Hash, and owns its contents,
/// needed so it can be part of the key for caching in the `DepsCache`.
/// It is also cloned often as part of `Context`, hence the `RC`.
/// `im-rs::OrdSet` was slower of small sets like this,
/// but this can change with improvements to std, im, or llvm.
/// Using a consistent type for this allows us to use the highly
/// optimized comparison operators like `is_subset` at the interfaces.
pub type FeaturesSet = Rc<BTreeSet<InternedString>>;

/// Resolver behavior, used to opt-in to new behavior that is
/// backwards-incompatible via the `resolver` field in the manifest.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum ResolveBehavior {
    /// V1 is the original resolver behavior.
    V1,
    /// V2 adds the new feature resolver.
    V2,
    /// V3 changes version preferences
    V3,
}

impl ResolveBehavior {
    pub fn from_manifest(resolver: &str) -> CargoResult<ResolveBehavior> {
        match resolver {
            "1" => Ok(ResolveBehavior::V1),
            "2" => Ok(ResolveBehavior::V2),
            "3" => Ok(ResolveBehavior::V3),
            s => anyhow::bail!(
                "`resolver` setting `{}` is not valid, valid options are \"1\", \"2\" or \"3\"",
                s
            ),
        }
    }

    pub fn to_manifest(&self) -> String {
        match self {
            ResolveBehavior::V1 => "1",
            ResolveBehavior::V2 => "2",
            ResolveBehavior::V3 => "3",
        }
        .to_owned()
    }
}

/// Options for how the resolve should work.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct ResolveOpts {
    /// Whether or not dev-dependencies should be included.
    ///
    /// This may be set to `false` by things like `cargo install` or `-Z avoid-dev-deps`.
    /// It also gets set to `false` when activating dependencies in the resolver.
    pub dev_deps: bool,
    /// Set of features requested on the command-line.
    pub features: RequestedFeatures,
}

impl ResolveOpts {
    /// Creates a `ResolveOpts` that resolves everything.
    pub fn everything() -> ResolveOpts {
        ResolveOpts {
            dev_deps: true,
            features: RequestedFeatures::CliFeatures(CliFeatures::new_all(true)),
        }
    }

    pub fn new(dev_deps: bool, features: RequestedFeatures) -> ResolveOpts {
        ResolveOpts { dev_deps, features }
    }
}

/// A key that when stord in a hash map ensures that there is only one
/// semver compatible version of each crate.
/// Find the activated version of a crate based on the name, source, and semver compatibility.
#[derive(Clone, PartialEq, Eq, Debug, Ord, PartialOrd)]
pub struct ActivationsKey(InternedString, SemverCompatibility, SourceId);

impl ActivationsKey {
    pub fn new(
        name: InternedString,
        ver: SemverCompatibility,
        source_id: SourceId,
    ) -> ActivationsKey {
        ActivationsKey(name, ver, source_id)
    }
}

impl std::hash::Hash for ActivationsKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state);
        self.1.hash(state);
        // self.2.hash(state); // Packages that only differ by SourceId are rare enough to not be worth hashing
    }
}

/// A type that represents when cargo treats two Versions as compatible.
/// Versions `a` and `b` are compatible if their left-most nonzero digit is the
/// same.
#[derive(Clone, Copy, Eq, PartialEq, Hash, Debug, PartialOrd, Ord)]
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
    pub fn as_activations_key(self) -> ActivationsKey {
        ActivationsKey(self.name(), self.version().into(), self.source_id())
    }
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
            .map(|(_, candidates, _)| candidates.len())
            .unwrap_or(0)
    }

    pub fn flatten(&self) -> impl Iterator<Item = (PackageId, &Dependency)> + '_ {
        self.remaining_siblings
            .remaining()
            .map(move |(d, _, _)| (self.parent.package_id(), d))
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

/// Note that an `OrdSet` is used for the remaining dependencies that need
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
    /// Forcing the `OrdSet` into a multi set.
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
    pub fn pop_most_constrained(&mut self) -> Option<(bool, (Summary, DepInfo))> {
        while let Some((mut deps_frame, insertion_time)) = self.data.remove_min() {
            let just_here_for_the_error_messages = deps_frame.just_for_error_messages;

            // Figure out what our next dependency to activate is, and if nothing is
            // listed then we're entirely done with this frame (yay!) and we can
            // move on to the next frame.
            let sibling = deps_frame.remaining_siblings.iter().next().cloned();
            if let Some(sibling) = sibling {
                let parent = Summary::clone(&deps_frame.parent);
                self.data.insert((deps_frame, insertion_time));
                return Some((just_here_for_the_error_messages, (parent, sibling)));
            }
        }
        None
    }
    pub fn iter(&mut self) -> impl Iterator<Item = (PackageId, &Dependency)> + '_ {
        self.data.iter().flat_map(|(other, _)| other.flatten())
    }
}

/// Information about the dependencies for a crate, a tuple of:
///
/// (dependency info, candidates, features activated)
pub type DepInfo = (Dependency, Rc<Vec<Summary>>, FeaturesSet);

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

    /// A dependency listed a feature that wasn't actually available on the
    /// candidate. For example we tried to activate feature `foo` but the
    /// candidate we're activating didn't actually have the feature `foo`.
    MissingFeature(InternedString),

    /// A dependency listed a feature that ended up being a required dependency.
    /// For example we tried to activate feature `foo` but the
    /// candidate we're activating didn't actually have the feature `foo`
    /// it had a dependency `foo` instead.
    RequiredDependencyAsFeature(InternedString),

    /// A dependency listed a feature for an optional dependency, but that
    /// optional dependency is "hidden" using namespaced `dep:` syntax.
    NonImplicitDependencyAsFeature(InternedString),
}

impl ConflictReason {
    pub fn is_links(&self) -> bool {
        matches!(self, ConflictReason::Links(_))
    }

    pub fn is_missing_feature(&self) -> bool {
        matches!(self, ConflictReason::MissingFeature(_))
    }

    pub fn is_required_dependency_as_features(&self) -> bool {
        matches!(self, ConflictReason::RequiredDependencyAsFeature(_))
    }
}

/// A list of packages that have gotten in the way of resolving a dependency.
/// If resolving a dependency fails then this represents an incompatibility,
/// that dependency will never be resolve while all of these packages are active.
/// This is useless if the packages can't be simultaneously activated for other reasons.
pub type ConflictMap = BTreeMap<PackageId, ConflictReason>;

pub struct RcVecIter<T> {
    vec: Rc<Vec<T>>,
    offset: usize,
}

impl<T> RcVecIter<T> {
    pub fn new(vec: Rc<Vec<T>>) -> RcVecIter<T> {
        RcVecIter { vec, offset: 0 }
    }

    pub fn peek(&self) -> Option<&T> {
        self.vec.get(self.offset)
    }

    pub fn remaining(&self) -> impl Iterator<Item = &T> + '_ {
        self.vec.get(self.offset..).into_iter().flatten()
    }

    pub fn iter(&mut self) -> impl Iterator<Item = &T> + '_ {
        let iter = self.vec.get(self.offset..).into_iter().flatten();
        // This call to `ìnspect()` is used to increment `self.offset` when iterating the inner `Vec`,
        // while keeping the `ExactSizeIterator` property.
        iter.inspect(|_| self.offset += 1)
    }
}

// Not derived to avoid `T: Clone`
impl<T> Clone for RcVecIter<T> {
    fn clone(&self) -> RcVecIter<T> {
        RcVecIter {
            vec: self.vec.clone(),
            offset: self.offset,
        }
    }
}
