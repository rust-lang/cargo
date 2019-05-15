use std::fmt;

use crate::core::{Dependency, PackageId, Registry, Summary};
use crate::util::lev_distance::lev_distance;
use crate::util::Config;
use failure::{Error, Fail};
use semver;

use super::context::Context;
use super::types::{ConflictMap, ConflictReason};

/// Error during resolution providing a path of `PackageId`s.
pub struct ResolveError {
    cause: Error,
    package_path: Vec<PackageId>,
}

impl ResolveError {
    pub fn new<E: Into<Error>>(cause: E, package_path: Vec<PackageId>) -> Self {
        Self {
            cause: cause.into(),
            package_path,
        }
    }

    /// Returns a path of packages from the package whose requirements could not be resolved up to
    /// the root.
    pub fn package_path(&self) -> &[PackageId] {
        &self.package_path
    }
}

impl Fail for ResolveError {
    fn cause(&self) -> Option<&dyn Fail> {
        self.cause.as_fail().cause()
    }
}

impl fmt::Debug for ResolveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.cause.fmt(f)
    }
}

impl fmt::Display for ResolveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.cause.fmt(f)
    }
}

pub type ActivateResult<T> = Result<T, ActivateError>;

#[derive(Debug)]
pub enum ActivateError {
    Fatal(failure::Error),
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

pub(super) fn activation_error(
    cx: &Context,
    registry: &mut dyn Registry,
    parent: &Summary,
    dep: &Dependency,
    conflicting_activations: &ConflictMap,
    candidates: &[Summary],
    config: Option<&Config>,
) -> ResolveError {
    let to_resolve_err = |err| {
        ResolveError::new(
            err,
            cx.parents
                .path_to_bottom(&parent.package_id())
                .into_iter()
                .cloned()
                .collect(),
        )
    };

    if !candidates.is_empty() {
        let mut msg = format!("failed to select a version for `{}`.", dep.package_name());
        msg.push_str("\n    ... required by ");
        msg.push_str(&describe_path(
            &cx.parents.path_to_bottom(&parent.package_id()),
        ));

        msg.push_str("\nversions that meet the requirements `");
        msg.push_str(&dep.version_req().to_string());
        msg.push_str("` are: ");
        msg.push_str(
            &candidates
                .iter()
                .map(|v| v.version())
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join(", "),
        );

        let mut conflicting_activations: Vec<_> = conflicting_activations.iter().collect();
        conflicting_activations.sort_unstable();
        let (links_errors, mut other_errors): (Vec<_>, Vec<_>) = conflicting_activations
            .drain(..)
            .rev()
            .partition(|&(_, r)| r.is_links());

        for &(p, r) in links_errors.iter() {
            if let ConflictReason::Links(ref link) = *r {
                msg.push_str("\n\nthe package `");
                msg.push_str(&*dep.package_name());
                msg.push_str("` links to the native library `");
                msg.push_str(link);
                msg.push_str("`, but it conflicts with a previous package which links to `");
                msg.push_str(link);
                msg.push_str("` as well:\n");
            }
            msg.push_str(&describe_path(&cx.parents.path_to_bottom(p)));
        }

        let (features_errors, mut other_errors): (Vec<_>, Vec<_>) = other_errors
            .drain(..)
            .partition(|&(_, r)| r.is_missing_features());

        for &(p, r) in features_errors.iter() {
            if let ConflictReason::MissingFeatures(ref features) = *r {
                msg.push_str("\n\nthe package `");
                msg.push_str(&*p.name());
                msg.push_str("` depends on `");
                msg.push_str(&*dep.package_name());
                msg.push_str("`, with features: `");
                msg.push_str(features);
                msg.push_str("` but `");
                msg.push_str(&*dep.package_name());
                msg.push_str("` does not have these features.\n");
            }
            // p == parent so the full path is redundant.
        }

        let (required_dependency_as_features_errors, other_errors): (Vec<_>, Vec<_>) = other_errors
            .drain(..)
            .partition(|&(_, r)| r.is_required_dependency_as_features());

        for &(p, r) in required_dependency_as_features_errors.iter() {
            if let ConflictReason::RequiredDependencyAsFeatures(ref features) = *r {
                msg.push_str("\n\nthe package `");
                msg.push_str(&*p.name());
                msg.push_str("` depends on `");
                msg.push_str(&*dep.package_name());
                msg.push_str("`, with features: `");
                msg.push_str(features);
                msg.push_str("` but `");
                msg.push_str(&*dep.package_name());
                msg.push_str("` does not have these features.\n");
                msg.push_str(
                    " It has a required dependency with that name, \
                     but only optional dependencies can be used as features.\n",
                );
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
            msg.push_str(&describe_path(&cx.parents.path_to_bottom(p)));
        }

        msg.push_str("\n\nfailed to select a version for `");
        msg.push_str(&*dep.package_name());
        msg.push_str("` which could resolve this conflict");

        return to_resolve_err(failure::format_err!("{}", msg));
    }

    // We didn't actually find any candidates, so we need to
    // give an error message that nothing was found.
    //
    // Maybe the user mistyped the ver_req? Like `dep="2"` when `dep="0.2"`
    // was meant. So we re-query the registry with `deb="*"` so we can
    // list a few versions that were actually found.
    let all_req = semver::VersionReq::parse("*").unwrap();
    let mut new_dep = dep.clone();
    new_dep.set_version_req(all_req);
    let mut candidates = match registry.query_vec(&new_dep, false) {
        Ok(candidates) => candidates,
        Err(e) => return to_resolve_err(e),
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
            "failed to select a version for the requirement `{} = \"{}\"`\n  \
             candidate versions found which didn't match: {}\n  \
             location searched: {}\n",
            dep.package_name(),
            dep.version_req(),
            versions,
            registry.describe_source(dep.source_id()),
        );
        msg.push_str("required by ");
        msg.push_str(&describe_path(
            &cx.parents.path_to_bottom(&parent.package_id()),
        ));

        // If we have a path dependency with a locked version, then this may
        // indicate that we updated a sub-package and forgot to run `cargo
        // update`. In this case try to print a helpful error!
        if dep.source_id().is_path() && dep.version_req().to_string().starts_with('=') {
            msg.push_str(
                "\nconsider running `cargo update` to update \
                 a path dependency's locked version",
            );
        }

        if registry.is_replaced(dep.source_id()) {
            msg.push_str("\nperhaps a crate was updated and forgotten to be re-vendored?");
        }

        msg
    } else {
        // Maybe the user mistyped the name? Like `dep-thing` when `Dep_Thing`
        // was meant. So we try asking the registry for a `fuzzy` search for suggestions.
        let mut candidates = Vec::new();
        if let Err(e) = registry.query(&new_dep, &mut |s| candidates.push(s.name()), true) {
            return to_resolve_err(e);
        };
        candidates.sort_unstable();
        candidates.dedup();
        let mut candidates: Vec<_> = candidates
            .iter()
            .map(|n| (lev_distance(&*new_dep.package_name(), &*n), n))
            .filter(|&(d, _)| d < 4)
            .collect();
        candidates.sort_by_key(|o| o.0);
        let mut msg = format!(
            "no matching package named `{}` found\n\
             location searched: {}\n",
            dep.package_name(),
            dep.source_id()
        );
        if !candidates.is_empty() {
            let mut names = candidates
                .iter()
                .take(3)
                .map(|c| c.1.as_str())
                .collect::<Vec<_>>();

            if candidates.len() > 3 {
                names.push("...");
            }

            msg.push_str("perhaps you meant: ");
            msg.push_str(&names.iter().enumerate().fold(
                String::default(),
                |acc, (i, el)| match i {
                    0 => acc + el,
                    i if names.len() - 1 == i && candidates.len() <= 3 => acc + " or " + el,
                    _ => acc + ", " + el,
                },
            ));
            msg.push_str("\n");
        }
        msg.push_str("required by ");
        msg.push_str(&describe_path(
            &cx.parents.path_to_bottom(&parent.package_id()),
        ));

        msg
    };

    if let Some(config) = config {
        if config.offline() {
            msg.push_str(
                "\nAs a reminder, you're using offline mode (--offline) \
                 which can sometimes cause surprising resolution failures, \
                 if this error is too confusing you may wish to retry \
                 without the offline flag.",
            );
        }
    }

    to_resolve_err(failure::format_err!("{}", msg))
}

/// Returns String representation of dependency chain for a particular `pkgid`.
pub(super) fn describe_path(path: &[&PackageId]) -> String {
    use std::fmt::Write;
    let mut dep_path_desc = format!("package `{}`", path[0]);
    for dep in path[1..].iter() {
        write!(dep_path_desc, "\n    ... which is depended on by `{}`", dep).unwrap();
    }
    dep_path_desc
}
