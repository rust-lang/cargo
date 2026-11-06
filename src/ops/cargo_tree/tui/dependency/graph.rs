//! Build the flat dependency tree shown by the TUI.

use super::resolve::{FeatureDependency, ResolvedWorkspace};
use super::*;

pub(super) fn build_tree(
    resolved: ResolvedWorkspace,
    options: &ResolutionOptions,
) -> DependencyTree {
    let workspace_name = resolved.workspace_name.clone();
    let mut collected = collect_packages(&resolved);
    let mut parents = if resolved.show_features {
        wire_feature_edges(&resolved, &collected.pkg_index, &mut collected.nodes)
    } else {
        wire_edges(&resolved, &collected.pkg_index, &mut collected.nodes)
    };
    if matches!(
        options.display_depth,
        crate::ops::cargo_tree::DisplayDepth::Workspace
    ) {
        parents = filter_to_workspace_packages(
            &mut collected.nodes,
            &collected.pkg_index,
            &collected.roots,
            &resolved.workspace_members,
        );
    }
    let mut dependency_usages = resolved.usages;
    for usages in dependency_usages.values_mut() {
        usages.sort_by(|left, right| {
            (left.parent, left.kind, left.dependency.name_in_toml()).cmp(&(
                right.parent,
                right.kind,
                right.dependency.name_in_toml(),
            ))
        });
    }

    DependencyTree {
        workspace_name,
        parents,
        dependency_usages,
        nodes: collected.nodes,
        roots: collected.roots,
    }
}

/// The node arena with empty children, a package-to-node index, and root ids.
struct CollectedPackages {
    nodes: Vec<DependencyNode>,
    pkg_index: FxHashMap<PackageId, NodeId>,
    roots: Vec<NodeId>,
}

/// Create one [`DependencyNode::Crate`] per reachable package.
///
/// Starting from the workspace roots, walk the resolved graph and assign each
/// unique package a stable arena node id. Child links are filled in later.
fn collect_packages(resolved: &ResolvedWorkspace) -> CollectedPackages {
    let capacity = resolved.packages.len();
    let mut remaining: Vec<PackageId> = Vec::with_capacity(capacity);
    remaining.extend(resolved.root_ids.iter().copied());

    let mut nodes: Vec<DependencyNode> = Vec::with_capacity(capacity);
    let mut pkg_index: FxHashMap<PackageId, NodeId> =
        FxHashMap::with_capacity_and_hasher(capacity, Default::default());

    while let Some(package_id) = remaining.pop() {
        if pkg_index.contains_key(&package_id) {
            continue;
        }

        let Some(snapshot) = resolved.packages.get(&package_id) else {
            continue;
        };

        let node_id = NodeId(nodes.len());
        nodes.push(DependencyNode::Crate(Dependency::from(snapshot)));
        pkg_index.insert(package_id, node_id);

        if let Some(deps) = resolved.edges.get(&package_id) {
            remaining.extend(deps.iter().map(|(dep_id, _)| *dep_id));
        }
    }

    let roots = resolved
        .root_ids
        .iter()
        .filter_map(|pkg_id| pkg_index.get(pkg_id).copied())
        .collect();

    CollectedPackages {
        nodes,
        pkg_index,
        roots,
    }
}

/// Filters non-workspace package occurrences while preserving feature nodes and roots.
fn filter_to_workspace_packages(
    nodes: &mut [DependencyNode],
    package_index: &FxHashMap<PackageId, NodeId>,
    roots: &[NodeId],
    workspace_members: &FxHashSet<PackageId>,
) -> Vec<Vec<NodeId>> {
    let root_ids: FxHashSet<_> = roots.iter().copied().collect();
    let mut is_workspace_member = vec![false; nodes.len()];
    for (&package_id, &node_id) in package_index {
        is_workspace_member[node_id.0] = workspace_members.contains(&package_id);
    }

    let keep: Vec<_> = nodes
        .iter()
        .enumerate()
        .map(|(idx, node)| {
            root_ids.contains(&NodeId(idx))
                || !matches!(node, DependencyNode::Crate(_))
                || is_workspace_member[idx]
        })
        .collect();

    for node in nodes.iter_mut() {
        let children = match node {
            DependencyNode::Crate(dependency) => &mut dependency.children,
            DependencyNode::Group(group) => &mut group.children,
            DependencyNode::Feature(feature) => &mut feature.children,
        };
        children.retain(|child| keep[child.0]);
    }

    let keep_group: Vec<_> = nodes
        .iter()
        .map(|node| !matches!(node, DependencyNode::Group(group) if group.children.is_empty()))
        .collect();
    for node in nodes.iter_mut() {
        let children = match node {
            DependencyNode::Crate(dependency) => &mut dependency.children,
            DependencyNode::Group(group) => &mut group.children,
            DependencyNode::Feature(feature) => &mut feature.children,
        };
        children.retain(|child| keep_group[child.0]);
    }

    let mut parents = vec![Vec::new(); nodes.len()];
    for (parent_idx, node) in nodes.iter().enumerate() {
        for &child in node.children() {
            parents[child.0].push(NodeId(parent_idx));
        }
    }
    parents
}

/// Wire the dependency edges between the already-collected arena nodes.
///
/// Normal dependencies become direct children of the crate node.
///
/// Dev and build dependencies are grouped under synthetic
/// `[dev-dependencies]` / `[build-dependencies]` nodes.
///
/// While attaching those child links, this pass also builds the reverse
/// parent index for every node.
fn wire_edges(
    resolved: &ResolvedWorkspace,
    pkg_index: &FxHashMap<PackageId, NodeId>,
    nodes: &mut Vec<DependencyNode>,
) -> Vec<Vec<NodeId>> {
    let mut parents: Vec<Vec<NodeId>> = vec![Vec::new(); nodes.len()];

    for (pkg_id, &node_id) in pkg_index.iter() {
        let Some(edges) = resolved.edges.get(pkg_id) else {
            continue;
        };

        let mut classified = ClassifiedDeps::populate(edges, pkg_index);
        let mut children: Vec<NodeId> = Vec::with_capacity(
            classified.normal.len()
                + (!classified.dev.is_empty()) as usize  // expanded as group, so one child
                + (!classified.build.is_empty()) as usize,
        );

        // Normal deps are direct children of the crate node.
        for &child_id in &classified.normal {
            children.push(child_id);
            parents[child_id.0].push(node_id);
        }

        // Dev and build deps go under group nodes.
        for (kind, group_deps) in [
            (DepKind::Development, &mut classified.dev),
            (DepKind::Build, &mut classified.build),
        ] {
            if group_deps.is_empty() {
                continue;
            }

            let group_id = NodeId(nodes.len());
            for &child_id in group_deps.iter() {
                parents[child_id.0].push(group_id);
            }

            nodes.push(DependencyNode::Group(DependencyGroup {
                kind,
                children: std::mem::take(group_deps),
            }));

            parents.push(vec![node_id]);
            children.push(group_id);
        }

        if let Some(DependencyNode::Crate(dep)) = nodes.get_mut(node_id.0) {
            dep.children = children;
        }
    }

    parents
}

/// Wire package dependencies through feature nodes when Cargo feature edges
/// are selected. Feature nodes are keyed by the package whose feature they
/// describe, matching Cargo's tree graph identity.
fn wire_feature_edges(
    resolved: &ResolvedWorkspace,
    pkg_index: &FxHashMap<PackageId, NodeId>,
    nodes: &mut Vec<DependencyNode>,
) -> Vec<Vec<NodeId>> {
    let mut parents = vec![Vec::new(); nodes.len()];
    let mut feature_index: FxHashMap<(PackageId, String), NodeId> = FxHashMap::default();
    let mut pending_features = Vec::new();

    for (&source_package, &source_id) in pkg_index {
        let Some(edges) = resolved.edges.get(&source_package) else {
            continue;
        };
        let mut feature_deps_by_edge: FxHashMap<(PackageId, DepKind), Vec<&FeatureDependency>> =
            FxHashMap::default();
        if let Some(feature_dependencies) = resolved.graph_feature_dependencies.get(&source_package)
        {
            for dependency in feature_dependencies {
                feature_deps_by_edge
                    .entry((dependency.package_id, dependency.kind))
                    .or_default()
                    .push(dependency);
            }
        }

        let mut normal = Vec::new();
        let mut dev = Vec::new();
        let mut build = Vec::new();
        for &(target_package, kind) in edges {
            let target_id = match pkg_index.get(&target_package) {
                Some(id) => *id,
                None => continue,
            };
            let feature_deps = feature_deps_by_edge
                .get(&(target_package, kind))
                .into_iter()
                .flatten();

            let mut has_direct_edge = false;
            let mut has_dependency_metadata = false;
            for dependency in feature_deps {
                has_dependency_metadata = true;
                let mut requested_features = dependency.features.clone();
                if dependency.uses_default_features {
                    requested_features.push("default".to_owned());
                }
                requested_features.sort_unstable();
                requested_features.dedup();
                if requested_features.is_empty() {
                    has_direct_edge = true;
                }
                for feature_name in requested_features {
                    let feature_owner = if resolved.inverted {
                        source_package
                    } else {
                        target_package
                    };
                    let feature_id = ensure_feature_node(
                        feature_owner,
                        feature_name,
                        Some(target_id),
                        nodes,
                        &mut parents,
                        &mut feature_index,
                        &mut pending_features,
                    );
                    match kind {
                        DepKind::Normal => normal.push(feature_id),
                        DepKind::Development => dev.push(feature_id),
                        DepKind::Build => build.push(feature_id),
                    }
                }
            }

            if has_direct_edge || !has_dependency_metadata {
                match kind {
                    DepKind::Normal => normal.push(target_id),
                    DepKind::Development => dev.push(target_id),
                    DepKind::Build => build.push(target_id),
                }
            }
        }

        normal.sort_unstable_by_key(|id| id.0);
        normal.dedup();
        dev.sort_unstable_by_key(|id| id.0);
        dev.dedup();
        build.sort_unstable_by_key(|id| id.0);
        build.dedup();
        for &child_id in &normal {
            parents[child_id.0].push(source_id);
        }
        let mut children = normal;
        for (kind, group_children) in [
            (DepKind::Development, &mut dev),
            (DepKind::Build, &mut build),
        ] {
            if group_children.is_empty() {
                continue;
            }
            let group_id = NodeId(nodes.len());
            for &child in group_children.iter() {
                parents[child.0].push(group_id);
            }
            nodes.push(DependencyNode::Group(DependencyGroup {
                kind,
                children: std::mem::take(group_children),
            }));
            parents.push(vec![source_id]);
            children.push(group_id);
        }
        if let Some(DependencyNode::Crate(package)) = nodes.get_mut(source_id.0) {
            package.children = children;
        }
    }

    if resolved.show_features {
        if resolved.inverted {
            add_cli_feature_edges(
                resolved,
                pkg_index,
                nodes,
                &mut parents,
                &mut feature_index,
                &mut pending_features,
            );
        }
        expand_internal_features(
            resolved,
            pkg_index,
            nodes,
            &mut parents,
            &mut feature_index,
            &mut pending_features,
            resolved.inverted,
        );
    }
    parents
}

#[allow(clippy::too_many_arguments)]
fn ensure_feature_node(
    owner: PackageId,
    name: String,
    package_child: Option<NodeId>,
    nodes: &mut Vec<DependencyNode>,
    parents: &mut Vec<Vec<NodeId>>,
    feature_index: &mut FxHashMap<(PackageId, String), NodeId>,
    pending: &mut Vec<(PackageId, String, NodeId)>,
) -> NodeId {
    let key = (owner, name.clone());
    let feature_id = if let Some(&id) = feature_index.get(&key) {
        id
    } else {
        let id = NodeId(nodes.len());
        nodes.push(DependencyNode::Feature(DependencyFeature {
            package_name: owner.name().as_str().to_owned(),
            name: name.clone(),
            command_line: false,
            children: Vec::new(),
        }));
        parents.push(Vec::new());
        feature_index.insert(key, id);
        pending.push((owner, name, id));
        id
    };

    if let Some(package_child) = package_child {
        let children = match &mut nodes[feature_id.0] {
            DependencyNode::Feature(feature) => &mut feature.children,
            _ => unreachable!("feature index only contains feature nodes"),
        };
        if !children.contains(&package_child) {
            children.push(package_child);
            parents[package_child.0].push(feature_id);
        }
    }
    feature_id
}

fn add_cli_feature_edges(
    resolved: &ResolvedWorkspace,
    pkg_index: &FxHashMap<PackageId, NodeId>,
    nodes: &mut Vec<DependencyNode>,
    parents: &mut Vec<Vec<NodeId>>,
    feature_index: &mut FxHashMap<(PackageId, String), NodeId>,
    pending: &mut Vec<(PackageId, String, NodeId)>,
) {
    for (&package_id, cli_features) in &resolved.cli_features {
        if !pkg_index.contains_key(&package_id) {
            continue;
        }

        let mut requested_features = Vec::new();
        if cli_features.all_features
            && let Some(features) = resolved.feature_map.get(&package_id)
        {
            requested_features.extend(features.keys().cloned());
        }
        if cli_features.uses_default_features {
            requested_features.push("default".to_owned());
        }
        let mut dependency_features = Vec::new();
        for feature in cli_features.features.iter() {
            match feature {
                FeatureValue::Feature(name) => requested_features.push(name.to_string()),
                FeatureValue::DepFeature { .. } => dependency_features.push(feature),
                FeatureValue::Dep { .. } => {}
            }
        }
        requested_features.sort_unstable();
        requested_features.dedup();

        for name in requested_features {
            let feature_id = ensure_feature_node(
                package_id,
                name,
                None,
                nodes,
                parents,
                feature_index,
                pending,
            );
            mark_command_line_feature(feature_id, nodes);
            add_package_feature_parent(package_id, feature_id, pkg_index, nodes, parents);
        }

        let Some(dependencies) = resolved.feature_dependencies.get(&package_id) else {
            continue;
        };
        for feature in dependency_features {
            let FeatureValue::DepFeature {
                dep_name,
                dep_feature,
                weak,
            } = feature
            else {
                unreachable!("only dependency features were collected")
            };
            for dependency in dependencies
                .iter()
                .filter(|dependency| dependency.name == dep_name.as_str())
            {
                if !pkg_index.contains_key(&dependency.package_id) {
                    continue;
                }
                if dependency.optional && !weak {
                    let feature_id = ensure_feature_node(
                        package_id,
                        dep_name.to_string(),
                        None,
                        nodes,
                        parents,
                        feature_index,
                        pending,
                    );
                    mark_command_line_feature(feature_id, nodes);
                    add_package_feature_parent(package_id, feature_id, pkg_index, nodes, parents);
                }

                let feature_id = ensure_feature_node(
                    dependency.package_id,
                    dep_feature.to_string(),
                    None,
                    nodes,
                    parents,
                    feature_index,
                    pending,
                );
                mark_command_line_feature(feature_id, nodes);
                add_package_feature_parent(
                    dependency.package_id,
                    feature_id,
                    pkg_index,
                    nodes,
                    parents,
                );
            }
        }
    }
}

fn mark_command_line_feature(feature_id: NodeId, nodes: &mut [DependencyNode]) {
    if let Some(DependencyNode::Feature(feature)) = nodes.get_mut(feature_id.0) {
        feature.command_line = true;
    }
}

fn expand_internal_features(
    resolved: &ResolvedWorkspace,
    pkg_index: &FxHashMap<PackageId, NodeId>,
    nodes: &mut Vec<DependencyNode>,
    parents: &mut Vec<Vec<NodeId>>,
    feature_index: &mut FxHashMap<(PackageId, String), NodeId>,
    pending: &mut Vec<(PackageId, String, NodeId)>,
    inverted: bool,
) {
    let mut processed = FxHashSet::default();
    while let Some((package_id, feature_name, from_id)) = pending.pop() {
        if !processed.insert((package_id, feature_name.clone())) {
            continue;
        }
        let Some(values) = resolved
            .feature_map
            .get(&package_id)
            .and_then(|features| features.get(&feature_name))
        else {
            continue;
        };

        for value in values {
            match value {
                FeatureValue::Feature(name) => {
                    let Some(&package_child) = pkg_index.get(&package_id) else {
                        continue;
                    };
                    let child_id = ensure_feature_node(
                        package_id,
                        name.to_string(),
                        (!inverted).then_some(package_child),
                        nodes,
                        parents,
                        feature_index,
                        pending,
                    );
                    if inverted {
                        add_package_feature_parent(package_id, child_id, pkg_index, nodes, parents);
                        add_feature_child(child_id, from_id, nodes, parents);
                    } else {
                        add_feature_child(from_id, child_id, nodes, parents);
                    }
                }
                FeatureValue::Dep { .. } => {}
                FeatureValue::DepFeature {
                    dep_name,
                    dep_feature,
                    weak,
                } => {
                    let Some(dependencies) = resolved.feature_dependencies.get(&package_id) else {
                        continue;
                    };
                    for dependency in dependencies
                        .iter()
                        .filter(|dependency| dependency.name == dep_name.as_str())
                    {
                        if dependency.optional && !weak {
                            let Some(&package_child) = pkg_index.get(&package_id) else {
                                continue;
                            };
                            let dep_feature_id = ensure_feature_node(
                                package_id,
                                dep_name.to_string(),
                                (!inverted).then_some(package_child),
                                nodes,
                                parents,
                                feature_index,
                                pending,
                            );
                            if inverted {
                                add_package_feature_parent(
                                    package_id,
                                    dep_feature_id,
                                    pkg_index,
                                    nodes,
                                    parents,
                                );
                                add_feature_child(dep_feature_id, from_id, nodes, parents);
                            } else {
                                add_feature_child(from_id, dep_feature_id, nodes, parents);
                            }
                        }

                        let package_child = if inverted {
                            pkg_index.get(&package_id)
                        } else {
                            pkg_index.get(&dependency.package_id)
                        };
                        let Some(&package_child) = package_child else {
                            continue;
                        };
                        let dep_feature_id = ensure_feature_node(
                            dependency.package_id,
                            dep_feature.to_string(),
                            (!inverted).then_some(package_child),
                            nodes,
                            parents,
                            feature_index,
                            pending,
                        );
                        if inverted {
                            add_package_feature_parent(
                                dependency.package_id,
                                dep_feature_id,
                                pkg_index,
                                nodes,
                                parents,
                            );
                            add_feature_child(dep_feature_id, from_id, nodes, parents);
                        } else {
                            add_feature_child(from_id, dep_feature_id, nodes, parents);
                        }
                    }
                }
            }
        }
    }
}

fn add_package_feature_parent(
    package_id: PackageId,
    feature_id: NodeId,
    pkg_index: &FxHashMap<PackageId, NodeId>,
    nodes: &mut [DependencyNode],
    parents: &mut [Vec<NodeId>],
) {
    let Some(&package_node) = pkg_index.get(&package_id) else {
        return;
    };
    let children = match &mut nodes[package_node.0] {
        DependencyNode::Crate(package) => &mut package.children,
        _ => return,
    };
    if !children.contains(&feature_id) {
        children.push(feature_id);
        parents[feature_id.0].push(package_node);
    }
}

fn add_feature_child(
    from: NodeId,
    child: NodeId,
    nodes: &mut [DependencyNode],
    parents: &mut [Vec<NodeId>],
) {
    let children = match &mut nodes[from.0] {
        DependencyNode::Feature(feature) => &mut feature.children,
        _ => unreachable!("feature edges originate at feature nodes"),
    };
    if !children.contains(&child) {
        children.push(child);
        parents[child.0].push(from);
    }
}

#[derive(Default)]
struct ClassifiedDeps {
    normal: Vec<NodeId>,
    dev: Vec<NodeId>,
    build: Vec<NodeId>,
}

impl ClassifiedDeps {
    /// Classify a package's edges into normal, dev, and build buckets.
    fn populate(edges: &[(PackageId, DepKind)], pkg_index: &FxHashMap<PackageId, NodeId>) -> Self {
        let mut classified = ClassifiedDeps::default();

        for &(dep_id, kind) in edges {
            let Some(&child_id) = pkg_index.get(&dep_id) else {
                continue;
            };

            match kind {
                DepKind::Normal => classified.normal.push(child_id),
                DepKind::Development => classified.dev.push(child_id),
                DepKind::Build => classified.build.push(child_id),
            }
        }

        classified
    }
}
