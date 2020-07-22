//! Implementation of `cargo tree`.

use self::format::Pattern;
use crate::core::compiler::{CompileKind, RustcTargetData};
use crate::core::dependency::DepKind;
use crate::core::resolver::{ForceAllTargets, HasDevUnits, ResolveOpts};
use crate::core::{Package, PackageId, PackageIdSpec, Workspace};
use crate::ops::{self, Packages};
use crate::util::{CargoResult, Config};
use crate::{drop_print, drop_println};
use anyhow::{bail, Context};
use graph::Graph;
use std::collections::{HashMap, HashSet};
use std::str::FromStr;

mod format;
mod graph;

pub use {graph::EdgeKind, graph::Node};

pub struct TreeOptions {
    pub features: Vec<String>,
    pub no_default_features: bool,
    pub all_features: bool,
    /// The packages to display the tree for.
    pub packages: Packages,
    /// The platform to filter for.
    pub target: Target,
    /// The dependency kinds to display.
    pub edge_kinds: HashSet<EdgeKind>,
    pub invert: Vec<String>,
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
    /// The style of characters to use.
    pub charset: Charset,
    /// A format string indicating how each package should be displayed.
    pub format: String,
    /// Includes features in the tree as separate nodes.
    pub graph_features: bool,
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

pub enum Charset {
    Utf8,
    Ascii,
}

impl FromStr for Charset {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Charset, &'static str> {
        match s {
            "utf8" => Ok(Charset::Utf8),
            "ascii" => Ok(Charset::Ascii),
            _ => Err("invalid charset"),
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
    if opts.graph_features && opts.duplicates {
        bail!("the `-e features` flag does not support `--duplicates`");
    }
    let requested_targets = match &opts.target {
        Target::All | Target::Host => Vec::new(),
        Target::Specific(t) => t.clone(),
    };
    // TODO: Target::All is broken with -Zfeatures=itarget. To handle that properly,
    // `FeatureResolver` will need to be taught what "all" means.
    let requested_kinds = CompileKind::from_requested_targets(ws.config(), &requested_targets)?;
    let target_data = RustcTargetData::new(ws, &requested_kinds)?;
    let specs = opts.packages.to_package_id_specs(ws)?;
    let resolve_opts = ResolveOpts::new(
        /*dev_deps*/ true,
        &opts.features,
        opts.all_features,
        !opts.no_default_features,
    );
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
    let ws_resolve = ops::resolve_ws_with_opts(
        ws,
        &target_data,
        &requested_kinds,
        &resolve_opts,
        &specs,
        has_dev,
        force_all,
    )?;
    // Download all Packages. Some display formats need to display package metadata.
    let package_map: HashMap<PackageId, &Package> = ws_resolve
        .pkg_set
        .get_many(ws_resolve.pkg_set.package_ids())?
        .into_iter()
        .map(|pkg| (pkg.package_id(), pkg))
        .collect();

    let mut graph = graph::build(
        ws,
        &ws_resolve.targeted_resolve,
        &ws_resolve.resolved_features,
        &specs,
        &resolve_opts.features,
        &target_data,
        &requested_kinds,
        package_map,
        opts,
    )?;

    let root_specs = if opts.invert.is_empty() {
        specs
    } else {
        opts.invert
            .iter()
            .map(|p| PackageIdSpec::parse(p))
            .collect::<CargoResult<Vec<PackageIdSpec>>>()?
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

    print(ws.config(), opts, root_indexes, &graph)?;
    Ok(())
}

/// Prints a tree for each given root.
fn print(
    config: &Config,
    opts: &TreeOptions,
    roots: Vec<usize>,
    graph: &Graph<'_>,
) -> CargoResult<()> {
    let format = Pattern::new(&opts.format)
        .with_context(|| format!("tree format `{}` not valid", opts.format))?;

    let symbols = match opts.charset {
        Charset::Utf8 => &UTF8_SYMBOLS,
        Charset::Ascii => &ASCII_SYMBOLS,
    };

    // The visited deps is used to display a (*) whenever a dep has
    // already been printed (ignored with --no-dedupe).
    let mut visited_deps = HashSet::new();

    for (i, root_index) in roots.into_iter().enumerate() {
        if i != 0 {
            drop_println!(config);
        }

        // A stack of bools used to determine where | symbols should appear
        // when printing a line.
        let mut levels_continue = vec![];
        // The print stack is used to detect dependency cycles when
        // --no-dedupe is used. It contains a Node for each level.
        let mut print_stack = vec![];

        print_node(
            config,
            graph,
            root_index,
            &format,
            symbols,
            opts.prefix,
            opts.no_dedupe,
            &mut visited_deps,
            &mut levels_continue,
            &mut print_stack,
        );
    }

    Ok(())
}

/// Prints a package and all of its dependencies.
fn print_node<'a>(
    config: &Config,
    graph: &'a Graph<'_>,
    node_index: usize,
    format: &Pattern,
    symbols: &Symbols,
    prefix: Prefix,
    no_dedupe: bool,
    visited_deps: &mut HashSet<usize>,
    levels_continue: &mut Vec<bool>,
    print_stack: &mut Vec<usize>,
) {
    let new = no_dedupe || visited_deps.insert(node_index);

    match prefix {
        Prefix::Depth => drop_print!(config, "{}", levels_continue.len()),
        Prefix::Indent => {
            if let Some((last_continues, rest)) = levels_continue.split_last() {
                for continues in rest {
                    let c = if *continues { symbols.down } else { " " };
                    drop_print!(config, "{}   ", c);
                }

                let c = if *last_continues {
                    symbols.tee
                } else {
                    symbols.ell
                };
                drop_print!(config, "{0}{1}{1} ", c, symbols.right);
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
        " (*)"
    };
    drop_println!(config, "{}{}", format.display(graph, node_index), star);

    if !new || in_cycle {
        return;
    }
    print_stack.push(node_index);

    for kind in &[
        EdgeKind::Dep(DepKind::Normal),
        EdgeKind::Dep(DepKind::Build),
        EdgeKind::Dep(DepKind::Development),
        EdgeKind::Feature,
    ] {
        print_dependencies(
            config,
            graph,
            node_index,
            format,
            symbols,
            prefix,
            no_dedupe,
            visited_deps,
            levels_continue,
            print_stack,
            kind,
        );
    }
    print_stack.pop();
}

/// Prints all the dependencies of a package for the given dependency kind.
fn print_dependencies<'a>(
    config: &Config,
    graph: &'a Graph<'_>,
    node_index: usize,
    format: &Pattern,
    symbols: &Symbols,
    prefix: Prefix,
    no_dedupe: bool,
    visited_deps: &mut HashSet<usize>,
    levels_continue: &mut Vec<bool>,
    print_stack: &mut Vec<usize>,
    kind: &EdgeKind,
) {
    let deps = graph.connected_nodes(node_index, kind);
    if deps.is_empty() {
        return;
    }

    let name = match kind {
        EdgeKind::Dep(DepKind::Normal) => None,
        EdgeKind::Dep(DepKind::Build) => Some("[build-dependencies]"),
        EdgeKind::Dep(DepKind::Development) => Some("[dev-dependencies]"),
        EdgeKind::Feature => None,
    };

    if let Prefix::Indent = prefix {
        if let Some(name) = name {
            for continues in &**levels_continue {
                let c = if *continues { symbols.down } else { " " };
                drop_print!(config, "{}   ", c);
            }

            drop_println!(config, "{}", name);
        }
    }

    let mut it = deps.iter().peekable();
    while let Some(dependency) = it.next() {
        levels_continue.push(it.peek().is_some());
        print_node(
            config,
            graph,
            *dependency,
            format,
            symbols,
            prefix,
            no_dedupe,
            visited_deps,
            levels_continue,
            print_stack,
        );
        levels_continue.pop();
    }
}
