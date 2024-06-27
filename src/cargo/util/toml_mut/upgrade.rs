use std::fmt::Display;

use crate::CargoResult;

/// Upgrade an existing requirement to a new version.
/// Copied from cargo-edit.
pub(crate) fn upgrade_requirement(
    req: &str,
    version: &semver::Version,
) -> CargoResult<Option<(String, semver::VersionReq)>> {
    let req_text = req.to_string();
    let raw_req = semver::VersionReq::parse(&req_text)
        .expect("semver to generate valid version requirements");
    if raw_req.comparators.is_empty() {
        // Empty matches everything, no-change.
        Ok(None)
    } else {
        let comparators: CargoResult<Vec<_>> = raw_req
            .comparators
            .into_iter()
            .map(|p| set_comparator(p, version))
            .collect();
        let comparators = comparators?;
        let new_req = semver::VersionReq { comparators };
        let mut new_req_text = new_req.to_string();
        if new_req_text.starts_with('^') && !req.starts_with('^') {
            new_req_text.remove(0);
        }
        // Validate contract
        #[cfg(debug_assertions)]
        {
            assert!(
                new_req.matches(version),
                "New req {} is invalid, because {} does not match {}",
                new_req_text,
                new_req,
                version
            )
        }
        if new_req_text == req_text {
            Ok(None)
        } else {
            Ok(Some((new_req_text, new_req)))
        }
    }
}

fn set_comparator(
    mut pred: semver::Comparator,
    version: &semver::Version,
) -> CargoResult<semver::Comparator> {
    match pred.op {
        semver::Op::Wildcard => {
            pred.major = version.major;
            if pred.minor.is_some() {
                pred.minor = Some(version.minor);
            }
            if pred.patch.is_some() {
                pred.patch = Some(version.patch);
            }
            Ok(pred)
        }
        semver::Op::Exact => Ok(assign_partial_req(version, pred)),
        semver::Op::Greater | semver::Op::GreaterEq | semver::Op::Less | semver::Op::LessEq => {
            let user_pred = pred.to_string();
            Err(unsupported_version_req(user_pred))
        }
        semver::Op::Tilde => Ok(assign_partial_req(version, pred)),
        semver::Op::Caret => Ok(assign_partial_req(version, pred)),
        _ => {
            let user_pred = pred.to_string();
            Err(unsupported_version_req(user_pred))
        }
    }
}

fn assign_partial_req(
    version: &semver::Version,
    mut pred: semver::Comparator,
) -> semver::Comparator {
    pred.major = version.major;
    if pred.minor.is_some() {
        pred.minor = Some(version.minor);
    }
    if pred.patch.is_some() {
        pred.patch = Some(version.patch);
    }
    pred.pre = version.pre.clone();
    pred
}

fn unsupported_version_req(req: impl Display) -> anyhow::Error {
    anyhow::format_err!("Support for modifying {} is currently unsupported", req)
}

#[cfg(test)]
mod test {
    use super::*;

    mod upgrade_requirement {
        use super::*;

        #[track_caller]
        fn assert_req_bump<'a, O: Into<Option<&'a str>>>(version: &str, req: &str, expected: O) {
            let version = semver::Version::parse(version).unwrap();
            let actual = upgrade_requirement(req, &version)
                .unwrap()
                .map(|(actual, _req)| actual);
            let expected = expected.into();
            assert_eq!(actual.as_deref(), expected);
        }

        #[test]
        fn wildcard_major() {
            assert_req_bump("1.0.0", "*", None);
        }

        #[test]
        fn wildcard_minor() {
            assert_req_bump("1.0.0", "1.*", None);
            assert_req_bump("1.1.0", "1.*", None);
            assert_req_bump("2.0.0", "1.*", "2.*");
        }

        #[test]
        fn wildcard_patch() {
            assert_req_bump("1.0.0", "1.0.*", None);
            assert_req_bump("1.1.0", "1.0.*", "1.1.*");
            assert_req_bump("1.1.1", "1.0.*", "1.1.*");
            assert_req_bump("2.0.0", "1.0.*", "2.0.*");
        }

        #[test]
        fn caret_major() {
            assert_req_bump("1.0.0", "1", None);
            assert_req_bump("1.0.0", "^1", None);

            assert_req_bump("1.1.0", "1", None);
            assert_req_bump("1.1.0", "^1", None);

            assert_req_bump("2.0.0", "1", "2");
            assert_req_bump("2.0.0", "^1", "^2");
        }

        #[test]
        fn caret_minor() {
            assert_req_bump("1.0.0", "1.0", None);
            assert_req_bump("1.0.0", "^1.0", None);

            assert_req_bump("1.1.0", "1.0", "1.1");
            assert_req_bump("1.1.0", "^1.0", "^1.1");

            assert_req_bump("1.1.1", "1.0", "1.1");
            assert_req_bump("1.1.1", "^1.0", "^1.1");

            assert_req_bump("2.0.0", "1.0", "2.0");
            assert_req_bump("2.0.0", "^1.0", "^2.0");
        }

        #[test]
        fn caret_patch() {
            assert_req_bump("1.0.0", "1.0.0", None);
            assert_req_bump("1.0.0", "^1.0.0", None);

            assert_req_bump("1.1.0", "1.0.0", "1.1.0");
            assert_req_bump("1.1.0", "^1.0.0", "^1.1.0");

            assert_req_bump("1.1.1", "1.0.0", "1.1.1");
            assert_req_bump("1.1.1", "^1.0.0", "^1.1.1");

            assert_req_bump("2.0.0", "1.0.0", "2.0.0");
            assert_req_bump("2.0.0", "^1.0.0", "^2.0.0");
        }

        #[test]
        fn tilde_major() {
            assert_req_bump("1.0.0", "~1", None);
            assert_req_bump("1.1.0", "~1", None);
            assert_req_bump("2.0.0", "~1", "~2");
        }

        #[test]
        fn tilde_minor() {
            assert_req_bump("1.0.0", "~1.0", None);
            assert_req_bump("1.1.0", "~1.0", "~1.1");
            assert_req_bump("1.1.1", "~1.0", "~1.1");
            assert_req_bump("2.0.0", "~1.0", "~2.0");
        }

        #[test]
        fn tilde_patch() {
            assert_req_bump("1.0.0", "~1.0.0", None);
            assert_req_bump("1.1.0", "~1.0.0", "~1.1.0");
            assert_req_bump("1.1.1", "~1.0.0", "~1.1.1");
            assert_req_bump("2.0.0", "~1.0.0", "~2.0.0");
        }

        #[test]
        fn equal_major() {
            assert_req_bump("1.0.0", "=1", None);
            assert_req_bump("1.1.0", "=1", None);
            assert_req_bump("2.0.0", "=1", "=2");
        }

        #[test]
        fn equal_minor() {
            assert_req_bump("1.0.0", "=1.0", None);
            assert_req_bump("1.1.0", "=1.0", "=1.1");
            assert_req_bump("1.1.1", "=1.0", "=1.1");
            assert_req_bump("2.0.0", "=1.0", "=2.0");
        }

        #[test]
        fn equal_patch() {
            assert_req_bump("1.0.0", "=1.0.0", None);
            assert_req_bump("1.1.0", "=1.0.0", "=1.1.0");
            assert_req_bump("1.1.1", "=1.0.0", "=1.1.1");
            assert_req_bump("2.0.0", "=1.0.0", "=2.0.0");
        }
    }
}
