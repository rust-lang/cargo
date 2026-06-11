//! Compatibility between semver's [`VersionReq`] and PubGrub's [`VersionSet`].
//!
//! PubGrub needs more operations on version requirements than the `semver`
//! crate provides on [`VersionReq`] (notably negation, intersection and union).
//! [`SemverPubgrub`] is a representation of a [`VersionReq`] that supports those
//! operations while remaining bug-for-bug compatible with semver's `matches`.
//!
//! This is a specialization (to [`semver::Version`]) and port of the
//! `semver-pubgrub` crate <https://github.com/pubgrub-rs/semver-pubgrub>,
//! adapted to the published `pubgrub` 0.4 API. The structure deliberately
//! mirrors `semver`'s `eval.rs` so the two can be kept in sync.

use std::cmp::{max, min};
use std::fmt::{self, Display};
use std::num::NonZeroU64;
use std::ops::Bound;

use pubgrub::{Range, VersionSet};
use semver::{BuildMetadata, Comparator, Op, Prerelease, Version, VersionReq};

/// A [`VersionReq`] re-expressed as a pair of PubGrub [`Range`]s, split into the
/// versions matched among normal releases and among pre-releases.
///
/// semver applies different rules to pre-release versions, so we track the two
/// kinds of matches separately and recombine them in [`Self::contains`].
#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct SemverPubgrub {
    normal: Range<Version>,
    pre: Range<Version>,
}

impl SemverPubgrub {
    pub fn empty() -> Self {
        SemverPubgrub {
            normal: Range::empty(),
            pre: Range::empty(),
        }
    }

    pub fn full() -> Self {
        SemverPubgrub {
            normal: Range::full(),
            pre: Range::full(),
        }
    }

    pub fn singleton(v: Version) -> Self {
        let is_pre = !v.pre.is_empty();
        let singleton = Range::<Version>::singleton(v);
        if !is_pre {
            SemverPubgrub {
                normal: singleton,
                pre: Range::empty(),
            }
        } else {
            SemverPubgrub {
                normal: Range::empty(),
                pre: singleton,
            }
        }
    }

    pub fn complement(&self) -> Self {
        SemverPubgrub {
            normal: self.normal.complement(),
            pre: self.pre.complement(),
        }
    }

    pub fn intersection(&self, other: &Self) -> Self {
        SemverPubgrub {
            normal: self.normal.intersection(&other.normal),
            pre: self.pre.intersection(&other.pre),
        }
    }

    pub fn contains(&self, v: &Version) -> bool {
        // Must be bug-for-bug compatible with `matches_req`:
        // https://github.com/dtolnay/semver/blob/master/src/eval.rs
        if v.pre.is_empty() {
            self.normal.contains(v)
        } else {
            self.pre.contains(v)
        }
    }

    pub fn union(&self, other: &Self) -> Self {
        SemverPubgrub {
            normal: self.normal.union(&other.normal),
            pre: self.pre.union(&other.pre),
        }
    }

    pub fn is_disjoint(&self, other: &Self) -> bool {
        self.normal.is_disjoint(&other.normal) && self.pre.is_disjoint(&other.pre)
    }

    pub fn subset_of(&self, other: &Self) -> bool {
        self.normal.subset_of(&other.normal) && self.pre.subset_of(&other.pre)
    }

    /// A range covering all versions in a single semver-compatibility bucket.
    pub fn compatibility(compat: &SemverCompatibility) -> Self {
        let r = compat.to_range();
        SemverPubgrub {
            normal: r.clone(),
            pre: r,
        }
    }

    /// Convert to a pair of bounds usable with
    /// [`BTreeMap::range`](std::collections::BTreeMap::range). Every version
    /// contained in `self` falls within the output, but the output may be
    /// wider than `self`. Returns `None` when the range is empty.
    pub fn bounding_range(&self) -> Option<(Bound<&Version>, Bound<&Version>)> {
        use Bound::*;
        let Some((ns, ne)) = self.normal.bounding_range() else {
            return self.pre.bounding_range();
        };
        let Some((ps, pe)) = self.pre.bounding_range() else {
            return Some((ns, ne));
        };
        let start = match (ns, ps) {
            (Included(n), Included(p)) => Included(min(n, p)),
            (Included(i), Excluded(e)) | (Excluded(e), Included(i)) => {
                if e < i {
                    Excluded(e)
                } else {
                    Included(i)
                }
            }
            (Excluded(n), Excluded(p)) => Excluded(min(n, p)),
            (Unbounded, _) | (_, Unbounded) => Unbounded,
        };
        let end = match (ne, pe) {
            (Included(n), Included(p)) => Included(max(n, p)),
            (Included(i), Excluded(e)) | (Excluded(e), Included(i)) => {
                if i < e {
                    Excluded(e)
                } else {
                    Included(i)
                }
            }
            (Excluded(n), Excluded(p)) => Excluded(max(n, p)),
            (Unbounded, _) | (_, Unbounded) => Unbounded,
        };
        Some((start, end))
    }
}

impl Display for SemverPubgrub {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SemverPubgrub {{ normal: {}, pre: {} }}", self.normal, self.pre)
    }
}

impl VersionSet for SemverPubgrub {
    type V = Version;

    fn empty() -> Self {
        Self::empty()
    }

    fn full() -> Self {
        Self::full()
    }

    fn singleton(v: Self::V) -> Self {
        Self::singleton(v)
    }

    fn complement(&self) -> Self {
        self.complement()
    }

    fn intersection(&self, other: &Self) -> Self {
        self.intersection(other)
    }

    fn contains(&self, v: &Self::V) -> bool {
        self.contains(v)
    }

    fn union(&self, other: &Self) -> Self {
        self.union(other)
    }

    fn is_disjoint(&self, other: &Self) -> bool {
        self.is_disjoint(other)
    }

    fn subset_of(&self, other: &Self) -> bool {
        self.subset_of(other)
    }
}

impl From<&VersionReq> for SemverPubgrub {
    fn from(req: &VersionReq) -> Self {
        let mut out = Self::full();
        // The normal range is the intersection of all comparators.
        for cmp in &req.comparators {
            out = out.intersection(&matches_impl(cmp));
        }
        // The pre-release range is the union of each comparator's pre window,
        // then intersected with the normal pre range.
        let mut pre = Range::empty();
        for cmp in &req.comparators {
            pre = pre.union(&pre_is_compatible(cmp));
        }
        out.pre = pre.intersection(&out.pre);
        out
    }
}

// ----- semver `eval.rs` port -------------------------------------------------

fn matches_impl(cmp: &Comparator) -> SemverPubgrub {
    match cmp.op {
        Op::Exact | Op::Wildcard => matches_exact(cmp),
        Op::Greater => matches_greater(cmp),
        Op::GreaterEq => matches_exact(cmp).union(&matches_greater(cmp)),
        Op::Less => matches_less(cmp),
        Op::LessEq => matches_exact(cmp).union(&matches_less(cmp)),
        Op::Tilde => matches_tilde(cmp),
        Op::Caret => matches_caret(cmp),
        _ => unreachable!("update to a semver version that supports this Op"),
    }
}

fn matches_exact(cmp: &Comparator) -> SemverPubgrub {
    let low = Version {
        major: cmp.major,
        minor: cmp.minor.unwrap_or(0),
        patch: cmp.patch.unwrap_or(0),
        pre: cmp.pre.clone(),
        build: BuildMetadata::EMPTY,
    };
    if !cmp.pre.is_empty() {
        return SemverPubgrub {
            normal: Range::empty(),
            pre: between(low, bump_pre),
        };
    }
    let normal = if cmp.patch.is_some() {
        between(low, bump_patch)
    } else if cmp.minor.is_some() {
        between(low, bump_minor)
    } else {
        between(low, bump_major)
    };

    SemverPubgrub {
        normal: simplified_to_normal(&normal),
        pre: Range::empty(),
    }
}

fn matches_greater(cmp: &Comparator) -> SemverPubgrub {
    let low = Version {
        major: cmp.major,
        minor: cmp.minor.unwrap_or(0),
        patch: cmp.patch.unwrap_or(0),
        pre: cmp.pre.clone(),
        build: BuildMetadata::EMPTY,
    };
    let bump = if cmp.patch.is_some() {
        bump_pre(&low)
    } else if cmp.minor.is_some() {
        bump_minor(&low)
    } else {
        bump_major(&low)
    };
    let low_bound = match bump {
        Bound::Included(_) => unreachable!(),
        Bound::Excluded(v) => Bound::Included(v),
        Bound::Unbounded => return SemverPubgrub::empty(),
    };
    let out = Range::from_range_bounds((low_bound, Bound::Unbounded));
    SemverPubgrub {
        normal: simplified_to_normal(&out),
        pre: out,
    }
}

fn matches_less(cmp: &Comparator) -> SemverPubgrub {
    let out = Range::strictly_lower_than(Version {
        major: cmp.major,
        minor: cmp.minor.unwrap_or(0),
        patch: cmp.patch.unwrap_or(0),
        pre: if cmp.patch.is_some() {
            cmp.pre.clone()
        } else {
            Prerelease::new("0").unwrap()
        },
        build: BuildMetadata::EMPTY,
    });
    SemverPubgrub {
        normal: simplified_to_normal(&out),
        pre: out,
    }
}

fn matches_tilde(cmp: &Comparator) -> SemverPubgrub {
    let low = Version {
        major: cmp.major,
        minor: cmp.minor.unwrap_or(0),
        patch: cmp.patch.unwrap_or(0),
        pre: cmp.pre.clone(),
        build: BuildMetadata::EMPTY,
    };
    if cmp.patch.is_some() {
        let out = between(low, bump_minor);
        return SemverPubgrub {
            normal: simplified_to_normal(&out),
            pre: out,
        };
    }
    let normal = if cmp.minor.is_some() {
        between(low, bump_minor)
    } else {
        between(low, bump_major)
    };
    SemverPubgrub {
        normal: simplified_to_normal(&normal),
        pre: Range::empty(),
    }
}

fn matches_caret(cmp: &Comparator) -> SemverPubgrub {
    let low = Version {
        major: cmp.major,
        minor: cmp.minor.unwrap_or(0),
        patch: cmp.patch.unwrap_or(0),
        pre: if cmp.patch.is_some() {
            cmp.pre.clone()
        } else {
            Prerelease::new("0").unwrap()
        },
        build: BuildMetadata::EMPTY,
    };
    let Some(minor) = cmp.minor else {
        let out = between(low, bump_major);
        return SemverPubgrub {
            normal: simplified_to_normal(&out),
            pre: out,
        };
    };

    if cmp.patch.is_none() {
        let out = if cmp.major > 0 {
            between(low, bump_major)
        } else {
            between(low, bump_minor)
        };
        return SemverPubgrub {
            normal: simplified_to_normal(&out),
            pre: out,
        };
    };

    let out = if cmp.major > 0 {
        between(low, bump_major)
    } else if minor > 0 {
        between(low, bump_minor)
    } else {
        between(low, bump_patch)
    };
    SemverPubgrub {
        normal: simplified_to_normal(&out),
        pre: out,
    }
}

fn pre_is_compatible(cmp: &Comparator) -> Range<Version> {
    if cmp.pre.is_empty() {
        return Range::empty();
    }
    let (Some(minor), Some(patch)) = (cmp.minor, cmp.patch) else {
        return Range::empty();
    };

    Range::between(
        Version {
            major: cmp.major,
            minor,
            patch,
            pre: Prerelease::new("0").unwrap(),
            build: BuildMetadata::EMPTY,
        },
        Version::new(cmp.major, minor, patch),
    )
}

// ----- bump helpers (port of semver-pubgrub `bump_helpers.rs`) ---------------

fn bump_major(v: &Version) -> Bound<Version> {
    match v.major.checked_add(1) {
        Some(new) => Bound::Excluded(Version {
            major: new,
            minor: 0,
            patch: 0,
            pre: Prerelease::new("0").unwrap(),
            build: BuildMetadata::EMPTY,
        }),
        None => Bound::Unbounded,
    }
}

fn bump_minor(v: &Version) -> Bound<Version> {
    match v.minor.checked_add(1) {
        Some(new) => Bound::Excluded(Version {
            major: v.major,
            minor: new,
            patch: 0,
            pre: Prerelease::new("0").unwrap(),
            build: BuildMetadata::EMPTY,
        }),
        None => bump_major(v),
    }
}

fn bump_patch(v: &Version) -> Bound<Version> {
    match v.patch.checked_add(1) {
        Some(new) => Bound::Excluded(Version {
            major: v.major,
            minor: v.minor,
            patch: new,
            pre: Prerelease::new("0").unwrap(),
            build: BuildMetadata::EMPTY,
        }),
        None => bump_minor(v),
    }
}

fn bump_pre(v: &Version) -> Bound<Version> {
    if !v.pre.is_empty() {
        Bound::Excluded(Version {
            major: v.major,
            minor: v.minor,
            patch: v.patch,
            pre: Prerelease::new(&format!("{}.0", v.pre)).unwrap(),
            build: BuildMetadata::EMPTY,
        })
    } else {
        bump_patch(v)
    }
}

fn between(low: Version, into: impl Fn(&Version) -> Bound<Version>) -> Range<Version> {
    let high = into(&low);
    Range::from_range_bounds((Bound::Included(low), high))
}

fn bump_up_to_normal(v: &Version) -> Option<Version> {
    if v.pre.is_empty() {
        None
    } else {
        Some(Version {
            major: v.major,
            minor: v.minor,
            patch: v.patch,
            pre: Prerelease::EMPTY,
            build: BuildMetadata::EMPTY,
        })
    }
}

fn simplified_bounds_to_normal(bounds: (Bound<Version>, Bound<Version>)) -> (Bound<Version>, Bound<Version>) {
    let (mut from, mut to) = bounds;
    if let Bound::Included(f) | Bound::Excluded(f) = &from {
        if let Some(n) = bump_up_to_normal(f) {
            from = Bound::Included(n)
        }
    };
    if let Bound::Included(f) | Bound::Excluded(f) = &to {
        if let Some(n) = bump_up_to_normal(f) {
            to = Bound::Excluded(n)
        }
    };
    (from, to)
}

fn simplified_to_normal(input: &Range<Version>) -> Range<Version> {
    Range::from_iter(
        input
            .iter()
            .map(|(from, to)| simplified_bounds_to_normal((from.clone(), to.clone()))),
    )
}

// ----- semver compatibility buckets ------------------------------------------

/// Describes when Cargo treats two versions as compatible: versions `a` and `b`
/// are compatible when their left-most nonzero component is equal.
///
/// PubGrub allows only one version of a package; Cargo allows one per
/// compatibility bucket. We therefore encode the bucket into the PubGrub
/// package identity (see [`super::package`]).
#[derive(Clone, Copy, Eq, PartialEq, Hash, Debug, PartialOrd, Ord)]
pub enum SemverCompatibility {
    Patch(u64),
    Minor(NonZeroU64),
    Major(NonZeroU64),
}

impl SemverCompatibility {
    /// The smallest (pre-release) version contained in this bucket.
    pub fn minimum(&self) -> Version {
        match *self {
            Self::Major(new) => Version {
                major: new.into(),
                minor: 0,
                patch: 0,
                pre: Prerelease::new("0").unwrap(),
                build: BuildMetadata::EMPTY,
            },
            Self::Minor(new) => Version {
                major: 0,
                minor: new.into(),
                patch: 0,
                pre: Prerelease::new("0").unwrap(),
                build: BuildMetadata::EMPTY,
            },
            Self::Patch(new) => Version {
                major: 0,
                minor: 0,
                patch: new,
                pre: Prerelease::new("0").unwrap(),
                build: BuildMetadata::EMPTY,
            },
        }
    }

    /// The smallest non-pre-release version contained in this bucket.
    pub fn canonical(&self) -> Version {
        match *self {
            Self::Major(new) => Version::new(new.into(), 0, 0),
            Self::Minor(new) => Version::new(0, new.into(), 0),
            Self::Patch(new) => Version::new(0, 0, new),
        }
    }

    pub fn next(&self) -> Option<SemverCompatibility> {
        let one = NonZeroU64::new(1).unwrap();
        match *self {
            Self::Patch(s) => Some(
                s.checked_add(1)
                    .map(Self::Patch)
                    .unwrap_or_else(|| Self::Minor(one)),
            ),
            Self::Minor(s) => Some(
                s.checked_add(1)
                    .map(Self::Minor)
                    .unwrap_or_else(|| Self::Major(one)),
            ),
            Self::Major(s) => s.checked_add(1).map(Self::Major),
        }
    }

    fn maximum_bound(&self) -> Bound<Version> {
        if let Some(next) = self.next() {
            Bound::Excluded(next.minimum())
        } else {
            Bound::Unbounded
        }
    }

    /// The PubGrub range matching exactly this compatibility bucket.
    pub fn to_range(&self) -> Range<Version> {
        Range::from_range_bounds((Bound::Included(self.minimum()), self.maximum_bound()))
    }
}

impl From<&Version> for SemverCompatibility {
    fn from(ver: &Version) -> Self {
        if let Some(m) = NonZeroU64::new(ver.major) {
            return SemverCompatibility::Major(m);
        }
        if let Some(m) = NonZeroU64::new(ver.minor) {
            return SemverCompatibility::Minor(m);
        }
        SemverCompatibility::Patch(ver.patch)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    const OPS: &[&str] = &["^", "~", "=", "<", ">", "<=", ">="];

    /// `SemverPubgrub::contains` must agree with semver's `VersionReq::matches`.
    #[test]
    fn contains_matches_semver() {
        let reqs = [
            "^1", "^1.2", "^1.2.3", "~1.2", "~1.2.3", "=1.2.3", ">1.2.3", ">=1.2.3", "<1.2.3",
            "<=1.2.3", "^0.2", "^0.0.3", "^0", "*", "1.*", "1.2.*", ">=1.2, <1.5",
            "^1.2.3-alpha", "=1.2.3-beta.1",
        ];
        let vers = [
            "0.0.1", "0.2.0", "0.2.5", "1.0.0", "1.2.0", "1.2.3", "1.2.4", "1.4.9", "1.5.0",
            "2.0.0", "1.2.3-alpha", "1.2.3-beta.1", "1.2.3-beta.2",
        ];
        for raw_req in reqs {
            let req = VersionReq::parse(raw_req).unwrap();
            let pg: SemverPubgrub = (&req).into();
            for raw_ver in vers {
                let ver = Version::parse(raw_ver).unwrap();
                assert_eq!(
                    req.matches(&ver),
                    pg.contains(&ver),
                    "mismatch for req `{raw_req}` and version `{raw_ver}`",
                );
            }
        }
    }

    /// Exhaustive-ish cross-check across a grid of operators and operands.
    #[test]
    fn contains_matches_semver_grid() {
        let operands = ["0", "0.0", "0.0.3", "0.2", "0.2.5", "1", "1.2", "1.2.3", "2.0.0"];
        let vers: Vec<Version> = [
            "0.0.1", "0.0.3", "0.2.0", "0.2.5", "1.0.0", "1.2.0", "1.2.3", "2.0.0", "2.1.0",
        ]
        .iter()
        .map(|v| Version::parse(v).unwrap())
        .collect();
        for op in OPS {
            for operand in operands {
                let raw_req = format!("{op}{operand}");
                let Ok(req) = VersionReq::parse(&raw_req) else {
                    continue;
                };
                let pg: SemverPubgrub = (&req).into();
                for ver in &vers {
                    assert_eq!(
                        req.matches(ver),
                        pg.contains(ver),
                        "mismatch for req `{raw_req}` and version `{ver}`",
                    );
                }
            }
        }
    }

    #[test]
    fn compatibility_buckets() {
        assert_eq!(
            SemverCompatibility::from(&Version::parse("1.2.3").unwrap()),
            SemverCompatibility::Major(NonZeroU64::new(1).unwrap())
        );
        assert_eq!(
            SemverCompatibility::from(&Version::parse("0.2.3").unwrap()),
            SemverCompatibility::Minor(NonZeroU64::new(2).unwrap())
        );
        assert_eq!(
            SemverCompatibility::from(&Version::parse("0.0.3").unwrap()),
            SemverCompatibility::Patch(3)
        );
    }
}
