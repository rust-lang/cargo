//! Reconstruct a Cargo [`Resolve`] from a PubGrub solution.
//!
//! PubGrub returns a [`SelectedDependencies`] mapping each [`PubGrubPackage`] to
//! the version it selected. We project that back onto Cargo's model:
//!
//! * concrete [`PubGrubPackage::Bucket`] packages become the resolved
//!   [`PackageId`]s and graph nodes;
//! * the feature/default-feature packages tell us which features each package
//!   ended up with;
//! * graph edges are recovered by walking each resolved summary's
//!   dependencies, keeping the ones that the feature solution activated, and
//!   linking them to the selected child version.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use semver::Version;

use pubgrub::SelectedDependencies;

use crate::core::dependency::DepKind;
use crate::core::resolver::Resolve;
use crate::core::resolver::ResolveVersion;
use crate::core::{Dependency, PackageId, Registry, SourceId};
use crate::util::Graph;
use crate::util::errors::CargoResult;
use crate::util::interning::{INTERNED_DEFAULT, InternedString};

use super::package::{FeatureNamespace, PubGrubPackage};
use super::provider::Provider;
use super::semver_pubgrub::SemverCompatibility;

/// Per-package activation facts gathered from the PubGrub solution.
#[derive(Default)]
struct Activation {
    /// Activated named features (including `default`).
    features: BTreeSet<InternedString>,
    /// Activated optional dependencies (the toml names that appeared as
    /// `BucketFeatures{.., Dep(name)}` in the solution).
    deps: HashSet<InternedString>,
    /// Whether this package was resolved as a workspace member (dev-deps).
    member: bool,
}

pub(super) fn into_resolve<T: Registry>(
    provider: &Provider<'_, T>,
    solution: &SelectedDependencies<PubGrubPackage, Version>,
    resolve_version: ResolveVersion,
) -> CargoResult<Resolve> {
    // (name, source) -> selected versions (one per compatibility bucket).
    let mut selected: HashMap<(InternedString, SourceId), BTreeSet<Version>> = HashMap::new();
    // PackageId -> activation facts.
    let mut activations: HashMap<PackageId, Activation> = HashMap::new();
    let mut package_ids: BTreeSet<PackageId> = BTreeSet::new();

    for (pkg, version) in solution.iter() {
        match pkg {
            PubGrubPackage::Bucket { name, member, all_features: _ } => {
                let pid = PackageId::new(name.name, version.clone(), name.source);
                package_ids.insert(pid);
                selected
                    .entry((name.name, name.source))
                    .or_default()
                    .insert(version.clone());
                let act = activations.entry(pid).or_default();
                act.member |= *member;
            }
            PubGrubPackage::BucketFeatures { name, feature } => {
                let pid = PackageId::new(name.name, version.clone(), name.source);
                let act = activations.entry(pid).or_default();
                match feature {
                    FeatureNamespace::Feat(f) => {
                        act.features.insert(*f);
                    }
                    // Optional-dependency activations don't contribute to the
                    // user-facing feature list, but do gate optional edges.
                    FeatureNamespace::Dep(d) => {
                        act.deps.insert(*d);
                    }
                }
            }
            PubGrubPackage::BucketDefaultFeatures { name } => {
                let pid = PackageId::new(name.name, version.clone(), name.source);
                activations
                    .entry(pid)
                    .or_default()
                    .features
                    .insert(INTERNED_DEFAULT);
            }
            // Wide/links/root packages are not real graph nodes.
            PubGrubPackage::Root
            | PubGrubPackage::Wide { .. }
            | PubGrubPackage::WideFeatures { .. }
            | PubGrubPackage::WideDefaultFeatures { .. }
            | PubGrubPackage::Links { .. } => {}
        }
    }

    // Build the dependency graph.
    let mut graph: Graph<PackageId, HashSet<Dependency>> = Graph::new();
    for pid in &package_ids {
        graph.add(*pid);
    }

    for pid in &package_ids {
        let Some(summary) = provider.summary_for(pid.name(), pid.source_id(), pid.version())? else {
            anyhow::bail!("pubgrub selected `{pid}` but it has no summary");
        };
        let act = activations.get(pid);
        let member = act.is_some_and(|a| a.member);
        for dep in summary.dependencies() {
            // Determine whether this dependency is part of the resolved graph:
            //
            // * dev-dependencies are only recorded for workspace members;
            // * optional dependencies are recorded only when activated (some
            //   feature turned them on), so that unactivated optional deps do
            //   not introduce spurious edges (and cycles);
            // * all other dependencies are always recorded.
            let active = match dep.kind() {
                DepKind::Development => member,
                _ => {
                    !dep.is_optional()
                        || act.is_some_and(|a| a.deps.contains(&dep.name_in_toml()))
                }
            };
            if !active {
                continue;
            }
            let Some(child) = resolve_child(provider, dep, pid, solution, &selected) else {
                // An active dependency with no resolved child indicates a bug
                // in the encoding rather than a benign skip.
                anyhow::bail!(
                    "pubgrub could not map dependency `{}` of `{pid}` to a resolved package",
                    dep.package_name()
                );
            };
            graph.link(*pid, child).insert(dep.clone());
        }
    }

    // Checksums, features and replacements.
    let mut cksums = HashMap::new();
    let mut features: HashMap<PackageId, Vec<InternedString>> = HashMap::new();
    let mut summaries = HashMap::new();
    let mut replacements = HashMap::new();
    {
        let registry = provider.registry();
        for pid in &package_ids {
            let summary = provider
                .summary_for(pid.name(), pid.source_id(), pid.version())?
                .expect("summary present");
            cksums.insert(*pid, summary.checksum().map(|s| s.to_string()));
            summaries.insert(*pid, summary);
            if let Some((from, to)) = registry.used_replacement_for(*pid) {
                replacements.insert(from, to);
            }
            if let Some(act) = activations.get(pid) {
                let mut feats: Vec<InternedString> = act.features.iter().copied().collect();
                feats.sort_unstable();
                features.insert(*pid, feats);
            }
        }
    }

    let resolve = Resolve::new(
        graph,
        replacements,
        features,
        cksums,
        BTreeMap::new(),
        Vec::new(),
        resolve_version,
        summaries,
    );

    super::super::check_cycles(&resolve)?;
    super::super::check_duplicate_pkgs_in_lockfile(&resolve)?;
    Ok(resolve)
}

/// Find the resolved child [`PackageId`] that satisfies `dep` from `parent`.
fn resolve_child<T: Registry>(
    provider: &Provider<'_, T>,
    dep: &Dependency,
    parent: &PackageId,
    solution: &SelectedDependencies<PubGrubPackage, Version>,
    selected: &HashMap<(InternedString, SourceId), BTreeSet<Version>>,
) -> Option<PackageId> {
    let (cray, _) = provider.from_dep(dep, parent.name(), parent.version());
    let (name, source, compat) = match cray {
        PubGrubPackage::Bucket { ref name, .. } => (name.name, name.source, name.compat),
        PubGrubPackage::Wide { ref name } => {
            // The wide package chose a bucket; read it from the solution.
            let chosen = solution.get(&cray)?;
            (name.name, name.source, SemverCompatibility::from(chosen))
        }
        _ => return None,
    };
    let versions = selected.get(&(name, source))?;
    versions
        .iter()
        .find(|v| SemverCompatibility::from(*v) == compat)
        .map(|v| PackageId::new(name, v.clone(), source))
}
