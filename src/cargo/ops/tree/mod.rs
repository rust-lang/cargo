//! Implementation of `cargo tree`.

use self::format::Pattern;
use crate::core::compiler::{CompileKind, RustcTargetData};
use crate::core::dependency::DepKind;
use crate::core::resolver::{ForceAllTargets, HasDevUnits, features::CliFeatures};
use crate::core::{Package, PackageId, PackageIdSpec, PackageIdSpecQuery, Workspace};
use crate::ops::resolve::SpecsAndResolvedFeatures;
use crate::ops::{self, Packages};
use crate::util::CargoResult;
use crate::util::style;
use crate::{drop_print, drop_println};
use anyhow::Context as _;
use graph::Graph;
use std::collections::{HashMap, HashSet};
use std::str::FromStr;

mod format;
mod graph;

pub use {graph::EdgeKind, graph::Node, graph::NodeId};

pub struct TreeOptions {
    pub cli_features: CliFeatures,
    /// The packages to display the tree for.
    pub packages: Packages,
    /// The platform to filter for.
    pub target: Target,
    /// The dependency kinds to display.
    pub edge_kinds: HashSet<EdgeKind>,
    pub invert: Vec<String>,
    /// The packages to prune from the display of the dependency tree.
    pub pkgs_to_prune: Vec<String>,
    /// The style of prefix for each line.
    pub prefix: Prefix,
    /// If `true`, duplicates will be repeated.
    /// If `false`, duplicates will be marked with `*`, and their dependencies
    /// won't be shown.
    pub no_dedupe: bool,
    /// If `true`, run in a special mode where it will scan for packages that
    /// appear with different versions, and report if any where found. Implies
    /// `invert`.
    pub duplicates: bool,
    /// A format string indicating how each package should be displayed.
    pub format: String,
    /// Includes features in the tree as separate nodes.
    pub graph_features: bool,
    /// Display depth of the dependency tree.
    /// If non-negative integer, display dependencies with that amount of max depth.
    /// If `workspace`, display dependencies from current workspace only.
    pub display_depth: DisplayDepth,
    /// Excludes proc-macro dependencies.
    pub no_proc_macro: bool,
    /// Include only public dependencies.
    pub public: bool,
}

#[derive(PartialEq)]
pub enum Target {
    Host,
    Specific(Vec<String>),
    All,
}

impl Target {
    pub fn from_cli(targets: Vec<String>) -> Target {
        match targets.len() {
            0 => Target::Host,
            1 if targets[0] == "all" => Target::All,
            _ => Target::Specific(targets),
        }
    }
}

#[derive(Clone, Copy)]
pub enum Prefix {
    None,
    Indent,
    Depth,
}

impl FromStr for Prefix {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Prefix, &'static str> {
        match s {
            "none" => Ok(Prefix::None),
            "indent" => Ok(Prefix::Indent),
            "depth" => Ok(Prefix::Depth),
            _ => Err("invalid prefix"),
        }
    }
}

#[derive(Clone, Copy)]
pub enum DisplayDepth {
    MaxDisplayDepth(u32),
    Workspace,
}

impl FromStr for DisplayDepth {
    type Err = clap::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "workspace" => Ok(Self::Workspace),
            s => s.parse().map(Self::MaxDisplayDepth).map_err(|_| {
                clap::Error::raw(
                    clap::error::ErrorKind::ValueValidation,
                    format!(
                        "supported values for --depth are non-negative integers and `workspace`, \
                                but `{}` is unknown",
                        s
                    ),
                )
            }),
        }
    }
}

struct Symbols {
    down: &'static str,
    tee: &'static str,
    ell: &'static str,
    right: &'static str,
}

static UTF8_SYMBOLS: Symbols = Symbols {
    down: "│",
    tee: "├",
    ell: "└",
    right: "─",
};

static ASCII_SYMBOLS: Symbols = Symbols {
    down: "|",
    tee: "|",
    ell: "`",
    right: "-",
};

/// Entry point for the `cargo tree` command.
pub fn build_and_print(ws: &Workspace<'_>, opts: &TreeOptions) -> CargoResult<()> {
    let requested_targets = match &opts.target {
        Target::All | Target::Host => Vec::new(),
        Target::Specific(t) => t.clone(),
    };
    // TODO: Target::All is broken with -Zfeatures=itarget. To handle that properly,
    // `FeatureResolver` will need to be taught what "all" means.
    let requested_kinds = CompileKind::from_requested_targets(ws.gctx(), &requested_targets)?;
    let mut target_data = RustcTargetData::new(ws, &requested_kinds)?;
    let specs = opts.packages.to_package_id_specs(ws)?;
    let has_dev = if opts
        .edge_kinds
        .contains(&EdgeKind::Dep(DepKind::Development))
    {
        HasDevUnits::Yes
    } else {
        HasDevUnits::No
    };
    let force_all = if opts.target == Target::All {
        ForceAllTargets::Yes
    } else {
        ForceAllTargets::No
    };
    let dry_run = false;
    let ws_resolve = ops::resolve_ws_with_opts(
        ws,
        &mut target_data,
        &requested_kinds,
        &opts.cli_features,
        &specs,
        has_dev,
        force_all,
        dry_run,
    )?;

    let package_map: HashMap<PackageId, &Package> = ws_resolve
        .pkg_set
        .packages()
        .map(|pkg| (pkg.package_id(), pkg))
        .collect();

    for SpecsAndResolvedFeatures {
        specs,
        resolved_features,
    } in ws_resolve.specs_and_features
    {
        let mut graph = graph::build(
            ws,
            &ws_resolve.targeted_resolve,
            &resolved_features,
            &specs,
            &opts.cli_features,
            &target_data,
            &requested_kinds,
            package_map.clone(),
            opts,
        )?;

        let root_specs = if opts.invert.is_empty() {
            specs
        } else {
            opts.invert
                .iter()
                .map(|p| PackageIdSpec::parse(p))
                .collect::<Result<Vec<PackageIdSpec>, _>>()?
        };
        let root_ids = ws_resolve.targeted_resolve.specs_to_ids(&root_specs)?;
        let root_indexes = graph.indexes_from_ids(&root_ids);

        let root_indexes = if opts.duplicates {
            // `-d -p foo` will only show duplicates within foo's subtree
            graph = graph.from_reachable(root_indexes.as_slice());
            graph.find_duplicates()
        } else {
            root_indexes
        };

        if !opts.invert.is_empty() || opts.duplicates {
            graph.invert();
        }

        // Packages to prune.
        let pkgs_to_prune = opts
            .pkgs_to_prune
            .iter()
            .map(|p| PackageIdSpec::parse(p).map_err(Into::into))
            .map(|r| {
                // Provide an error message if pkgid is not within the resolved
                // dependencies graph.
                r.and_then(|spec| spec.query(ws_resolve.targeted_resolve.iter()).and(Ok(spec)))
            })
            .collect::<CargoResult<Vec<PackageIdSpec>>>()?;

        if root_indexes.len() == 0 {
            ws.gctx().shell().warn(
                "nothing to print.\n\n\
            To find dependencies that require specific target platforms, \
            try to use option `--target all` first, and then narrow your search scope accordingly.",
            )?;
        } else {
            print(ws, opts, root_indexes, &pkgs_to_prune, &graph)?;
        }
    }
    Ok(())
}

/// Prints a tree for each given root.
fn print(
    ws: &Workspace<'_>,
    opts: &TreeOptions,
    roots: Vec<NodeId>,
    pkgs_to_prune: &[PackageIdSpec],
    graph: &Graph<'_>,
) -> CargoResult<()> {
    let format = Pattern::new(&opts.format)
        .with_context(|| format!("tree format `{}` not valid", opts.format))?;

    let symbols = if ws.gctx().shell().out_unicode() {
        &UTF8_SYMBOLS
    } else {
        &ASCII_SYMBOLS
    };

    // The visited deps is used to display a (*) whenever a dep has
    // already been printed (ignored with --no-dedupe).
    let mut visited_deps = HashSet::new();

    for (i, root_index) in roots.into_iter().enumerate() {
        if i != 0 {
            drop_println!(ws.gctx());
        }

        // A stack of bools used to determine where | symbols should appear
        // when printing a line.
        let mut levels_continue = vec![];
        // The print stack is used to detect dependency cycles when
        // --no-dedupe is used. It contains a Node for each level.
        let mut print_stack = vec![];

        print_node(
            ws,
            graph,
            root_index,
            &format,
            symbols,
            pkgs_to_prune,
            opts.prefix,
            opts.no_dedupe,
            opts.display_depth,
            &mut visited_deps,
            &mut levels_continue,
            &mut print_stack,
        )?;
    }

    Ok(())
}

/// Prints a package and all of its dependencies.
fn print_node<'a>(
    ws: &Workspace<'_>,
    graph: &'a Graph<'_>,
    node_index: NodeId,
    format: &Pattern,
    symbols: &Symbols,
    pkgs_to_prune: &[PackageIdSpec],
    prefix: Prefix,
    no_dedupe: bool,
    display_depth: DisplayDepth,
    visited_deps: &mut HashSet<NodeId>,
    levels_continue: &mut Vec<(anstyle::Style, bool)>,
    print_stack: &mut Vec<NodeId>,
) -> CargoResult<()> {
    let new = no_dedupe || visited_deps.insert(node_index);

    match prefix {
        Prefix::Depth => drop_print!(ws.gctx(), "{}", levels_continue.len()),
        Prefix::Indent => {
            if let Some(((last_style, last_continues), rest)) = levels_continue.split_last() {
                for (style, continues) in rest {
                    let c = if *continues { symbols.down } else { " " };
                    drop_print!(ws.gctx(), "{style}{c}{style:#}   ");
                }

                let c = if *last_continues {
                    symbols.tee
                } else {
                    symbols.ell
                };
                drop_print!(
                    ws.gctx(),
                    "{last_style}{0}{1}{1}{last_style:#} ",
                    c,
                    symbols.right
                );
            }
        }
        Prefix::None => {}
    }

    let in_cycle = print_stack.contains(&node_index);
    // If this node does not have any outgoing edges, don't include the (*)
    // since there isn't really anything "deduplicated", and it generally just
    // adds noise.
    let has_deps = graph.has_outgoing_edges(node_index);
    let star = if (new && !in_cycle) || !has_deps {
        ""
    } else {
        color_print::cstr!(" <yellow,dim>(*)</>")
    };
    drop_println!(ws.gctx(), "{}{}", format.display(graph, node_index), star);

    if !new || in_cycle {
        return Ok(());
    }
    print_stack.push(node_index);

    for kind in &[
        EdgeKind::Dep(DepKind::Normal),
        EdgeKind::Dep(DepKind::Build),
        EdgeKind::Dep(DepKind::Development),
        EdgeKind::Feature,
    ] {
        print_dependencies(
            ws,
            graph,
            node_index,
            format,
            symbols,
            pkgs_to_prune,
            prefix,
            no_dedupe,
            display_depth,
            visited_deps,
            levels_continue,
            print_stack,
            kind,
        )?;
    }
    print_stack.pop();

    Ok(())
}

/// Prints all the dependencies of a package for the given dependency kind.
fn print_dependencies<'a>(
    ws: &Workspace<'_>,
    graph: &'a Graph<'_>,
    node_index: NodeId,
    format: &Pattern,
    symbols: &Symbols,
    pkgs_to_prune: &[PackageIdSpec],
    prefix: Prefix,
    no_dedupe: bool,
    display_depth: DisplayDepth,
    visited_deps: &mut HashSet<NodeId>,
    levels_continue: &mut Vec<(anstyle::Style, bool)>,
    print_stack: &mut Vec<NodeId>,
    kind: &EdgeKind,
) -> CargoResult<()> {
    let deps = graph.edges_of_kind(node_index, kind);
    if deps.is_empty() {
        return Ok(());
    }

    let name = match kind {
        EdgeKind::Dep(DepKind::Normal) => None,
        EdgeKind::Dep(DepKind::Build) => Some(color_print::cstr!(
            "<bright-blue,bold>[build-dependencies]</>"
        )),
        EdgeKind::Dep(DepKind::Development) => Some(color_print::cstr!(
            "<bright-cyan,bold>[dev-dependencies]</>"
        )),
        EdgeKind::Feature => None,
    };

    if let Prefix::Indent = prefix {
        if let Some(name) = name {
            for (style, continues) in &**levels_continue {
                let c = if *continues { symbols.down } else { " " };
                drop_print!(ws.gctx(), "{style}{c}{style:#}   ");
            }

            drop_println!(ws.gctx(), "{name}");
        }
    }

    let (max_display_depth, filter_non_workspace_member) = match display_depth {
        DisplayDepth::MaxDisplayDepth(max) => (max, false),
        DisplayDepth::Workspace => (u32::MAX, true),
    };

    // Current level exceeds maximum display depth. Skip.
    if levels_continue.len() + 1 > max_display_depth as usize {
        return Ok(());
    }

    let mut it = deps
        .iter()
        .filter(|dep| {
            // Filter out packages to prune.
            match graph.node(dep.node()) {
                Node::Package { package_id, .. } => {
                    if filter_non_workspace_member && !ws.is_member_id(*package_id) {
                        return false;
                    }
                    !pkgs_to_prune.iter().any(|spec| spec.matches(*package_id))
                }
                Node::Feature { .. } => true,
            }
        })
        .peekable();

    while let Some(dependency) = it.next() {
        let style = edge_line_color(dependency.kind());
        levels_continue.push((style, it.peek().is_some()));
        print_node(
            ws,
            graph,
            dependency.node(),
            format,
            symbols,
            pkgs_to_prune,
            prefix,
            no_dedupe,
            display_depth,
            visited_deps,
            levels_continue,
            print_stack,
        )?;
        levels_continue.pop();
    }

    Ok(())
}

fn edge_line_color(kind: EdgeKind) -> anstyle::Style {
    match kind {
        EdgeKind::Dep(DepKind::Normal) => style::DEP_NORMAL,
        EdgeKind::Dep(DepKind::Build) => style::DEP_BUILD,
        EdgeKind::Dep(DepKind::Development) => style::DEP_DEV,
        EdgeKind::Feature => style::DEP_FEATURE,
    }
}
