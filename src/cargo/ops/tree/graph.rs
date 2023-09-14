//! Code for building the graph used by `cargo tree`.

use super::TreeOptions;
use crate::core::compiler::{CompileKind, RustcTargetData};
use crate::core::dependency::DepKind;
use crate::core::resolver::features::{CliFeatures, FeaturesFor, ResolvedFeatures};
use crate::core::resolver::Resolve;
use crate::core::{FeatureMap, FeatureValue, Package, PackageId, PackageIdSpec, Workspace};
use crate::util::interning::InternedString;
use crate::util::CargoResult;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum Node {
    Package {
        package_id: PackageId,
        /// Features that are enabled on this package.
        features: Vec<InternedString>,
        kind: CompileKind,
    },
    Feature {
        /// Index of the package node this feature is for.
        node_index: usize,
        /// Name of the feature.
        name: InternedString,
    },
}

/// The kind of edge, for separating dependencies into different sections.
#[derive(Debug, Copy, Hash, Eq, Clone, PartialEq)]
pub enum EdgeKind {
    Dep(DepKind),
    Feature,
}

/// Set of outgoing edges for a single node.
///
/// Edges are separated by the edge kind (`DepKind` or `Feature`). This is
/// primarily done so that the output can easily display separate sections
/// like `[build-dependencies]`.
///
/// The value is a `Vec` because each edge kind can have multiple outgoing
/// edges. For example, package "foo" can have multiple normal dependencies.
#[derive(Clone)]
struct Edges(HashMap<EdgeKind, Vec<usize>>);

impl Edges {
    fn new() -> Edges {
        Edges(HashMap::new())
    }

    /// Adds an edge pointing to the given node.
    fn add_edge(&mut self, kind: EdgeKind, index: usize) {
        let indexes = self.0.entry(kind).or_default();
        if !indexes.contains(&index) {
            indexes.push(index)
        }
    }
}

/// A graph of dependencies.
pub struct Graph<'a> {
    nodes: Vec<Node>,
    /// The indexes of `edges` correspond to the `nodes`. That is, `edges[0]`
    /// is the set of outgoing edges for `nodes[0]`. They should always be in
    /// sync.
    edges: Vec<Edges>,
    /// Index maps a node to an index, for fast lookup.
    index: HashMap<Node, usize>,
    /// Map for looking up packages.
    package_map: HashMap<PackageId, &'a Package>,
    /// Set of indexes of feature nodes that were added via the command-line.
    ///
    /// For example `--features foo` will mark the "foo" node here.
    cli_features: HashSet<usize>,
    /// Map of dependency names, used for building internal feature map for
    /// dep_name/feat_name syntax.
    ///
    /// Key is the index of a package node, value is a map of dep_name to a
    /// set of `(pkg_node_index, is_optional)`.
    dep_name_map: HashMap<usize, HashMap<InternedString, HashSet<(usize, bool)>>>,
}

impl<'a> Graph<'a> {
    fn new(package_map: HashMap<PackageId, &'a Package>) -> Graph<'a> {
        Graph {
            nodes: Vec::new(),
            edges: Vec::new(),
            index: HashMap::new(),
            package_map,
            cli_features: HashSet::new(),
            dep_name_map: HashMap::new(),
        }
    }

    /// Adds a new node to the graph, returning its new index.
    fn add_node(&mut self, node: Node) -> usize {
        let from_index = self.nodes.len();
        self.nodes.push(node);
        self.edges.push(Edges::new());
        self.index
            .insert(self.nodes[from_index].clone(), from_index);
        from_index
    }

    /// Returns a list of nodes the given node index points to for the given kind.
    pub fn connected_nodes(&self, from: usize, kind: &EdgeKind) -> Vec<usize> {
        match self.edges[from].0.get(kind) {
            Some(indexes) => {
                // Created a sorted list for consistent output.
                let mut indexes = indexes.clone();
                indexes.sort_unstable_by(|a, b| self.nodes[*a].cmp(&self.nodes[*b]));
                indexes
            }
            None => Vec::new(),
        }
    }

    /// Returns `true` if the given node has any outgoing edges.
    pub fn has_outgoing_edges(&self, index: usize) -> bool {
        !self.edges[index].0.is_empty()
    }

    /// Gets a node by index.
    pub fn node(&self, index: usize) -> &Node {
        &self.nodes[index]
    }

    /// Given a slice of PackageIds, returns the indexes of all nodes that match.
    pub fn indexes_from_ids(&self, package_ids: &[PackageId]) -> Vec<usize> {
        let mut result: Vec<(&Node, usize)> = self
            .nodes
            .iter()
            .enumerate()
            .filter(|(_i, node)| match node {
                Node::Package { package_id, .. } => package_ids.contains(package_id),
                _ => false,
            })
            .map(|(i, node)| (node, i))
            .collect();
        // Sort for consistent output (the same command should always return
        // the same output). "unstable" since nodes should always be unique.
        result.sort_unstable();
        result.into_iter().map(|(_node, i)| i).collect()
    }

    pub fn package_for_id(&self, id: PackageId) -> &Package {
        self.package_map[&id]
    }

    fn package_id_for_index(&self, index: usize) -> PackageId {
        match self.nodes[index] {
            Node::Package { package_id, .. } => package_id,
            Node::Feature { .. } => panic!("unexpected feature node"),
        }
    }

    /// Returns `true` if the given feature node index is a feature enabled
    /// via the command-line.
    pub fn is_cli_feature(&self, index: usize) -> bool {
        self.cli_features.contains(&index)
    }

    /// Returns a new graph by removing all nodes not reachable from the
    /// given nodes.
    pub fn from_reachable(&self, roots: &[usize]) -> Graph<'a> {
        // Graph built with features does not (yet) support --duplicates.
        assert!(self.dep_name_map.is_empty());
        let mut new_graph = Graph::new(self.package_map.clone());
        // Maps old index to new index. None if not yet visited.
        let mut remap: Vec<Option<usize>> = vec![None; self.nodes.len()];

        fn visit(
            graph: &Graph<'_>,
            new_graph: &mut Graph<'_>,
            remap: &mut Vec<Option<usize>>,
            index: usize,
        ) -> usize {
            if let Some(new_index) = remap[index] {
                // Already visited.
                return new_index;
            }
            let node = graph.node(index).clone();
            let new_from = new_graph.add_node(node);
            remap[index] = Some(new_from);
            // Visit dependencies.
            for (edge_kind, edge_indexes) in &graph.edges[index].0 {
                for edge_index in edge_indexes {
                    let new_to_index = visit(graph, new_graph, remap, *edge_index);
                    new_graph.edges[new_from].add_edge(*edge_kind, new_to_index);
                }
            }
            new_from
        }

        // Walk the roots, generating a new graph as it goes along.
        for root in roots {
            visit(self, &mut new_graph, &mut remap, *root);
        }

        new_graph
    }

    /// Inverts the direction of all edges.
    pub fn invert(&mut self) {
        let mut new_edges = vec![Edges::new(); self.edges.len()];
        for (from_idx, node_edges) in self.edges.iter().enumerate() {
            for (kind, edges) in &node_edges.0 {
                for edge_idx in edges {
                    new_edges[*edge_idx].add_edge(*kind, from_idx);
                }
            }
        }
        self.edges = new_edges;
    }

    /// Returns a list of nodes that are considered "duplicates" (same package
    /// name, with different versions/features/source/etc.).
    pub fn find_duplicates(&self) -> Vec<usize> {
        // Graph built with features does not (yet) support --duplicates.
        assert!(self.dep_name_map.is_empty());

        // Collect a map of package name to Vec<(&Node, usize)>.
        let mut packages = HashMap::new();
        for (i, node) in self.nodes.iter().enumerate() {
            if let Node::Package { package_id, .. } = node {
                packages
                    .entry(package_id.name())
                    .or_insert_with(Vec::new)
                    .push((node, i));
            }
        }

        let mut dupes: Vec<(&Node, usize)> = packages
            .into_iter()
            .filter(|(_name, indexes)| {
                indexes
                    .into_iter()
                    .map(|(node, _)| {
                        match node {
                            Node::Package {
                                package_id,
                                features,
                                ..
                            } => {
                                // Do not treat duplicates on the host or target as duplicates.
                                Node::Package {
                                    package_id: package_id.clone(),
                                    features: features.clone(),
                                    kind: CompileKind::Host,
                                }
                            }
                            _ => unreachable!(),
                        }
                    })
                    .collect::<HashSet<_>>()
                    .len()
                    > 1
            })
            .flat_map(|(_name, indexes)| indexes)
            .collect();

        // For consistent output.
        dupes.sort_unstable();
        dupes.into_iter().map(|(_node, i)| i).collect()
    }
}

/// Builds the graph.
pub fn build<'a>(
    ws: &Workspace<'_>,
    resolve: &Resolve,
    resolved_features: &ResolvedFeatures,
    specs: &[PackageIdSpec],
    cli_features: &CliFeatures,
    target_data: &RustcTargetData<'_>,
    requested_kinds: &[CompileKind],
    package_map: HashMap<PackageId, &'a Package>,
    opts: &TreeOptions,
) -> CargoResult<Graph<'a>> {
    let mut graph = Graph::new(package_map);
    let mut members_with_features = ws.members_with_features(specs, cli_features)?;
    members_with_features.sort_unstable_by_key(|e| e.0.package_id());
    for (member, cli_features) in members_with_features {
        let member_id = member.package_id();
        let features_for = FeaturesFor::from_for_host(member.proc_macro());
        for kind in requested_kinds {
            let member_index = add_pkg(
                &mut graph,
                resolve,
                resolved_features,
                member_id,
                features_for,
                target_data,
                *kind,
                opts,
            );
            if opts.graph_features {
                let fmap = resolve.summary(member_id).features();
                add_cli_features(&mut graph, member_index, &cli_features, fmap);
            }
        }
    }
    if opts.graph_features {
        add_internal_features(&mut graph, resolve);
    }
    Ok(graph)
}

/// Adds a single package node (if it does not already exist).
///
/// This will also recursively add all of its dependencies.
///
/// Returns the index to the package node.
fn add_pkg(
    graph: &mut Graph<'_>,
    resolve: &Resolve,
    resolved_features: &ResolvedFeatures,
    package_id: PackageId,
    features_for: FeaturesFor,
    target_data: &RustcTargetData<'_>,
    requested_kind: CompileKind,
    opts: &TreeOptions,
) -> usize {
    let node_features = resolved_features.activated_features(package_id, features_for);
    let node_kind = match features_for {
        FeaturesFor::HostDep => CompileKind::Host,
        FeaturesFor::ArtifactDep(target) => CompileKind::Target(target),
        FeaturesFor::NormalOrDev => requested_kind,
    };
    let node = Node::Package {
        package_id,
        features: node_features,
        kind: node_kind,
    };
    if let Some(idx) = graph.index.get(&node) {
        return *idx;
    }
    let from_index = graph.add_node(node);
    // Compute the dep name map which is later used for foo/bar feature lookups.
    let mut dep_name_map: HashMap<InternedString, HashSet<(usize, bool)>> = HashMap::new();
    let mut deps: Vec<_> = resolve.deps(package_id).collect();
    deps.sort_unstable_by_key(|(dep_id, _)| *dep_id);
    let show_all_targets = opts.target == super::Target::All;
    for (dep_id, deps) in deps {
        let mut deps: Vec<_> = deps
            .iter()
            // This filter is *similar* to the one found in `unit_dependencies::compute_deps`.
            // Try to keep them in sync!
            .filter(|dep| {
                let kind = match (node_kind, dep.kind()) {
                    (CompileKind::Host, _) => CompileKind::Host,
                    (_, DepKind::Build) => CompileKind::Host,
                    (_, DepKind::Normal) => node_kind,
                    (_, DepKind::Development) => node_kind,
                };
                // Filter out inactivated targets.
                if !show_all_targets && !target_data.dep_platform_activated(dep, kind) {
                    return false;
                }
                // Filter out dev-dependencies if requested.
                if !opts.edge_kinds.contains(&EdgeKind::Dep(dep.kind())) {
                    return false;
                }
                // Filter out proc-macrcos if requested.
                if opts.no_proc_macro && graph.package_for_id(dep_id).proc_macro() {
                    return false;
                }
                if dep.is_optional() {
                    // If the new feature resolver does not enable this
                    // optional dep, then don't use it.
                    if !resolved_features.is_dep_activated(
                        package_id,
                        features_for,
                        dep.name_in_toml(),
                    ) {
                        return false;
                    }
                }
                true
            })
            .collect();

        // This dependency is eliminated from the dependency tree under
        // the current target and feature set.
        if deps.is_empty() {
            continue;
        }

        deps.sort_unstable_by_key(|dep| dep.name_in_toml());
        let dep_pkg = graph.package_map[&dep_id];

        for dep in deps {
            let dep_features_for = if dep.is_build() || dep_pkg.proc_macro() {
                FeaturesFor::HostDep
            } else {
                features_for
            };
            let dep_index = add_pkg(
                graph,
                resolve,
                resolved_features,
                dep_id,
                dep_features_for,
                target_data,
                requested_kind,
                opts,
            );
            if opts.graph_features {
                // Add the dependency node with feature nodes in-between.
                dep_name_map
                    .entry(dep.name_in_toml())
                    .or_default()
                    .insert((dep_index, dep.is_optional()));
                if dep.uses_default_features() {
                    add_feature(
                        graph,
                        InternedString::new("default"),
                        Some(from_index),
                        dep_index,
                        EdgeKind::Dep(dep.kind()),
                    );
                }
                for feature in dep.features().iter() {
                    add_feature(
                        graph,
                        *feature,
                        Some(from_index),
                        dep_index,
                        EdgeKind::Dep(dep.kind()),
                    );
                }
                if !dep.uses_default_features() && dep.features().is_empty() {
                    // No features, use a direct connection.
                    graph.edges[from_index].add_edge(EdgeKind::Dep(dep.kind()), dep_index);
                }
            } else {
                graph.edges[from_index].add_edge(EdgeKind::Dep(dep.kind()), dep_index);
            }
        }
    }
    if opts.graph_features {
        assert!(graph
            .dep_name_map
            .insert(from_index, dep_name_map)
            .is_none());
    }

    from_index
}

/// Adds a feature node between two nodes.
///
/// That is, it adds the following:
///
/// ```text
/// from -Edge-> featname -Edge::Feature-> to
/// ```
///
/// Returns a tuple `(missing, index)`.
/// `missing` is true if this feature edge was already added.
/// `index` is the index of the index in the graph of the `Feature` node.
fn add_feature(
    graph: &mut Graph<'_>,
    name: InternedString,
    from: Option<usize>,
    to: usize,
    kind: EdgeKind,
) -> (bool, usize) {
    // `to` *must* point to a package node.
    assert!(matches! {graph.nodes[to], Node::Package{..}});
    let node = Node::Feature {
        node_index: to,
        name,
    };
    let (missing, node_index) = match graph.index.get(&node) {
        Some(idx) => (false, *idx),
        None => (true, graph.add_node(node)),
    };
    if let Some(from) = from {
        graph.edges[from].add_edge(kind, node_index);
    }
    graph.edges[node_index].add_edge(EdgeKind::Feature, to);
    (missing, node_index)
}

/// Adds nodes for features requested on the command-line for the given member.
///
/// Feature nodes are added as "roots" (i.e., they have no "from" index),
/// because they come from the outside world. They usually only appear with
/// `--invert`.
fn add_cli_features(
    graph: &mut Graph<'_>,
    package_index: usize,
    cli_features: &CliFeatures,
    feature_map: &FeatureMap,
) {
    // NOTE: Recursive enabling of features will be handled by
    // add_internal_features.

    // Create a set of feature names requested on the command-line.
    let mut to_add: HashSet<FeatureValue> = HashSet::new();
    if cli_features.all_features {
        to_add.extend(feature_map.keys().map(|feat| FeatureValue::Feature(*feat)));
    }

    if cli_features.uses_default_features {
        to_add.insert(FeatureValue::Feature(InternedString::new("default")));
    }
    to_add.extend(cli_features.features.iter().cloned());

    // Add each feature as a node, and mark as "from command-line" in graph.cli_features.
    for fv in to_add {
        match fv {
            FeatureValue::Feature(feature) => {
                let index = add_feature(graph, feature, None, package_index, EdgeKind::Feature).1;
                graph.cli_features.insert(index);
            }
            // This is enforced by CliFeatures.
            FeatureValue::Dep { .. } => panic!("unexpected cli dep feature {}", fv),
            FeatureValue::DepFeature {
                dep_name,
                dep_feature,
                weak,
            } => {
                let dep_connections = match graph.dep_name_map[&package_index].get(&dep_name) {
                    // Clone to deal with immutable borrow of `graph`. :(
                    Some(dep_connections) => dep_connections.clone(),
                    None => {
                        // --features bar?/feat where `bar` is not activated should be ignored.
                        // If this wasn't weak, then this is a bug.
                        if weak {
                            continue;
                        }
                        panic!(
                            "missing dep graph connection for CLI feature `{}` for member {:?}\n\
                             Please file a bug report at https://github.com/rust-lang/cargo/issues",
                            fv,
                            graph.nodes.get(package_index)
                        );
                    }
                };
                for (dep_index, is_optional) in dep_connections {
                    if is_optional {
                        // Activate the optional dep on self.
                        let index =
                            add_feature(graph, dep_name, None, package_index, EdgeKind::Feature).1;
                        graph.cli_features.insert(index);
                    }
                    let index =
                        add_feature(graph, dep_feature, None, dep_index, EdgeKind::Feature).1;
                    graph.cli_features.insert(index);
                }
            }
        }
    }
}

/// Recursively adds connections between features in the `[features]` table
/// for every package.
fn add_internal_features(graph: &mut Graph<'_>, resolve: &Resolve) {
    // Collect features already activated by dependencies or command-line.
    let feature_nodes: Vec<(PackageId, usize, usize, InternedString)> = graph
        .nodes
        .iter()
        .enumerate()
        .filter_map(|(i, node)| match node {
            Node::Package { .. } => None,
            Node::Feature { node_index, name } => {
                let package_id = graph.package_id_for_index(*node_index);
                Some((package_id, *node_index, i, *name))
            }
        })
        .collect();

    for (package_id, package_index, feature_index, feature_name) in feature_nodes {
        add_feature_rec(
            graph,
            resolve,
            feature_name,
            package_id,
            feature_index,
            package_index,
        );
    }
}

/// Recursively add feature nodes for all features enabled by the given feature.
///
/// `from` is the index of the node that enables this feature.
/// `package_index` is the index of the package node for the feature.
fn add_feature_rec(
    graph: &mut Graph<'_>,
    resolve: &Resolve,
    feature_name: InternedString,
    package_id: PackageId,
    from: usize,
    package_index: usize,
) {
    let feature_map = resolve.summary(package_id).features();
    let Some(fvs) = feature_map.get(&feature_name) else {
        return;
    };
    for fv in fvs {
        match fv {
            FeatureValue::Feature(dep_name) => {
                let (missing, feat_index) = add_feature(
                    graph,
                    *dep_name,
                    Some(from),
                    package_index,
                    EdgeKind::Feature,
                );
                // Don't recursive if the edge already exists to deal with cycles.
                if missing {
                    add_feature_rec(
                        graph,
                        resolve,
                        *dep_name,
                        package_id,
                        feat_index,
                        package_index,
                    );
                }
            }
            // Dependencies are already shown in the graph as dep edges. I'm
            // uncertain whether or not this might be confusing in some cases
            // (like feature `"somefeat" = ["dep:somedep"]`), so maybe in the
            // future consider explicitly showing this?
            FeatureValue::Dep { .. } => {}
            FeatureValue::DepFeature {
                dep_name,
                dep_feature,
                // Note: `weak` is mostly handled when the graph is built in
                // `is_dep_activated` which is responsible for skipping
                // unactivated weak dependencies. Here it is only used to
                // determine if the feature of the dependency name is
                // activated on self.
                weak,
            } => {
                let dep_indexes = match graph.dep_name_map[&package_index].get(dep_name) {
                    Some(indexes) => indexes.clone(),
                    None => {
                        tracing::debug!(
                            "enabling feature {} on {}, found {}/{}, \
                             dep appears to not be enabled",
                            feature_name,
                            package_id,
                            dep_name,
                            dep_feature
                        );
                        continue;
                    }
                };
                for (dep_index, is_optional) in dep_indexes {
                    let dep_pkg_id = graph.package_id_for_index(dep_index);
                    if is_optional && !weak {
                        // Activate the optional dep on self.
                        add_feature(
                            graph,
                            *dep_name,
                            Some(from),
                            package_index,
                            EdgeKind::Feature,
                        );
                    }
                    let (missing, feat_index) = add_feature(
                        graph,
                        *dep_feature,
                        Some(from),
                        dep_index,
                        EdgeKind::Feature,
                    );
                    if missing {
                        add_feature_rec(
                            graph,
                            resolve,
                            *dep_feature,
                            dep_pkg_id,
                            feat_index,
                            dep_index,
                        );
                    }
                }
            }
        }
    }
}
