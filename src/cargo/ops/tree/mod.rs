//! Implementation of `cargo tree`.

use self::format::Pattern;
use crate::core::compiler::{CompileKind, RustcTargetData};
use crate::core::dependency::DepKind;
use crate::core::resolver::{HasDevUnits, ResolveOpts};
use crate::core::{Package, PackageId, Workspace};
use crate::ops::{self, Packages};
use crate::util::CargoResult;
use anyhow::{bail, Context};
use graph::Graph;
use std::collections::{HashMap, HashSet};
use std::str::FromStr;

mod format;
mod graph;

pub use {graph::Edge, graph::Node};

pub struct TreeOptions {
    pub features: Vec<String>,
    pub no_default_features: bool,
    pub all_features: bool,
    /// The packages to display the tree for.
    pub packages: Packages,
    /// The platform to filter for.
    /// If `None`, use the host platform.
    pub target: Option<String>,
    /// If `true`, ignores the `target` field and returns all targets.
    pub no_filter_targets: bool,
    pub no_dev_dependencies: bool,
    pub invert: bool,
    /// Displays a list, with no indentation.
    pub no_indent: bool,
    /// Displays a list, with a number indicating the depth instead of using indentation.
    pub prefix_depth: bool,
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
enum Prefix {
    None,
    Indent,
    Depth,
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
    if opts.no_filter_targets && opts.target.is_some() {
        bail!("cannot specify both `--target` and `--no-filter-targets`");
    }
    if opts.graph_features && opts.duplicates {
        bail!("the `--graph-features` flag does not support `--duplicates`");
    }
    let requested_kind = CompileKind::from_requested_target(ws.config(), opts.target.as_deref())?;
    let target_data = RustcTargetData::new(ws, requested_kind)?;
    let specs = opts.packages.to_package_id_specs(ws)?;
    let resolve_opts = ResolveOpts::new(
        /*dev_deps*/ true,
        &opts.features,
        opts.all_features,
        !opts.no_default_features,
    );
    let has_dev = if opts.no_dev_dependencies {
        HasDevUnits::No
    } else {
        HasDevUnits::Yes
    };
    let ws_resolve = ops::resolve_ws_with_opts(
        ws,
        &target_data,
        requested_kind,
        &resolve_opts,
        &specs,
        has_dev,
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
        requested_kind,
        package_map,
        opts,
    )?;

    let root_ids = ws_resolve.targeted_resolve.specs_to_ids(&specs)?;
    let root_indexes = graph.indexes_from_ids(&root_ids);

    let root_indexes = if opts.duplicates {
        // `-d -p foo` will only show duplicates within foo's subtree
        graph = graph.from_reachable(root_indexes.as_slice());
        graph.find_duplicates()
    } else {
        root_indexes
    };

    if opts.invert || opts.duplicates {
        graph.invert();
    }

    print(opts, root_indexes, &graph)?;
    Ok(())
}

/// Prints a tree for each given root.
fn print(opts: &TreeOptions, roots: Vec<usize>, graph: &Graph<'_>) -> CargoResult<()> {
    let format = Pattern::new(&opts.format)
        .with_context(|| format!("tree format `{}` not valid", opts.format))?;

    let symbols = match opts.charset {
        Charset::Utf8 => &UTF8_SYMBOLS,
        Charset::Ascii => &ASCII_SYMBOLS,
    };

    let prefix = if opts.prefix_depth {
        Prefix::Depth
    } else if opts.no_indent {
        Prefix::None
    } else {
        Prefix::Indent
    };

    for (i, root_index) in roots.into_iter().enumerate() {
        if i != 0 {
            println!();
        }

        // The visited deps is used to display a (*) whenever a dep has
        // already been printed (ignored with --no-dedupe).
        let mut visited_deps = HashSet::new();
        // A stack of bools used to determine where | symbols should appear
        // when printing a line.
        let mut levels_continue = vec![];
        // The print stack is used to detect dependency cycles when
        // --no-dedupe is used. It contains a Node for each level.
        let mut print_stack = vec![];

        print_node(
            graph,
            root_index,
            &format,
            symbols,
            prefix,
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
        Prefix::Depth => print!("{}", levels_continue.len()),
        Prefix::Indent => {
            if let Some((last_continues, rest)) = levels_continue.split_last() {
                for continues in rest {
                    let c = if *continues { symbols.down } else { " " };
                    print!("{}   ", c);
                }

                let c = if *last_continues {
                    symbols.tee
                } else {
                    symbols.ell
                };
                print!("{0}{1}{1} ", c, symbols.right);
            }
        }
        Prefix::None => {}
    }

    let in_cycle = print_stack.contains(&node_index);
    let star = if new && !in_cycle { "" } else { " (*)" };
    println!("{}{}", format.display(graph, node_index), star);

    if !new || in_cycle {
        return;
    }
    print_stack.push(node_index);

    for kind in &[
        Edge::Dep(DepKind::Normal),
        Edge::Dep(DepKind::Build),
        Edge::Dep(DepKind::Development),
        Edge::Feature,
    ] {
        print_dependencies(
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
    graph: &'a Graph<'_>,
    node_index: usize,
    format: &Pattern,
    symbols: &Symbols,
    prefix: Prefix,
    no_dedupe: bool,
    visited_deps: &mut HashSet<usize>,
    levels_continue: &mut Vec<bool>,
    print_stack: &mut Vec<usize>,
    kind: &Edge,
) {
    let deps = graph.connected_nodes(node_index, kind);
    if deps.is_empty() {
        return;
    }

    let name = match kind {
        Edge::Dep(DepKind::Normal) => None,
        Edge::Dep(DepKind::Build) => Some("[build-dependencies]"),
        Edge::Dep(DepKind::Development) => Some("[dev-dependencies]"),
        Edge::Feature => None,
    };

    if let Prefix::Indent = prefix {
        if let Some(name) = name {
            for continues in &**levels_continue {
                let c = if *continues { symbols.down } else { " " };
                print!("{}   ", c);
            }

            println!("{}", name);
        }
    }

    let mut it = deps.iter().peekable();
    while let Some(dependency) = it.next() {
        levels_continue.push(it.peek().is_some());
        print_node(
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
