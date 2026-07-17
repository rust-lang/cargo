//! Extend `semver::VersionReq` with  [`matches_prerelease`] which doesn't preclude pre-releases by default.
//!
//! Please refer to the semantic proposal, see [RFC 3493].
//!
//! [RFC 3493]: https://rust-lang.github.io/rfcs/3493-precise-pre-release-cargo-update.html

use semver::{Comparator, Op, Prerelease, Version, VersionReq};

pub(crate) fn matches_prerelease(req: &VersionReq, ver: &Version) -> bool {
    // Whether there are pre release version can be as lower bound
    let lower_bound_prerelease = &req.comparators.iter().any(|cmp| {
        if matches!(cmp.op, Op::Greater | Op::GreaterEq) && !cmp.pre.is_empty() {
            true
        } else {
            false
        }
    });
    for cmp in &req.comparators {
        if !matches_prerelease_impl(cmp, ver, lower_bound_prerelease) {
            return false;
        }
    }

    true
}

fn matches_prerelease_impl(cmp: &Comparator, ver: &Version, lower_bound_prerelease: &bool) -> bool {
    match cmp.op {
        Op::Exact | Op::Wildcard => matches_exact_prerelease(cmp, ver),
        Op::Greater => matches_greater(cmp, ver),
        Op::GreaterEq => {
            if matches_exact_prerelease(cmp, ver) {
                return true;
            }
            matches_greater(cmp, ver)
        }
        Op::Less => {
            if *lower_bound_prerelease {
                matches_less(&fill_partial_req(cmp), ver)
            } else {
                matches_less(&fill_partial_req_include_pre(cmp), ver)
            }
        }
        Op::LessEq => {
            if matches_exact_prerelease(cmp, ver) {
                return true;
            }
            matches_less(&fill_partial_req(cmp), ver)
        }
        Op::Tilde => matches_tilde_prerelease(cmp, ver),
        Op::Caret => matches_caret_prerelease(cmp, ver),
        _ => unreachable!(),
    }
}

// See https://github.com/dtolnay/semver/blob/69efd3cc770ead273a06ad1788477b3092996d29/src/eval.rs#L44-L62
fn matches_exact(cmp: &Comparator, ver: &Version) -> bool {
    if ver.major != cmp.major {
        return false;
    }

    if let Some(minor) = cmp.minor {
        if ver.minor != minor {
            return false;
        }
    }

    if let Some(patch) = cmp.patch {
        if ver.patch != patch {
            return false;
        }
    }

    ver.pre == cmp.pre
}

// See https://github.com/dtolnay/semver/blob/69efd3cc770ead273a06ad1788477b3092996d29/src/eval.rs#L64-L88
fn matches_greater(cmp: &Comparator, ver: &Version) -> bool {
    if ver.major != cmp.major {
        return ver.major > cmp.major;
    }

    match cmp.minor {
        None => return false,
        Some(minor) => {
            if ver.minor != minor {
                return ver.minor > minor;
            }
        }
    }

    match cmp.patch {
        None => return false,
        Some(patch) => {
            if ver.patch != patch {
                return ver.patch > patch;
            }
        }
    }

    ver.pre > cmp.pre
}

// See https://github.com/dtolnay/semver/blob/69efd3cc770ead273a06ad1788477b3092996d29/src/eval.rs#L90-L114
fn matches_less(cmp: &Comparator, ver: &Version) -> bool {
    if ver.major != cmp.major {
        return ver.major < cmp.major;
    }

    match cmp.minor {
        None => return false,
        Some(minor) => {
            if ver.minor != minor {
                return ver.minor < minor;
            }
        }
    }

    match cmp.patch {
        None => return false,
        Some(patch) => {
            if ver.patch != patch {
                return ver.patch < patch;
            }
        }
    }

    ver.pre < cmp.pre
}

fn fill_partial_req(cmp: &Comparator) -> Comparator {
    let mut cmp = cmp.clone();
    if cmp.minor.is_none() {
        cmp.minor = Some(0);
        cmp.patch = Some(0);
    } else if cmp.patch.is_none() {
        cmp.patch = Some(0);
    }
    cmp
}

fn fill_partial_req_include_pre(cmp: &Comparator) -> Comparator {
    let mut cmp = cmp.clone();
    if cmp.minor.is_none() {
        cmp.minor = Some(0);
        cmp.patch = Some(0);
        cmp.pre = Prerelease::new("0").unwrap();
    } else if cmp.patch.is_none() {
        cmp.patch = Some(0);
    }
    if cmp.pre.is_empty() {
        cmp.pre = Prerelease::new("0").unwrap();
    }
    cmp
}

fn matches_exact_prerelease(cmp: &Comparator, ver: &Version) -> bool {
    if matches_exact(cmp, ver) {
        return true;
    }

    // If the comparator has a prerelease tag like =3.0.0-alpha.24,
    // then it should be only exactly match 3.0.0-alpha.24.
    if !cmp.pre.is_empty() {
        return false;
    }

    if !matches_greater(&fill_partial_req(cmp), ver) {
        return false;
    }

    let mut upper = Comparator {
        op: Op::Less,
        pre: Prerelease::new("0").unwrap(),
        ..cmp.clone()
    };

    match (upper.minor.is_some(), upper.patch.is_some()) {
        (true, true) => {
            upper.patch = Some(upper.patch.unwrap() + 1);
        }
        (true, false) => {
            // Partial Exact VersionReq eg. =0.24
            upper.minor = Some(upper.minor.unwrap() + 1);
            upper.patch = Some(0);
        }
        (false, false) => {
            // Partial Exact VersionReq eg. =0
            upper.major += 1;
            upper.minor = Some(0);
            upper.patch = Some(0);
        }
        _ => {}
    }

    matches_less(&upper, ver)
}

fn matches_tilde_prerelease(cmp: &Comparator, ver: &Version) -> bool {
    if matches_exact(cmp, ver) {
        return true;
    }

    if !matches_greater(&fill_partial_req(cmp), ver) {
        return false;
    }

    let mut upper = Comparator {
        op: Op::Less,
        pre: Prerelease::new("0").unwrap(),
        ..cmp.clone()
    };

    match (upper.minor.is_some(), upper.patch.is_some()) {
        (true, _) => {
            upper.minor = Some(upper.minor.unwrap() + 1);
            upper.patch = Some(0);
        }
        (false, false) => {
            upper.major += 1;
            upper.minor = Some(0);
            upper.patch = Some(0);
        }
        _ => {}
    }

    matches_less(&upper, ver)
}

fn matches_caret_prerelease(cmp: &Comparator, ver: &Version) -> bool {
    if matches_exact(cmp, ver) {
        return true;
    }

    if !matches_greater(&fill_partial_req(cmp), ver) {
        return false;
    }

    let mut upper = Comparator {
        op: Op::Less,
        pre: Prerelease::new("0").unwrap(),
        ..cmp.clone()
    };

    match (
        upper.major > 0,
        upper.minor.is_some(),
        upper.patch.is_some(),
    ) {
        (true, _, _) | (_, false, false) => {
            upper.major += 1;
            upper.minor = Some(0);
            upper.patch = Some(0);
        }
        (_, true, false) => {
            upper.minor = Some(upper.minor.unwrap() + 1);
            upper.patch = Some(0);
        }
        (_, true, _) if upper.minor.unwrap() > 0 => {
            upper.minor = Some(upper.minor.unwrap() + 1);
            upper.patch = Some(0);
        }
        (_, true, _) if upper.minor.unwrap() == 0 => {
            if upper.patch.is_none() {
                upper.patch = Some(1);
            } else {
                upper.patch = Some(upper.patch.unwrap() + 1);
            }
        }
        _ => {}
    }

    matches_less(&upper, ver)
}

#[cfg(test)]
mod matches_prerelease_semantic {
    use crate::util::semver_ext::VersionReqExt;
    use semver::{Version, VersionReq};

    fn assert_match_all(req: &VersionReq, versions: &[&str]) {
        for string in versions {
            let parsed = Version::parse(string).unwrap();
            assert!(
                req.matches_prerelease(&parsed),
                "{} did not match {}",
                req,
                string,
            );
        }
    }

    fn assert_match_none(req: &VersionReq, versions: &[&str]) {
        for string in versions {
            let parsed = Version::parse(string).unwrap();
            assert!(
                !req.matches_prerelease(&parsed),
                "{} matched {}",
                req,
                string
            );
        }
    }

    pub(super) fn req(text: &str) -> VersionReq {
        VersionReq::parse(text).unwrap()
    }

    #[test]
    fn test_exact() {
        // =I.J.K-pre only match I.J.K-pre
        let ref r = req("=4.2.1-0");
        // Only exactly match 4.2.1-0
        assert_match_all(r, &["4.2.1-0"]);
        // Not match others
        assert_match_none(r, &["1.2.3", "4.2.0", "4.2.1-1", "4.2.2"]);

        // =I.J.K equivalent to >=I.J.K, <I.J.(K+1)-0
        for r in &[req("=4.2.1"), req(">=4.2.1, <4.2.2-0")] {
            assert_match_all(r, &["4.2.1"]);
            assert_match_none(r, &["1.2.3", "4.2.1-0", "4.2.2-0", "4.2.2"]);
        }

        // =I.J equivalent to >=I.J.0, <I.(J+1).0-0
        for r in &[req("=4.2"), req(">=4.2.0, <4.3.0-0")] {
            assert_match_all(r, &["4.2.0", "4.2.1", "4.2.9"]);
            assert_match_none(r, &["0.0.1", "2.1.2-0", "4.2.0-0"]);
            assert_match_none(r, &["4.3.0-0", "4.3.0", "5.0.0-0", "5.0.0"]);
        }

        // =I equivalent to >=I.0.0, <(I+1).0.0-0
        for r in &[req("=4"), req(">=4.0.0, <5.0.0-0")] {
            assert_match_all(r, &["4.0.0", "4.2.1", "4.2.4-0", "4.9.9"]);
            assert_match_none(r, &["0.0.1", "2.1.2-0", "4.0.0-0"]);
            assert_match_none(r, &["5.0.0-0", "5.0.0", "5.0.1"]);
        }
    }

    #[test]
    fn test_greater_eq() {
        // >=I.J.K-0
        let ref r = req(">=4.2.1-0");
        assert_match_all(r, &["4.2.1-0", "4.2.1", "5.0.0"]);
        assert_match_none(r, &["0.0.0", "1.2.3"]);

        // >=I.J.K
        let ref r = req(">=4.2.1");
        assert_match_all(r, &["4.2.1", "5.0.0"]);
        assert_match_none(r, &["0.0.0", "4.2.1-0"]);

        // >=I.J equivalent to >=I.J.0
        for r in &[req(">=4.2"), req(">=4.2.0")] {
            assert_match_all(r, &["4.2.1-0", "4.2.0", "4.3.0"]);
            assert_match_none(r, &["0.0.0", "4.1.1", "4.2.0-0"]);
        }

        // >=I equivalent to >=I.0.0
        for r in &[req(">=4"), req(">=4.0.0")] {
            assert_match_all(r, &["4.0.0", "4.1.0-1", "5.0.0"]);
            assert_match_none(r, &["0.0.0", "1.2.3", "4.0.0-0"]);
        }
    }

    #[test]
    fn test_less() {
        // <I.J.K equivalent to <I.J.K-0
        for r in &[req("<4.2.1"), req("<4.2.1-0")] {
            assert_match_all(r, &["0.0.0", "4.0.0"]);
            assert_match_none(r, &["4.2.1-0", "4.2.2", "5.0.0-0", "5.0.0"]);
        }

        // <I.J equivalent to <I.J.0-0
        for r in &[req("<4.2"), req("<4.2.0-0")] {
            assert_match_all(r, &["0.0.0", "4.1.0"]);
            assert_match_none(r, &["4.2.0-0", "4.2.0", "4.3.0-0", "4.3.0"]);
        }

        // <I equivalent to <I.0.0-0
        for r in &[req("<4"), req("<4.0.0-0")] {
            assert_match_all(r, &["0.0.0", "3.9.0"]);
            assert_match_none(r, &["4.0.0-0", "4.0.0", "5.0.0-1", "5.0.0"]);
        }
    }

    #[test]
    fn test_less_upper_bound() {
        // Lower bound without prerelease tag, so upper bound equivalent to <I.J.K-0
        for r in &[
            req(">1.2.3, <2"),
            req(">1.2.3, <2.0"),
            req(">1.2.3, <2.0.0"),
            req(">=1.2.3, <2.0.0"),
            req(">1.2.3, <2.0.0-0"),
        ] {
            assert_match_all(r, &["1.2.4", "1.9.9"]);
            assert_match_none(r, &["2.0.0-0", "2.0.0", "2.1.2"]);
        }

        // Lower bound has prerelease tag, so upper bound doesn't change.
        for r in &[
            req(">1.2.3-0, <2"),
            req(">1.2.3-0, <2.0"),
            req(">1.2.3-0, <2.0.0"),
            req(">=1.2.3-0, <2.0.0"),
        ] {
            assert_match_all(r, &["1.2.4", "1.9.9", "2.0.0-0"]);
            assert_match_none(r, &["2.0.0", "2.1.2"]);
        }

        for r in &[
            req(">=2.0.0-0, <2"),
            req(">=2.0.0-0, <2.0"),
            req(">=2.0.0-0, <2.0.0"),
        ] {
            assert_match_all(r, &["2.0.0-0", "2.0.0-11"]);
            assert_match_none(r, &["0.0.9", "2.0.0"]);
        }

        // There is no intersection between lower bound and upper bound, in this case nothing matches
        let ref r = req(">5.0.0, <2.0.0");
        assert_match_none(r, &["1.2.3", "3.0.0", "6.0.0"]);
        let ref r = req(">5.0.0-0, <2.0.0");
        assert_match_none(r, &["1.2.3", "3.0.0", "6.0.0"]);
    }

    #[test]
    fn test_caret() {
        // ^I.J.K.0 (for I>0) — equivalent to >=I.J.K-0, <(I+1).0.0-0
        for r in &[req("^1.2.3-0"), req(">=1.2.3-0, <2.0.0-0")] {
            assert_match_all(r, &["1.2.3-0", "1.2.3-1", "1.2.3", "1.9.9"]);
            assert_match_none(r, &["0.0.9", "1.1.1-0", "2.0.0-0", "2.1.1"]);
        }

        // ^I.J.K (for I>0) — equivalent to >=I.J.K, <(I+1).0.0-0
        for r in &[req("^1.2.3"), req(">=1.2.3, <2.0.0-0")] {
            assert_match_all(r, &["1.2.3", "1.9.9"]);
            assert_match_none(
                r,
                &["0.0.9", "1.1.1-0", "1.2.3-0", "1.2.3-1", "2.0.0-0", "2.1.1"],
            );
        }

        // ^0.J.K-0 (for J>0) — equivalent to >=0.J.K-0, <0.(J+1).0-0
        for r in &[req("^0.2.3-0"), req(">=0.2.3-0, <0.3.0-0")] {
            assert_match_all(r, &["0.2.3-0", "0.2.3", "0.2.9-0", "0.2.9"]);
            assert_match_none(r, &["0.0.9", "0.3.0-0", "0.3.11", "1.1.1"]);
        }

        // ^0.J.K (for J>0) — equivalent to >=0.J.K-0, <0.(J+1).0-0
        for r in &[req("^0.2.3"), req(">=0.2.3, <0.3.0-0")] {
            assert_match_all(r, &["0.2.3", "0.2.9-0", "0.2.9"]);
            assert_match_none(r, &["0.0.9", "0.2.3-0", "0.3.0-0", "0.3.11", "1.1.1"]);
        }

        // ^0.0.K-0 — equivalent to >=0.0.K-0, <0.0.(K+1)-0
        for r in &[req("^0.0.3-0"), req(">=0.0.3-0, <0.1.0-0")] {
            assert_match_all(r, &["0.0.3-0", "0.0.3-1", "0.0.3"]);
            assert_match_none(r, &["0.0.1", "0.3.0-0", "0.4.0-0", "1.1.1"]);
        }

        // ^0.0.K — equivalent to >=0.0.K, <0.0.(K+1)-0
        for r in &[req("^0.0.3"), req(">=0.0.3, <0.1.0-0")] {
            assert_match_all(r, &["0.0.3"]);
            assert_match_none(
                r,
                &["0.0.1", "0.0.3-0", "0.3.0-0", "0.0.3-1", "0.4.0-0", "1.1.1"],
            );
        }

        // ^I.J (for I>0 or J>0) — equivalent to >=I.J.0, <(I+1).0.0-0)
        for r in &[req("^1.2"), req(">=1.2.0, <2.0.0-0")] {
            assert_match_all(r, &["1.2.0", "1.9.0-0", "1.9.9"]);
            assert_match_none(r, &["0.0.1", "0.0.4-0", "1.2.0-0", "2.0.0-0", "4.0.1"]);
        }

        // ^0.0 — equivalent to >=0.0.0, <0.1.0-0
        for r in &[req("^0.0"), req(">=0.0.0, <0.1.0-0")] {
            assert_match_all(r, &["0.0.0", "0.0.1", "0.0.4-0"]);
            assert_match_none(r, &["0.0.0-0", "0.1.0-0", "0.1.0", "1.1.1"]);
        }

        // ^I — equivalent to >=I.0.0, <(I+1).0.0-0
        for r in &[req("^1"), req(">=1.0.0, <2.0.0-0")] {
            assert_match_all(r, &["1.0.0", "1.0.1"]);
            assert_match_none(r, &["0.1.0-0", "0.1.0", "1.0.0-0", "2.0.0-0", "3.1.2"]);
        }
    }

    #[test]
    fn test_wildcard() {
        // I.J.* — equivalent to =I.J
        //
        // =I.J equivalent to >=I.J.0, <I.(J+1).0-0
        for r in &[req("4.2.*"), req("=4.2")] {
            // Match >= 4.2.0, < 4.3.0-0
            assert_match_all(r, &["4.2.0", "4.2.1", "4.2.9"]);
            // Not Match < 4.2.0
            assert_match_none(r, &["0.0.1", "2.1.2-0", "4.2.0-0"]);
            // Not Match >= 4.3.0-0
            assert_match_none(r, &["4.3.0-0", "4.3.0", "5.0.0", "5.0.1"]);
        }

        // I.* or I.*.* — equivalent to =I
        //
        // =I equivalent to >=I.0.0, <(I+1).0.0-0
        for r in &[req("4.*"), req("4.*.*"), req("=4")] {
            // Match >= 4.0.0, < 5.0.0-0
            assert_match_all(r, &["4.0.0", "4.2.1", "4.9.9"]);
            // Not Match < 4.0.0
            assert_match_none(r, &["0.0.1", "2.1.2-0", "4.0.0-0"]);
            // Not Match >= 5.0.0-0
            assert_match_none(r, &["5.0.0-0", "5.0.0", "5.0.1"]);
        }
    }

    #[test]
    fn test_greater() {
        // >I.J.K-0
        let ref r = req(">4.2.1-0");
        assert_match_all(r, &["4.2.1", "4.2.2", "5.0.0"]);
        assert_match_none(r, &["0.0.0", "4.2.1-0"]);

        // >I.J.K
        let ref r = req(">4.2.1");
        assert_match_all(r, &["4.2.2", "5.0.0-0", "5.0.0"]);
        assert_match_none(r, &["0.0.0", "4.2.1-0", "4.2.1"]);

        // >I.J equivalent to >=I.(J+1).0-0
        for r in &[req(">4.2"), req(">=4.3.0-0")] {
            assert_match_all(r, &["4.3.0-0", "4.3.0", "5.0.0"]);
            assert_match_none(r, &["0.0.0", "4.2.1"]);
        }

        // >I equivalent to >=(I+1).0.0-0
        for r in &[req(">4"), req(">=5.0.0-0")] {
            assert_match_all(r, &["5.0.0-0", "5.0.0"]);
            assert_match_none(r, &["0.0.0", "4.2.1"]);
        }
    }

    #[test]
    fn test_less_eq() {
        // <=I.J.K
        let ref r = req("<=4.2.1");
        assert_match_all(r, &["0.0.0", "4.2.1-0", "4.2.1"]);
        assert_match_none(r, &["4.2.2", "5.0.0-0", "5.0.0"]);
        // <=I.J.K-0
        let ref r = req("<=4.2.1-0");
        assert_match_all(r, &["0.0.0", "4.2.1-0"]);
        assert_match_none(r, &["4.2.1", "4.2.2", "5.0.0-0", "5.0.0"]);

        // <=I.J equivalent to <I.(J+1).0-0
        for r in &[req("<=4.2"), req("<4.3.0-0")] {
            assert_match_all(r, &["0.0.0", "4.2.0-0"]);
            assert_match_none(r, &["4.3.0-0", "4.3.0", "4.4.0"]);
        }

        // <=I equivalent to <(I+1).0.0-0
        for r in &[req("<=4"), req("<5.0.0-0")] {
            assert_match_all(r, &["0.0.0", "4.0.0-0", "4.0.0"]);
            assert_match_none(r, &["5.0.0-1", "5.0.0"]);
        }
    }

    #[test]
    fn test_tilde() {
        // ~I.J.K-0 — equivalent to >=I.J.K-0, <I.(J+1).0-0
        for r in &[req("~1.2.3-0"), req(">= 1.2.3-0, < 1.3.0-0")] {
            assert_match_all(r, &["1.2.3-0", "1.2.3", "1.2.4-0", "1.2.4"]);
            assert_match_none(r, &["0.0.1", "1.1.0-0"]);
            assert_match_none(r, &["1.3.0-0", "1.3.0", "1.3.1", "2.0.0"]);
        }

        // ~I.J.K — equivalent to >=I.J.K, <I.(J+1).0-0
        for r in &[req("~1.2.3"), req(">= 1.2.3, < 1.3.0-0")] {
            assert_match_all(r, &["1.2.3", "1.2.4-0", "1.2.4"]);
            assert_match_none(r, &["0.0.1", "1.1.0-0", "1.2.3-0"]);
            assert_match_none(r, &["1.3.0-0", "1.3.0", "1.3.1", "2.0.0"]);
        }

        // ~I.J — equivalent to >=I.J.0, <I.(J+1).0-0
        for r in &[req("~0.24"), req(">=0.24.0, <0.25.0-0")] {
            assert_match_all(r, &["0.24.0", "0.24.1-0", "0.24.1", "0.24.9"]);
            assert_match_none(r, &["0.0.1", "0.9.9", "0.24.0-0"]);
            assert_match_none(r, &["0.25.0-0", "1.1.0", "1.2.3", "2.0.0"]);
        }

        // ~I — >=I.0.0, <(I+1).0.0-0
        for r in &[req("~1"), req(">=1.0.0, <2.0.0-0")] {
            assert_match_all(r, &["1.0.0", "1.1.0-0", "1.1.0"]);
            assert_match_none(r, &["0.0.1", "0.9.9", "1.0.0-0"]);
            assert_match_none(r, &["2.0.0-0", "2.0.0", "2.0.1"]);
        }
    }
}
