use std::fmt;
use std::fmt::Write as _;
use std::task::Poll;

use crate::core::{Dependency, PackageId, Registry, Summary};
use crate::sources::IndexSummary;
use crate::sources::source::QueryKind;
use crate::util::edit_distance::{closest, edit_distance};
use crate::util::errors::CargoResult;
use crate::util::{GlobalContext, OptVersionReq, VersionExt};
use anyhow::Error;

use super::context::ResolverContext;
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
    resolver_ctx: &ResolverContext,
    registry: &mut dyn Registry,
    parent: &Summary,
    dep: &Dependency,
    conflicting_activations: &ConflictMap,
    candidates: &[Summary],
    gctx: Option<&GlobalContext>,
) -> ResolveError {
    let to_resolve_err = |err| {
        ResolveError::new(
            err,
            resolver_ctx
                .parents
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
        msg.push_str(&describe_path_in_context(
            resolver_ctx,
            &parent.package_id(),
        ));

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
                    msg.push_str("\n\npackage `");
                    msg.push_str(&*dep.package_name());
                    msg.push_str("` links to the native library `");
                    msg.push_str(link);
                    msg.push_str("`, but it conflicts with a previous package which links to `");
                    msg.push_str(link);
                    msg.push_str("` as well:\n");
                    msg.push_str(&describe_path_in_context(resolver_ctx, p));
                    msg.push_str("\nOnly one package in the dependency graph may specify the same links value. This helps ensure that only one copy of a native library is linked in the final binary. ");
                    msg.push_str("Try to adjust your dependencies so that only one package uses the `links = \"");
                    msg.push_str(link);
                    msg.push_str("\"` value. For more information, see https://doc.rust-lang.org/cargo/reference/resolver.html#links.");
                }
                ConflictReason::MissingFeature(feature) => {
                    msg.push_str("\n\npackage `");
                    msg.push_str(&*p.name());
                    msg.push_str("` depends on `");
                    msg.push_str(&*dep.package_name());
                    msg.push_str("` with feature `");
                    msg.push_str(feature);
                    msg.push_str("` but `");
                    msg.push_str(&*dep.package_name());
                    msg.push_str("` does not have that feature.\n");
                    let latest = candidates.last().expect("in the non-empty branch");
                    if let Some(closest) = closest(feature, latest.features().keys(), |k| k) {
                        msg.push_str(" package `");
                        msg.push_str(&*dep.package_name());
                        msg.push_str("` does have feature `");
                        msg.push_str(closest);
                        msg.push_str("`\n");
                    }
                    // p == parent so the full path is redundant.
                }
                ConflictReason::RequiredDependencyAsFeature(feature) => {
                    msg.push_str("\n\npackage `");
                    msg.push_str(&*p.name());
                    msg.push_str("` depends on `");
                    msg.push_str(&*dep.package_name());
                    msg.push_str("` with feature `");
                    msg.push_str(feature);
                    msg.push_str("` but `");
                    msg.push_str(&*dep.package_name());
                    msg.push_str("` does not have that feature.\n");
                    msg.push_str(
                        " A required dependency with that name exists, \
                         but only optional dependencies can be used as features.\n",
                    );
                    // p == parent so the full path is redundant.
                }
                ConflictReason::NonImplicitDependencyAsFeature(feature) => {
                    msg.push_str("\n\npackage `");
                    msg.push_str(&*p.name());
                    msg.push_str("` depends on `");
                    msg.push_str(&*dep.package_name());
                    msg.push_str("` with feature `");
                    msg.push_str(feature);
                    msg.push_str("` but `");
                    msg.push_str(&*dep.package_name());
                    msg.push_str("` does not have that feature.\n");
                    msg.push_str(
                        " An optional dependency with that name exists, \
                         but that dependency uses the \"dep:\" \
                         syntax in the features table, so it does not have an \
                         implicit feature with that name.\n",
                    );
                    // p == parent so the full path is redundant.
                }
            }
        }

        if has_semver {
            // Group these errors together.
            msg.push_str("\n\nall possible versions conflict with previously selected packages.");
            for (p, r) in &conflicting_activations {
                if let ConflictReason::Semver = r {
                    msg.push_str("\n\n  previously selected ");
                    msg.push_str(&describe_path_in_context(resolver_ctx, p));
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
    let mut msg = String::new();
    let mut hints = String::new();
    if let Some(version_candidates) = rejected_versions(registry, dep) {
        let version_candidates = match version_candidates {
            Ok(c) => c,
            Err(e) => return to_resolve_err(e),
        };

        let locked_version = dep
            .version_req()
            .locked_version()
            .map(|v| format!(" (locked to {})", v))
            .unwrap_or_default();
        let _ = writeln!(
            &mut msg,
            "failed to select a version for the requirement `{} = \"{}\"`{}",
            dep.package_name(),
            dep.version_req(),
            locked_version
        );
        for candidate in version_candidates {
            match candidate {
                IndexSummary::Candidate(summary) => {
                    // HACK: If this was a real candidate, we wouldn't hit this case.
                    // so it must be a patch which get normalized to being a candidate
                    let _ = writeln!(&mut msg, "  version {} is unavailable", summary.version());
                }
                IndexSummary::Yanked(summary) => {
                    let _ = writeln!(&mut msg, "  version {} is yanked", summary.version());
                }
                IndexSummary::Offline(summary) => {
                    let _ = writeln!(&mut msg, "  version {} is not cached", summary.version());
                }
                IndexSummary::Unsupported(summary, schema_version) => {
                    if let Some(rust_version) = summary.rust_version() {
                        // HACK: technically its unsupported and we shouldn't make assumptions
                        // about the entry but this is limited and for diagnostics purposes
                        let _ = writeln!(
                            &mut msg,
                            "  version {} requires cargo {}",
                            summary.version(),
                            rust_version
                        );
                    } else {
                        let _ = writeln!(
                            &mut msg,
                            "  version {} requires a Cargo version that supports index version {}",
                            summary.version(),
                            schema_version
                        );
                    }
                }
                IndexSummary::Invalid(summary) => {
                    let _ = writeln!(
                        &mut msg,
                        "  version {}'s index entry is invalid",
                        summary.version()
                    );
                }
            }
        }
    } else if let Some(candidates) = alt_versions(registry, dep) {
        let candidates = match candidates {
            Ok(c) => c,
            Err(e) => return to_resolve_err(e),
        };
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

        let _ = writeln!(
            &mut msg,
            "failed to select a version for the requirement `{} = \"{}\"`{}",
            dep.package_name(),
            dep.version_req(),
            locked_version,
        );
        let _ = writeln!(
            &mut msg,
            "candidate versions found which didn't match: {versions}",
        );

        // If we have a pre-release candidate, then that may be what our user is looking for
        if let Some(pre) = candidates.iter().find(|c| c.version().is_prerelease()) {
            let _ = write!(
                &mut hints,
                "\nif you are looking for the prerelease package it needs to be specified explicitly"
            );
            let _ = write!(
                &mut hints,
                "\n    {} = {{ version = \"{}\" }}",
                pre.name(),
                pre.version()
            );
        }

        // If we have a path dependency with a locked version, then this may
        // indicate that we updated a sub-package and forgot to run `cargo
        // update`. In this case try to print a helpful error!
        if dep.source_id().is_path() && dep.version_req().is_locked() {
            let _ = write!(
                &mut hints,
                "\nconsider running `cargo update` to update \
                          a path dependency's locked version",
            );
        }

        if registry.is_replaced(dep.source_id()) {
            let _ = write!(
                &mut hints,
                "\nperhaps a crate was updated and forgotten to be re-vendored?"
            );
        }
    } else if let Some(name_candidates) = alt_names(registry, dep) {
        let name_candidates = match name_candidates {
            Ok(c) => c,
            Err(e) => return to_resolve_err(e),
        };
        let _ = writeln!(&mut msg, "no matching package found",);
        let _ = writeln!(&mut msg, "searched package name: `{}`", dep.package_name());
        let mut names = name_candidates
            .iter()
            .take(3)
            .map(|c| c.1.name().as_str())
            .collect::<Vec<_>>();

        if name_candidates.len() > 3 {
            names.push("...");
        }
        // Vertically align first suggestion with missing crate name
        // so a typo jumps out at you.
        let suggestions =
            names
                .iter()
                .enumerate()
                .fold(String::default(), |acc, (i, el)| match i {
                    0 => acc + el,
                    i if names.len() - 1 == i && name_candidates.len() <= 3 => acc + " or " + el,
                    _ => acc + ", " + el,
                });
        let _ = writeln!(&mut msg, "perhaps you meant:      {suggestions}");
    } else {
        let _ = writeln!(
            &mut msg,
            "no matching package named `{}` found",
            dep.package_name()
        );
    }

    let mut location_searched_msg = registry.describe_source(dep.source_id());
    if location_searched_msg.is_empty() {
        location_searched_msg = format!("{}", dep.source_id());
    }
    let _ = writeln!(&mut msg, "location searched: {}", location_searched_msg);
    let _ = write!(
        &mut msg,
        "required by {}",
        describe_path_in_context(resolver_ctx, &parent.package_id()),
    );

    if let Some(gctx) = gctx {
        if let Some(offline_flag) = gctx.offline_flag() {
            let _ = write!(
                &mut hints,
                "\nAs a reminder, you're using offline mode ({offline_flag}) \
                 which can sometimes cause surprising resolution failures, \
                 if this error is too confusing you may wish to retry \
                 without `{offline_flag}`.",
            );
        }
    }

    to_resolve_err(anyhow::format_err!("{msg}{hints}"))
}

// Maybe the user mistyped the ver_req? Like `dep="2"` when `dep="0.2"`
// was meant. So we re-query the registry with `dep="*"` so we can
// list a few versions that were actually found.
fn alt_versions(
    registry: &mut dyn Registry,
    dep: &Dependency,
) -> Option<CargoResult<Vec<Summary>>> {
    let mut wild_dep = dep.clone();
    wild_dep.set_version_req(OptVersionReq::Any);

    let candidates = loop {
        match registry.query_vec(&wild_dep, QueryKind::Exact) {
            Poll::Ready(Ok(candidates)) => break candidates,
            Poll::Ready(Err(e)) => return Some(Err(e)),
            Poll::Pending => match registry.block_until_ready() {
                Ok(()) => continue,
                Err(e) => return Some(Err(e)),
            },
        }
    };
    let mut candidates: Vec<_> = candidates.into_iter().map(|s| s.into_summary()).collect();
    candidates.sort_unstable_by(|a, b| b.version().cmp(a.version()));
    if candidates.is_empty() {
        None
    } else {
        Some(Ok(candidates))
    }
}

/// Maybe something is wrong with the available versions
fn rejected_versions(
    registry: &mut dyn Registry,
    dep: &Dependency,
) -> Option<CargoResult<Vec<IndexSummary>>> {
    let mut version_candidates = loop {
        match registry.query_vec(&dep, QueryKind::RejectedVersions) {
            Poll::Ready(Ok(candidates)) => break candidates,
            Poll::Ready(Err(e)) => return Some(Err(e)),
            Poll::Pending => match registry.block_until_ready() {
                Ok(()) => continue,
                Err(e) => return Some(Err(e)),
            },
        }
    };
    version_candidates.sort_unstable_by_key(|a| a.as_summary().version().clone());
    if version_candidates.is_empty() {
        None
    } else {
        Some(Ok(version_candidates))
    }
}

/// Maybe the user mistyped the name? Like `dep-thing` when `Dep_Thing`
/// was meant. So we try asking the registry for a `fuzzy` search for suggestions.
fn alt_names(
    registry: &mut dyn Registry,
    dep: &Dependency,
) -> Option<CargoResult<Vec<(usize, Summary)>>> {
    let mut wild_dep = dep.clone();
    wild_dep.set_version_req(OptVersionReq::Any);

    let name_candidates = loop {
        match registry.query_vec(&wild_dep, QueryKind::AlternativeNames) {
            Poll::Ready(Ok(candidates)) => break candidates,
            Poll::Ready(Err(e)) => return Some(Err(e)),
            Poll::Pending => match registry.block_until_ready() {
                Ok(()) => continue,
                Err(e) => return Some(Err(e)),
            },
        }
    };
    let mut name_candidates: Vec<_> = name_candidates
        .into_iter()
        .map(|s| s.into_summary())
        .collect();
    name_candidates.sort_unstable_by_key(|a| a.name());
    name_candidates.dedup_by(|a, b| a.name() == b.name());
    let mut name_candidates: Vec<_> = name_candidates
        .into_iter()
        .filter_map(|n| Some((edit_distance(&*wild_dep.package_name(), &*n.name(), 3)?, n)))
        .collect();
    name_candidates.sort_by_key(|o| o.0);

    if name_candidates.is_empty() {
        None
    } else {
        Some(Ok(name_candidates))
    }
}

/// Returns String representation of dependency chain for a particular `pkgid`
/// within given context.
pub(super) fn describe_path_in_context(cx: &ResolverContext, id: &PackageId) -> String {
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
