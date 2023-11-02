use std::fmt;
use std::task::Poll;

use crate::core::{Dependency, PackageId, Registry, Summary};
use crate::sources::source::QueryKind;
use crate::util::edit_distance::edit_distance;
use crate::util::{Config, OptVersionReq, VersionExt};
use anyhow::Error;

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

impl std::error::Error for ResolveError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.cause.source()
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
    Fatal(anyhow::Error),
    Conflict(PackageId, ConflictReason),
}

impl From<::anyhow::Error> for ActivateError {
    fn from(t: ::anyhow::Error) -> Self {
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
                .map(|(node, _)| node)
                .cloned()
                .collect(),
        )
    };

    if !candidates.is_empty() {
        let mut msg = format!("failed to select a version for `{}`.", dep.package_name());
        msg.push_str("\n    ... required by ");
        msg.push_str(&describe_path_in_context(cx, &parent.package_id()));

        msg.push_str("\nversions that meet the requirements `");
        msg.push_str(&dep.version_req().to_string());
        msg.push_str("` ");

        if let Some(v) = dep.version_req().locked_version() {
            msg.push_str("(locked to ");
            msg.push_str(&v.to_string());
            msg.push_str(") ");
        }

        msg.push_str("are: ");
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
        // This is reversed to show the newest versions first. I don't know if there is
        // a strong reason to do this, but that is how the code previously worked
        // (see https://github.com/rust-lang/cargo/pull/5037) and I don't feel like changing it.
        conflicting_activations.reverse();
        // Flag used for grouping all semver errors together.
        let mut has_semver = false;

        for (p, r) in &conflicting_activations {
            match r {
                ConflictReason::Semver => {
                    has_semver = true;
                }
                ConflictReason::Links(link) => {
                    msg.push_str("\n\nthe package `");
                    msg.push_str(&*dep.package_name());
                    msg.push_str("` links to the native library `");
                    msg.push_str(link);
                    msg.push_str("`, but it conflicts with a previous package which links to `");
                    msg.push_str(link);
                    msg.push_str("` as well:\n");
                    msg.push_str(&describe_path_in_context(cx, p));
                    msg.push_str("\nOnly one package in the dependency graph may specify the same links value. This helps ensure that only one copy of a native library is linked in the final binary. ");
                    msg.push_str("Try to adjust your dependencies so that only one package uses the links ='");
                    msg.push_str(&*dep.package_name());
                    msg.push_str("' value. For more information, see https://doc.rust-lang.org/cargo/reference/resolver.html#links.");
                }
                ConflictReason::MissingFeatures(features) => {
                    msg.push_str("\n\nthe package `");
                    msg.push_str(&*p.name());
                    msg.push_str("` depends on `");
                    msg.push_str(&*dep.package_name());
                    msg.push_str("`, with features: `");
                    msg.push_str(features);
                    msg.push_str("` but `");
                    msg.push_str(&*dep.package_name());
                    msg.push_str("` does not have these features.\n");
                    // p == parent so the full path is redundant.
                }
                ConflictReason::RequiredDependencyAsFeature(features) => {
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
                    // p == parent so the full path is redundant.
                }
                ConflictReason::NonImplicitDependencyAsFeature(features) => {
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
                        " It has an optional dependency with that name, \
                         but that dependency uses the \"dep:\" \
                         syntax in the features table, so it does not have an \
                         implicit feature with that name.\n",
                    );
                    // p == parent so the full path is redundant.
                }
                ConflictReason::PublicDependency(pkg_id) => {
                    // TODO: This needs to be implemented.
                    unimplemented!("pub dep {:?}", pkg_id);
                }
                ConflictReason::PubliclyExports(pkg_id) => {
                    // TODO: This needs to be implemented.
                    unimplemented!("pub exp {:?}", pkg_id);
                }
            }
        }

        if has_semver {
            // Group these errors together.
            msg.push_str("\n\nall possible versions conflict with previously selected packages.");
            for (p, r) in &conflicting_activations {
                if let ConflictReason::Semver = r {
                    msg.push_str("\n\n  previously selected ");
                    msg.push_str(&describe_path_in_context(cx, p));
                }
            }
        }

        msg.push_str("\n\nfailed to select a version for `");
        msg.push_str(&*dep.package_name());
        msg.push_str("` which could resolve this conflict");

        return to_resolve_err(anyhow::format_err!("{}", msg));
    }

    // We didn't actually find any candidates, so we need to
    // give an error message that nothing was found.
    //
    // Maybe the user mistyped the ver_req? Like `dep="2"` when `dep="0.2"`
    // was meant. So we re-query the registry with `dep="*"` so we can
    // list a few versions that were actually found.
    let mut new_dep = dep.clone();
    new_dep.set_version_req(OptVersionReq::Any);

    let mut candidates = loop {
        match registry.query_vec(&new_dep, QueryKind::Exact) {
            Poll::Ready(Ok(candidates)) => break candidates,
            Poll::Ready(Err(e)) => return to_resolve_err(e),
            Poll::Pending => match registry.block_until_ready() {
                Ok(()) => continue,
                Err(e) => return to_resolve_err(e),
            },
        }
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

        let locked_version = dep
            .version_req()
            .locked_version()
            .map(|v| format!(" (locked to {})", v))
            .unwrap_or_default();

        let mut msg = format!(
            "failed to select a version for the requirement `{} = \"{}\"`{}\n\
             candidate versions found which didn't match: {}\n\
             location searched: {}\n",
            dep.package_name(),
            dep.version_req(),
            locked_version,
            versions,
            registry.describe_source(dep.source_id()),
        );
        msg.push_str("required by ");
        msg.push_str(&describe_path_in_context(cx, &parent.package_id()));

        // If we have a pre-release candidate, then that may be what our user is looking for
        if let Some(pre) = candidates.iter().find(|c| c.version().is_prerelease()) {
            msg.push_str("\nif you are looking for the prerelease package it needs to be specified explicitly");
            msg.push_str(&format!(
                "\n    {} = {{ version = \"{}\" }}",
                pre.name(),
                pre.version()
            ));
        }

        // If we have a path dependency with a locked version, then this may
        // indicate that we updated a sub-package and forgot to run `cargo
        // update`. In this case try to print a helpful error!
        if dep.source_id().is_path() && dep.version_req().is_locked() {
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
        let mut candidates = loop {
            match registry.query_vec(&new_dep, QueryKind::Fuzzy) {
                Poll::Ready(Ok(candidates)) => break candidates,
                Poll::Ready(Err(e)) => return to_resolve_err(e),
                Poll::Pending => match registry.block_until_ready() {
                    Ok(()) => continue,
                    Err(e) => return to_resolve_err(e),
                },
            }
        };

        candidates.sort_unstable_by_key(|a| a.name());
        candidates.dedup_by(|a, b| a.name() == b.name());
        let mut candidates: Vec<_> = candidates
            .iter()
            .filter_map(|n| Some((edit_distance(&*new_dep.package_name(), &*n.name(), 3)?, n)))
            .collect();
        candidates.sort_by_key(|o| o.0);
        let mut msg: String;
        if candidates.is_empty() {
            msg = format!("no matching package named `{}` found\n", dep.package_name());
        } else {
            msg = format!(
                "no matching package found\nsearched package name: `{}`\n",
                dep.package_name()
            );
            let mut names = candidates
                .iter()
                .take(3)
                .map(|c| c.1.name().as_str())
                .collect::<Vec<_>>();

            if candidates.len() > 3 {
                names.push("...");
            }
            // Vertically align first suggestion with missing crate name
            // so a typo jumps out at you.
            msg.push_str("perhaps you meant:      ");
            msg.push_str(&names.iter().enumerate().fold(
                String::default(),
                |acc, (i, el)| match i {
                    0 => acc + el,
                    i if names.len() - 1 == i && candidates.len() <= 3 => acc + " or " + el,
                    _ => acc + ", " + el,
                },
            ));
            msg.push('\n');
        }
        msg.push_str(&format!("location searched: {}\n", dep.source_id()));
        msg.push_str("required by ");
        msg.push_str(&describe_path_in_context(cx, &parent.package_id()));

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

    to_resolve_err(anyhow::format_err!("{}", msg))
}

/// Returns String representation of dependency chain for a particular `pkgid`
/// within given context.
pub(super) fn describe_path_in_context(cx: &Context, id: &PackageId) -> String {
    let iter = cx
        .parents
        .path_to_bottom(id)
        .into_iter()
        .map(|(p, d)| (p, d.and_then(|d| d.iter().next())));
    describe_path(iter)
}

/// Returns String representation of dependency chain for a particular `pkgid`.
///
/// Note that all elements of `path` iterator should have `Some` dependency
/// except the first one. It would look like:
///
/// (pkg0, None)
/// -> (pkg1, dep from pkg1 satisfied by pkg0)
/// -> (pkg2, dep from pkg2 satisfied by pkg1)
/// -> ...
pub(crate) fn describe_path<'a>(
    mut path: impl Iterator<Item = (&'a PackageId, Option<&'a Dependency>)>,
) -> String {
    use std::fmt::Write;

    if let Some(p) = path.next() {
        let mut dep_path_desc = format!("package `{}`", p.0);
        for (pkg, dep) in path {
            let dep = dep.unwrap();
            let source_kind = if dep.source_id().is_path() {
                "path "
            } else if dep.source_id().is_git() {
                "git "
            } else {
                ""
            };
            let requirement = if source_kind.is_empty() {
                format!("{} = \"{}\"", dep.name_in_toml(), dep.version_req())
            } else {
                dep.name_in_toml().to_string()
            };
            let locked_version = dep
                .version_req()
                .locked_version()
                .map(|v| format!("(locked to {}) ", v))
                .unwrap_or_default();

            write!(
                dep_path_desc,
                "\n    ... which satisfies {}dependency `{}` {}of package `{}`",
                source_kind, requirement, locked_version, pkg
            )
            .unwrap();
        }

        return dep_path_desc;
    }

    String::new()
}
