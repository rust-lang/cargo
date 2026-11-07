//! Index package nodes and graph-wide counts for the metadata popup.

use crate::ops::cargo_tree::tui::TuiTree;
use crate::ops::cargo_tree::tui::dependency::NodeId;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::workspace::PackageId;

/// Cached graph lookups reused by both metadata tabs.
#[derive(Debug, Default)]
pub(super) struct GraphIndex {
    pub(super) package_nodes: FxHashMap<PackageId, NodeId>,
    pub(super) versions_by_name: FxHashMap<String, Vec<String>>,
    dependent_counts: FxHashMap<PackageId, DependentCounts>,
}
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct DependentCounts {
    pub(super) direct: usize,
    pub(super) total: usize,
}

impl GraphIndex {
    pub(super) fn new(source_tree: &TuiTree) -> Self {
        let mut index = Self::default();
        let mut pending = source_tree.roots.clone();
        let mut visited = FxHashSet::default();
        while let Some(node_index) = pending.pop() {
            if !visited.insert(node_index) {
                continue;
            }
            let Some(node) = source_tree.node(node_index) else {
                continue;
            };
            let Some(dependency) = node.as_dependency() else {
                pending.extend_from_slice(node.children());
                continue;
            };
            if let Some(package_id) = dependency.package_id {
                index.package_nodes.entry(package_id).or_insert(node_index);
                let versions = index
                    .versions_by_name
                    .entry(dependency.name.clone())
                    .or_default();
                if !versions.contains(&dependency.version) {
                    versions.push(dependency.version.clone());
                }
            }
            pending.extend_from_slice(node.children());
        }
        for versions in index.versions_by_name.values_mut() {
            versions.sort();
        }
        index
    }

    pub(super) fn dependents(&mut self, package_id: PackageId, tree: &TuiTree) -> DependentCounts {
        if let Some(counts) = self.dependent_counts.get(&package_id) {
            return *counts;
        }

        let mut direct: FxHashSet<_> = tree
            .dependency_usages
            .get(&package_id)
            .into_iter()
            .flatten()
            .map(|usage| usage.parent)
            .collect();
        direct.remove(&package_id);
        let mut total = FxHashSet::default();
        let mut pending: Vec<_> = direct.iter().copied().collect();
        while let Some(parent) = pending.pop() {
            if parent == package_id || !total.insert(parent) {
                continue;
            }
            if let Some(usages) = tree.dependency_usages.get(&parent) {
                pending.extend(usages.iter().map(|usage| usage.parent));
            }
        }

        let counts = DependentCounts {
            direct: direct.len(),
            total: total.len(),
        };
        self.dependent_counts.insert(package_id, counts);
        counts
    }
}
