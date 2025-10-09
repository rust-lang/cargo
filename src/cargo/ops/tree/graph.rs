//! Code for building the graph used by `cargo tree`.

use super::TreeOptions;
use crate::core::compiler::{CompileKind, RustcTargetData};
use crate::core::dependency::DepKind;
use crate::core::resolver::Resolve;
use crate::core::resolver::features::{CliFeatures, FeaturesFor, ResolvedFeatures};
use crate::core::{FeatureMap, FeatureValue, Package, PackageId, PackageIdSpec, Workspace};
use crate::util::CargoResult;
use crate::util::interning::{INTERNED_DEFAULT, InternedString};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Copy, Clone)]
pub struct NodeId {
    index: usize,
    #[allow(dead_code)] // intended for `derive(Debug)`
    debug: InternedString,
}

impl NodeId {
    fn new(index: usize, debug: InternedString) -> Self {
        Self { index, debug }
    }
}

impl PartialEq for NodeId {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index
    }
}

impl Eq for NodeId {}

impl PartialOrd for NodeId {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for NodeId {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.index.cmp(&other.index)
    }
}

impl std::hash::Hash for NodeId {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.index.hash(state)
    }
}

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
        node_index: NodeId,
        /// Name of the feature.
        name: InternedString,
    },
}

impl Node {
    fn name(&self) -> InternedString {
        match self {
            Self::Package { package_id, .. } => package_id.name(),
            Self::Feature { name, .. } => *name,
        }
    }
}

#[derive(Debug, Copy, Hash, Eq, Clone, PartialEq)]
pub struct Edge {
    kind: EdgeKind,
    node: NodeId,
    public: bool,
}

impl Edge {
    pub fn kind(&self) -> EdgeKind {
        self.kind
    }

    pub fn node(&self) -> NodeId {
        self.node
    }

    pub fn public(&self) -> bool {
        self.public
    }
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
#[derive(Clone, Debug)]
struct Edges(HashMap<EdgeKind, Vec<Edge>>);

impl Edges {
    fn new() -> Edges {
        Edges(HashMap::new())
    }

    /// Adds an edge pointing to the given node.
    fn add_edge(&mut self, edge: Edge) {
        let indexes = self.0.entry(edge.kind()).or_default();
        if !indexes.contains(&edge) {
            indexes.push(edge)
        }
    }

    fn all(&self) -> impl Iterator<Item = &Edge> + '_ {
        self.0.values().flatten()
    }

    fn of_kind(&self, kind: &EdgeKind) -> &[Edge] {
        self.0.get(kind).map(Vec::as_slice).unwrap_or_default()
    }

    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// A graph of dependencies.
#[derive(Debug)]
pub struct Graph<'a> {
    nodes: Vec<Node>,
    /// The indexes of `edges` correspond to the `nodes`. That is, `edges[0]`
    /// is the set of outgoing edges for `nodes[0]`. They should always be in
    /// sync.
    edges: Vec<Edges>,
    /// Index maps a node to an index, for fast lookup.
    index: HashMap<Node, NodeId>,
    /// Map for looking up packages.
    package_map: HashMap<PackageId, &'a Package>,
    /// Set of indexes of feature nodes that were added via the command-line.
    ///
    /// For example `--features foo` will mark the "foo" node here.
    cli_features: HashSet<NodeId>,
    /// Map of dependency names, used for building internal feature map for
    /// `dep_name/feat_name` syntax.
    ///
    /// Key is the index of a package node, value is a map of `dep_name` to a
    /// set of `(pkg_node_index, is_optional)`.
    dep_name_map: HashMap<NodeId, HashMap<InternedString, HashSet<(NodeId, bool)>>>,
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
    fn add_node(&mut self, node: Node) -> NodeId {
        let from_index = NodeId::new(self.nodes.len(), node.name());
        self.nodes.push(node);
        self.edges.push(Edges::new());
        self.index.insert(self.node(from_index).clone(), from_index);
        from_index
    }

    /// Returns a list of nodes the given node index points to for the given kind.
    pub fn edges_of_kind(&self, from: NodeId, kind: &EdgeKind) -> Vec<Edge> {
        let edges = self.edges(from).of_kind(kind);
        // Created a sorted list for consistent output.
        let mut edges = edges.to_owned();
        edges.sort_unstable_by(|a, b| self.node(a.node()).cmp(&self.node(b.node())));
        edges
    }

    fn edges(&self, from: NodeId) -> &Edges {
        &self.edges[from.index]
    }

    fn edges_mut(&mut self, from: NodeId) -> &mut Edges {
        &mut self.edges[from.index]
    }

    /// Returns `true` if the given node has any outgoing edges.
    pub fn has_outgoing_edges(&self, index: NodeId) -> bool {
        !self.edges(index).is_empty()
    }

    /// Gets a node by index.
    pub fn node(&self, index: NodeId) -> &Node {
        &self.nodes[index.index]
    }

    /// Given a slice of `PackageIds`, returns the indexes of all nodes that match.
    pub fn indexes_from_ids(&self, package_ids: &[PackageId]) -> Vec<NodeId> {
        let mut result: Vec<(&Node, NodeId)> = self
            .nodes
            .iter()
            .enumerate()
            .filter(|(_i, node)| match node {
                Node::Package { package_id, .. } => package_ids.contains(package_id),
                _ => false,
            })
            .map(|(i, node)| (node, NodeId::new(i, node.name())))
            .collect();
        // Sort for consistent output (the same command should always return
        // the same output). "unstable" since nodes should always be unique.
        result.sort_unstable();
        result.into_iter().map(|(_node, i)| i).collect()
    }

    pub fn package_for_id(&self, id: PackageId) -> &Package {
        self.package_map[&id]
    }

    fn package_id_for_index(&self, index: NodeId) -> PackageId {
        match self.node(index) {
            Node::Package { package_id, .. } => *package_id,
            Node::Feature { .. } => panic!("unexpected feature node"),
        }
    }

    /// Returns `true` if the given feature node index is a feature enabled
    /// via the command-line.
    pub fn is_cli_feature(&self, index: NodeId) -> bool {
        self.cli_features.contains(&index)
    }

    /// Returns a new graph by removing all nodes not reachable from the
    /// given nodes.
    pub fn from_reachable(&self, roots: &[NodeId]) -> Graph<'a> {
        // Graph built with features does not (yet) support --duplicates.
        assert!(self.dep_name_map.is_empty());
        let mut new_graph = Graph::new(self.package_map.clone());
        // Maps old index to new index. None if not yet visited.
        let mut remap: Vec<Option<NodeId>> = vec![None; self.nodes.len()];

        fn visit(
            graph: &Graph<'_>,
            new_graph: &mut Graph<'_>,
            remap: &mut Vec<Option<NodeId>>,
            index: NodeId,
        ) -> NodeId {
            if let Some(new_index) = remap[index.index] {
                // Already visited.
                return new_index;
            }
            let node = graph.node(index).clone();
            let new_from = new_graph.add_node(node);
            remap[index.index] = Some(new_from);
            // Visit dependencies.
            for edge in graph.edges(index).all() {
                let new_to_index = visit(graph, new_graph, remap, edge.node());
                let new_edge = Edge {
                    kind: edge.kind(),
                    node: new_to_index,
                    public: edge.public(),
                };
                new_graph.edges_mut(new_from).add_edge(new_edge);
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
            for edge in node_edges.all() {
                let new_edge = Edge {
                    kind: edge.kind(),
                    node: NodeId::new(from_idx, self.nodes[from_idx].name()),
                    public: edge.public(),
                };
                new_edges[edge.node().index].add_edge(new_edge);
            }
        }
        self.edges = new_edges;
    }

    /// Returns a list of nodes that are considered "duplicates" (same package
    /// name, with different versions/features/source/etc.).
    pub fn find_duplicates(&self) -> Vec<NodeId> {
        // Graph built with features does not (yet) support --duplicates.
        assert!(self.dep_name_map.is_empty());

        // Collect a map of package name to Vec<(&Node, NodeId)>.
        let mut packages = HashMap::new();
        for (i, node) in self.nodes.iter().enumerate() {
            if let Node::Package { package_id, .. } = node {
                packages
                    .entry(package_id.name())
                    .or_insert_with(Vec::new)
                    .push((node, NodeId::new(i, node.name())));
            }
        }

        let mut dupes: Vec<(&Node, NodeId)> = packages
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
    members_with_features.sort_unstable_by_key(|(member, _)| member.package_id());
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
) -> NodeId {
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
    let mut dep_name_map: HashMap<InternedString, HashSet<(NodeId, bool)>> = HashMap::new();
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
                // Filter out proc-macros if requested.
                if opts.no_proc_macro && graph.package_for_id(dep_id).proc_macro() {
                    return false;
                }
                // Filter out private dependencies if requested.
                if opts.public && !dep.is_public() {
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

        deps.sort_unstable_by_key(|dep| (dep.kind(), dep.name_in_toml()));
        let dep_pkg = graph.package_map[&dep_id];

        for dep in deps {
            let dep_features_for = match dep
                .artifact()
                .and_then(|artifact| artifact.target())
                .and_then(|target| target.to_resolved_compile_target(requested_kind))
            {
                // Dependency has a `{ …, target = <triple> }`
                Some(target) => FeaturesFor::ArtifactDep(target),
                // Get the information of the dependent crate from `features_for`.
                // If a dependent crate is
                //
                // * specified as an artifact dep with a `target`, or
                // * a host dep,
                //
                // its transitive deps, including build-deps, need to be built on that target.
                None if features_for != FeaturesFor::default() => features_for,
                // Dependent crate is a normal dep, then back to old rules:
                //
                // * normal deps, dev-deps -> inherited target
                // * build-deps -> host
                None => {
                    if dep.is_build() || dep_pkg.proc_macro() {
                        FeaturesFor::HostDep
                    } else {
                        features_for
                    }
                }
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
            let new_edge = Edge {
                kind: EdgeKind::Dep(dep.kind()),
                node: dep_index,
                public: dep.is_public(),
            };
            if opts.graph_features {
                // Add the dependency node with feature nodes in-between.
                dep_name_map
                    .entry(dep.name_in_toml())
                    .or_default()
                    .insert((dep_index, dep.is_optional()));
                if dep.uses_default_features() {
                    add_feature(graph, INTERNED_DEFAULT, Some(from_index), new_edge);
                }
                for feature in dep.features().iter() {
                    add_feature(graph, *feature, Some(from_index), new_edge);
                }
                if !dep.uses_default_features() && dep.features().is_empty() {
                    // No features, use a direct connection.
                    graph.edges_mut(from_index).add_edge(new_edge);
                }
            } else {
                graph.edges_mut(from_index).add_edge(new_edge);
            }
        }
    }
    if opts.graph_features {
        assert!(
            graph
                .dep_name_map
                .insert(from_index, dep_name_map)
                .is_none()
        );
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
    from: Option<NodeId>,
    to: Edge,
) -> (bool, NodeId) {
    // `to` *must* point to a package node.
    assert!(matches! {graph.node(to.node()), Node::Package{..}});
    let node = Node::Feature {
        node_index: to.node(),
        name,
    };
    let (missing, node_index) = match graph.index.get(&node) {
        Some(idx) => (false, *idx),
        None => (true, graph.add_node(node)),
    };
    if let Some(from) = from {
        let from_edge = Edge {
            kind: to.kind(),
            node: node_index,
            public: to.public(),
        };
        graph.edges_mut(from).add_edge(from_edge);
    }
    let to_edge = Edge {
        kind: EdgeKind::Feature,
        node: to.node(),
        public: true,
    };
    graph.edges_mut(node_index).add_edge(to_edge);
    (missing, node_index)
}

/// Adds nodes for features requested on the command-line for the given member.
///
/// Feature nodes are added as "roots" (i.e., they have no "from" index),
/// because they come from the outside world. They usually only appear with
/// `--invert`.
fn add_cli_features(
    graph: &mut Graph<'_>,
    package_index: NodeId,
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
        to_add.insert(FeatureValue::Feature(INTERNED_DEFAULT));
    }
    to_add.extend(cli_features.features.iter().cloned());

    // Add each feature as a node, and mark as "from command-line" in graph.cli_features.
    for fv in to_add {
        match fv {
            FeatureValue::Feature(feature) => {
                let feature_edge = Edge {
                    kind: EdgeKind::Feature,
                    node: package_index,
                    public: true,
                };
                let index = add_feature(graph, feature, None, feature_edge).1;
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
                            graph.nodes.get(package_index.index)
                        );
                    }
                };
                for (dep_index, is_optional) in dep_connections {
                    if is_optional {
                        // Activate the optional dep on self.
                        let feature_edge = Edge {
                            kind: EdgeKind::Feature,
                            node: package_index,
                            public: true,
                        };
                        let index = add_feature(graph, dep_name, None, feature_edge).1;
                        graph.cli_features.insert(index);
                    }
                    let dep_edge = Edge {
                        kind: EdgeKind::Feature,
                        node: dep_index,
                        public: true,
                    };
                    let index = add_feature(graph, dep_feature, None, dep_edge).1;
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
    let feature_nodes: Vec<(PackageId, NodeId, NodeId, InternedString)> = graph
        .nodes
        .iter()
        .enumerate()
        .filter_map(|(i, node)| match node {
            Node::Package { .. } => None,
            Node::Feature { node_index, name } => {
                let package_id = graph.package_id_for_index(*node_index);
                Some((package_id, *node_index, NodeId::new(i, *name), *name))
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
    from: NodeId,
    package_index: NodeId,
) {
    let feature_map = resolve.summary(package_id).features();
    let Some(fvs) = feature_map.get(&feature_name) else {
        return;
    };
    for fv in fvs {
        match fv {
            FeatureValue::Feature(dep_name) => {
                let feature_edge = Edge {
                    kind: EdgeKind::Feature,
                    node: package_index,
                    public: true,
                };
                let (missing, feat_index) = add_feature(graph, *dep_name, Some(from), feature_edge);
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
                        let feature_edge = Edge {
                            kind: EdgeKind::Feature,
                            node: package_index,
                            public: true,
                        };
                        add_feature(graph, *dep_name, Some(from), feature_edge);
                    }
                    let dep_edge = Edge {
                        kind: EdgeKind::Feature,
                        node: dep_index,
                        public: true,
                    };
                    let (missing, feat_index) =
                        add_feature(graph, *dep_feature, Some(from), dep_edge);
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
