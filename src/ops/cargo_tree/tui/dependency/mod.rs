//! Dependency graph data and Cargo resolution for the interactive tree.

use std::{collections::BTreeSet, fmt::Write as _, path::PathBuf, sync::Arc};

use crate::util::style::{DEP_BUILD, DEP_DEV, DEP_NORMAL};
use crate::{
    GlobalContext,
    compiler::{CompileKind, RustcTargetData},
    ops,
    ops::cargo_tree::DisplayDepth,
    resolver::features::{CliFeatures, ForceAllTargets, HasDevUnits},
    util::important_paths::find_root_manifest_for_wd,
    workspace::{
        FeatureValue, Package, PackageId, PackageIdSpec, PackageIdSpecQuery, Target, Workspace,
        dependency::{DepKind, Dependency as CargoDependency},
    },
};
use anyhow::{Context, Result, bail};
use cargo_util::paths::normalize_path;
use ratatui::style::Style;
use rustc_hash::{FxHashMap, FxHashSet};

/// Identifier for a node within the dependency tree arena.
///
/// The `usize` represents the index into the arena vector.
/// This is used for efficient storage and traversal of the tree structure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(pub usize);

/// Flat representation of a dependency node in the deduplicated tree.
///
/// See [`DependencyTree`] for the full tree structure.
#[derive(Debug, Clone)]
pub struct Dependency {
    /// Crate name.
    pub name: String,
    /// Crate version.
    pub version: String,
    /// Cargo package identity, including its source.
    pub package_id: Option<PackageId>,
    /// Local manifest directory (only for workspace members).
    pub manifest_dir: Option<String>,
    /// Whether this crate exposes a proc-macro target.
    pub is_proc_macro: bool,
    /// Metadata requested by the package display format.
    pub format_metadata: Option<Arc<PackageFormatMetadata>>,
    /// Children represented as node indices for downward traversal.
    pub children: Vec<NodeId>,
}

impl From<&PackageSnapshot> for Dependency {
    fn from(snapshot: &PackageSnapshot) -> Self {
        Dependency {
            name: snapshot.name.clone(),
            version: snapshot.version.clone(),
            package_id: Some(snapshot.package_id),
            manifest_dir: snapshot.manifest_dir.clone(),
            is_proc_macro: snapshot.is_proc_macro,
            format_metadata: snapshot.format_metadata.clone(),
            children: Vec::new(), // filled in by wire_edges
        }
    }
}

/// A manifest dependency declaration that resolves to a package.
#[derive(Debug, Clone)]
pub struct DependencyUsage {
    pub parent: PackageId,
    pub kind: DepKind,
    /// Cargo's declaration, shared with the resolved graph.
    pub dependency: CargoDependency,
}

/// Package metadata fields needed by tree formatting or the TUI details popup.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PackageFormatRequirements {
    pub license: bool,
    pub repository: bool,
    pub features: bool,
    pub library: bool,
    pub source: bool,
    /// Collect additional package metadata for the TUI details popup.
    pub metadata: bool,
}

impl PackageFormatRequirements {
    pub fn with_metadata(mut self) -> Self {
        self.metadata = true;
        self.license = true;
        self.repository = true;
        self.features = true;
        self.library = true;
        self.source = true;
        self
    }
}

/// Optional Cargo package values used by formatted tree labels and the TUI popup.
#[derive(Debug, Clone, Default)]
pub struct PackageFormatMetadata {
    pub description: Option<String>,
    pub edition: Option<String>,
    pub rust_version: Option<String>,
    pub targets: Option<String>,
    pub license: Option<String>,
    pub repository: Option<String>,
    pub homepage: Option<String>,
    pub documentation: Option<String>,
    pub authors: Option<String>,
    pub keywords: Option<String>,
    pub categories: Option<String>,
    pub features: Option<String>,
    pub library: Option<String>,
    pub source: Option<String>,
}

/// Dependency group node (e.g. `[dev-dependencies]`) within the deduplicated tree.
#[derive(Debug, Clone)]
pub struct DependencyGroup {
    /// Group kind in Cargo metadata.
    pub kind: DepKind,
    /// Children represented as node indices for downward traversal.
    pub children: Vec<NodeId>,
}

/// Feature node linking a package to an enabled feature or dependency feature.
#[derive(Debug, Clone)]
pub struct DependencyFeature {
    pub package_name: String,
    pub name: String,
    pub command_line: bool,
    pub children: Vec<NodeId>,
}

impl DependencyGroup {
    pub fn label(&self) -> String {
        format!("[{}]", self.kind.kind_table())
    }

    pub fn style(&self) -> Style {
        match self.kind {
            DepKind::Normal => DEP_NORMAL.into(),
            DepKind::Build => DEP_BUILD.into(),
            DepKind::Development => DEP_DEV.into(),
        }
    }
}

/// Unified dependency node type for the deduplicated tree arena.
#[derive(Debug, Clone)]
pub enum DependencyNode {
    Crate(Dependency),
    Group(DependencyGroup),
    Feature(DependencyFeature),
}

impl DependencyNode {
    pub fn children(&self) -> &[NodeId] {
        match self {
            Self::Crate(node) => &node.children,
            Self::Group(node) => &node.children,
            Self::Feature(node) => &node.children,
        }
    }

    pub fn is_group(&self) -> bool {
        matches!(self, Self::Group(_))
    }

    pub fn is_crate(&self) -> bool {
        matches!(self, Self::Crate(_))
    }

    pub fn display_name(&self) -> String {
        match self {
            Self::Crate(node) => node.name.clone(),
            Self::Group(node) => node.label(),
            Self::Feature(node) => node.name.clone(),
        }
    }

    pub fn as_dependency(&self) -> Option<&Dependency> {
        match self {
            Self::Crate(node) => Some(node),
            _ => None,
        }
    }

    pub fn as_group(&self) -> Option<&DependencyGroup> {
        match self {
            Self::Group(node) => Some(node),
            _ => None,
        }
    }

    pub fn display_label(&self) -> String {
        match self {
            Self::Crate(node) => node.name.clone(),
            Self::Group(node) => node.label(),
            Self::Feature(node) => format!("{} feature \"{}\"", node.package_name, node.name),
        }
    }
}

/// Deduplicated dependency tree: one arena node per unique package.
///
/// Parent relationships are stored in a separate reverse-index rather than
/// on each node, since a deduplicated node can have multiple parents.
///
/// Example:
///
/// app
/// |- foo
/// |  `- baz
/// `- bar
///    `- baz
///
/// ```text
/// nodes:
///   0 = app(children = [1, 2])
///   1 = foo(children = [3])
///   2 = bar(children = [3])
///   3 = baz(children = [])
///
/// parents:
///   0 -> []
///   1 -> [0]
///   2 -> [0]
///   3 -> [1, 2]
/// ```
#[derive(Debug, Clone)]
pub struct DependencyTree {
    /// Name of the root package (or workspace placeholder when missing).
    pub workspace_name: String,
    /// Arena storing all dependency nodes.
    pub nodes: Vec<DependencyNode>,
    /// For each node, the list of parent node ids (reverse index of children).
    pub parents: Vec<Vec<NodeId>>,
    /// Incoming manifest declarations keyed by the package they resolve to.
    pub dependency_usages: FxHashMap<PackageId, Vec<DependencyUsage>>,
    /// Workspace members represented as node ids (entry points into the arena).
    pub roots: Vec<NodeId>,
}

impl DependencyTree {
    /// Resolves the Cargo workspace via the `cargo` library and converts the
    /// resolved graph into a [`DependencyTree`].
    pub fn load(manifest_path: Option<PathBuf>) -> Result<Self> {
        Self::load_with_options(manifest_path, &ResolutionOptions::default())
    }

    /// Resolves the Cargo workspace using the supplied Cargo resolver options.
    pub fn load_with_options(
        manifest_path: Option<PathBuf>,
        options: &ResolutionOptions,
    ) -> Result<Self> {
        Self::load_with_options_and_format(
            manifest_path,
            options,
            PackageFormatRequirements::default(),
        )
    }

    /// Resolves the workspace and collects only the metadata requested by the
    /// package display format.
    pub fn load_with_options_and_format(
        manifest_path: Option<PathBuf>,
        options: &ResolutionOptions,
        format_requirements: PackageFormatRequirements,
    ) -> Result<Self> {
        let gctx = GlobalContext::default().context("failed to initialize Cargo context")?;
        Self::load_with_context_and_format(&gctx, manifest_path, options, format_requirements)
    }

    /// Resolves the workspace using an existing Cargo context.
    pub fn load_with_context_and_format(
        gctx: &GlobalContext,
        manifest_path: Option<PathBuf>,
        options: &ResolutionOptions,
        format_requirements: PackageFormatRequirements,
    ) -> Result<Self> {
        let resolved =
            resolve::ResolvedWorkspace::load(gctx, manifest_path, options, format_requirements)?;
        Ok(graph::build_tree(resolved, options))
    }

    /// Returns immutable access to a node identified by `id`.
    pub fn node(&self, id: NodeId) -> Option<&DependencyNode> {
        self.nodes.get(id.0)
    }

    /// Returns the workspace root node ids that should be rendered.
    pub fn roots(&self) -> &[NodeId] {
        &self.roots
    }

    /// Returns the crate node ids that can be matched by search.
    pub fn crate_nodes(&self) -> impl Iterator<Item = NodeId> {
        self.nodes
            .iter()
            .enumerate()
            .filter_map(|(idx, node)| node.is_crate().then_some(NodeId(idx)))
    }
}

/// Cargo resolver options that affect which packages and dependency edges are
/// present in the dependency tree.
#[derive(Debug, Default, Clone)]
pub struct ResolutionOptions {
    /// Resolve the tree for all workspace members.
    pub workspace: bool,
    /// Resolve the tree for the selected packages.
    pub packages: Vec<String>,
    /// Exclude workspace members when `workspace` is enabled.
    pub exclude: Vec<String>,
    /// Dependency kinds to retain in the tree.
    pub edges: Vec<String>,
    /// Deprecated alias for excluding development dependencies.
    pub no_dev_dependencies: bool,
    /// Packages to omit wherever they occur as dependencies.
    pub prune: Vec<String>,
    /// Packages whose reverse dependencies should be displayed.
    pub invert: Option<Vec<String>>,
    /// Show packages that occur with multiple versions and their reverse dependencies.
    pub duplicates: bool,
    /// Initial expansion depth or workspace-only tree mode.
    pub display_depth: DisplayDepth,
    /// Explicit target triples. The special value `all` is handled like
    /// `--all-targets`.
    pub targets: Vec<String>,
    /// Resolve dependencies for every target, including tests and examples.
    pub all_targets: bool,
    /// Features requested on the command line.
    pub features: Vec<String>,
    /// Activate every available feature.
    pub all_features: bool,
    /// Do not activate the default feature.
    pub no_default_features: bool,
}

/// Feature names declared by members of a Cargo workspace.
///
/// This is loaded lazily by the TUI when the feature selector is opened.
pub fn workspace_features(manifest_path: Option<PathBuf>) -> Result<Vec<String>> {
    let gctx = GlobalContext::default().context("failed to initialize Cargo context")?;
    let manifest_path = resolve::resolve_manifest_path(&gctx, manifest_path)?;
    let workspace =
        Workspace::new(&manifest_path, &gctx).context("failed to load Cargo workspace")?;

    let features = workspace
        .members()
        .flat_map(|package| package.summary().features().keys().map(ToString::to_string))
        .collect::<BTreeSet<_>>();
    Ok(features.into_iter().collect())
}

/// Package names declared by members of a Cargo workspace.
///
/// This is loaded lazily by the TUI when the package selector is opened.
pub fn workspace_packages(manifest_path: Option<PathBuf>) -> Result<Vec<String>> {
    let gctx = GlobalContext::default().context("failed to initialize Cargo context")?;
    let manifest_path = resolve::resolve_manifest_path(&gctx, manifest_path)?;
    let workspace =
        Workspace::new(&manifest_path, &gctx).context("failed to load Cargo workspace")?;

    let packages = workspace
        .members()
        .map(|package| package.name().as_str().to_owned())
        .collect::<BTreeSet<_>>();
    Ok(packages.into_iter().collect())
}

mod graph;
mod package;
mod resolve;

pub use package::PackageSnapshot;
