use crate::cli;
use crate::command_prelude::*;
use anyhow::{bail, format_err};
use cargo::core::dependency::DepKind;
use cargo::ops::tree::{self, EdgeKind};
use cargo::ops::Packages;
use cargo::util::CargoResult;
use std::collections::HashSet;
use std::str::FromStr;

pub fn cli() -> App {
    subcommand("tree")
        .about("Display a tree visualization of a dependency graph")
        .arg(opt("quiet", "Suppress status messages").short("q"))
        .arg_manifest_path()
        .arg_package_spec_no_all(
            "Package to be used as the root of the tree",
            "Display the tree for all packages in the workspace",
            "Exclude specific workspace members",
        )
        .arg(Arg::with_name("all").long("all").short("a").hidden(true))
        .arg(
            Arg::with_name("all-targets")
                .long("all-targets")
                .hidden(true),
        )
        .arg_features()
        .arg_target_triple(
            "Filter dependencies matching the given target-triple (default host platform)",
        )
        .arg(
            Arg::with_name("no-dev-dependencies")
                .long("no-dev-dependencies")
                .hidden(true),
        )
        .arg(
            multi_opt(
                "edges",
                "KINDS",
                "The kinds of dependencies to display \
                 (features, normal, build, dev, all, no-dev, no-build, no-normal)",
            )
            .short("e"),
        )
        .arg(
            optional_multi_opt(
                "invert",
                "SPEC",
                "Invert the tree direction and focus on the given package",
            )
            .short("i"),
        )
        .arg(Arg::with_name("no-indent").long("no-indent").hidden(true))
        .arg(
            Arg::with_name("prefix-depth")
                .long("prefix-depth")
                .hidden(true),
        )
        .arg(
            opt(
                "prefix",
                "Change the prefix (indentation) of how each entry is displayed",
            )
            .value_name("PREFIX")
            .possible_values(&["depth", "indent", "none"])
            .default_value("indent"),
        )
        .arg(opt(
            "no-dedupe",
            "Do not de-duplicate (repeats all shared dependencies)",
        ))
        .arg(
            opt(
                "duplicates",
                "Show only dependencies which come in multiple versions (implies -i)",
            )
            .short("d")
            .alias("duplicate"),
        )
        .arg(
            opt("charset", "Character set to use in output: utf8, ascii")
                .value_name("CHARSET")
                .possible_values(&["utf8", "ascii"])
                .default_value("utf8"),
        )
        .arg(
            opt("format", "Format string used for printing dependencies")
                .value_name("FORMAT")
                .short("f")
                .default_value("{p}"),
        )
        .arg(
            // Backwards compatibility with old cargo-tree.
            Arg::with_name("version")
                .long("version")
                .short("V")
                .hidden(true),
        )
}

pub fn exec(config: &mut Config, args: &ArgMatches<'_>) -> CliResult {
    if args.is_present("version") {
        let verbose = args.occurrences_of("verbose") > 0;
        let version = cli::get_version_string(verbose);
        cargo::drop_print!(config, "{}", version);
        return Ok(());
    }
    let prefix = if args.is_present("no-indent") {
        config
            .shell()
            .warn("the --no-indent flag has been changed to --prefix=none")?;
        "none"
    } else if args.is_present("prefix-depth") {
        config
            .shell()
            .warn("the --prefix-depth flag has been changed to --prefix=depth")?;
        "depth"
    } else {
        args.value_of("prefix").unwrap()
    };
    let prefix = tree::Prefix::from_str(prefix).map_err(|e| anyhow::anyhow!("{}", e))?;

    let no_dedupe = args.is_present("no-dedupe") || args.is_present("all");
    if args.is_present("all") {
        config.shell().warn(
            "The `cargo tree` --all flag has been changed to --no-dedupe, \
             and may be removed in a future version.\n\
             If you are looking to display all workspace members, use the --workspace flag.",
        )?;
    }

    let targets = if args.is_present("all-targets") {
        config
            .shell()
            .warn("the --all-targets flag has been changed to --target=all")?;
        vec!["all".to_string()]
    } else {
        args._values_of("target")
    };
    let target = tree::Target::from_cli(targets);

    let edge_kinds = parse_edge_kinds(config, args)?;
    let graph_features = edge_kinds.contains(&EdgeKind::Feature);

    let packages = args.packages_from_flags()?;
    let mut invert = args
        .values_of("invert")
        .map_or_else(|| Vec::new(), |is| is.map(|s| s.to_string()).collect());
    if args.is_present_with_zero_values("invert") {
        match &packages {
            Packages::Packages(ps) => {
                // Backwards compatibility with old syntax of `cargo tree -i -p foo`.
                invert.extend(ps.clone());
            }
            _ => {
                return Err(format_err!(
                    "The `-i` flag requires a package name.\n\
\n\
The `-i` flag is used to inspect the reverse dependencies of a specific\n\
package. It will invert the tree and display the packages that depend on the\n\
given package.\n\
\n\
Note that in a workspace, by default it will only display the package's\n\
reverse dependencies inside the tree of the workspace member in the current\n\
directory. The --workspace flag can be used to extend it so that it will show\n\
the package's reverse dependencies across the entire workspace. The -p flag\n\
can be used to display the package's reverse dependencies only with the\n\
subtree of the package given to -p.\n\
"
                )
                .into());
            }
        }
    }

    let ws = args.workspace(config)?;
    let charset = tree::Charset::from_str(args.value_of("charset").unwrap())
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    let opts = tree::TreeOptions {
        features: values(args, "features"),
        all_features: args.is_present("all-features"),
        no_default_features: args.is_present("no-default-features"),
        packages,
        target,
        edge_kinds,
        invert,
        prefix,
        no_dedupe,
        duplicates: args.is_present("duplicates"),
        charset,
        format: args.value_of("format").unwrap().to_string(),
        graph_features,
    };

    tree::build_and_print(&ws, &opts)?;
    Ok(())
}

fn parse_edge_kinds(config: &Config, args: &ArgMatches<'_>) -> CargoResult<HashSet<EdgeKind>> {
    let mut kinds: Vec<&str> = args
        .values_of("edges")
        .map_or_else(|| Vec::new(), |es| es.flat_map(|e| e.split(',')).collect());
    if args.is_present("no-dev-dependencies") {
        config
            .shell()
            .warn("the --no-dev-dependencies flag has changed to -e=no-dev")?;
        kinds.push("no-dev");
    }
    if kinds.is_empty() {
        kinds.extend(&["normal", "build", "dev"]);
    }

    let mut result = HashSet::new();
    let insert_defaults = |result: &mut HashSet<EdgeKind>| {
        result.insert(EdgeKind::Dep(DepKind::Normal));
        result.insert(EdgeKind::Dep(DepKind::Build));
        result.insert(EdgeKind::Dep(DepKind::Development));
    };
    let unknown = |k| {
        bail!(
            "unknown edge kind `{}`, valid values are \
                \"normal\", \"build\", \"dev\", \
                \"no-normal\", \"no-build\", \"no-dev\", \
                \"features\", or \"all\"",
            k
        )
    };
    if kinds.iter().any(|k| k.starts_with("no-")) {
        insert_defaults(&mut result);
        for kind in &kinds {
            match *kind {
                "no-normal" => result.remove(&EdgeKind::Dep(DepKind::Normal)),
                "no-build" => result.remove(&EdgeKind::Dep(DepKind::Build)),
                "no-dev" => result.remove(&EdgeKind::Dep(DepKind::Development)),
                "features" => result.insert(EdgeKind::Feature),
                "normal" | "build" | "dev" | "all" => {
                    bail!("`no-` dependency kinds cannot be mixed with other dependency kinds")
                }
                k => return unknown(k),
            };
        }
        return Ok(result);
    }
    for kind in &kinds {
        match *kind {
            "all" => {
                insert_defaults(&mut result);
                result.insert(EdgeKind::Feature);
            }
            "features" => {
                result.insert(EdgeKind::Feature);
            }
            "normal" => {
                result.insert(EdgeKind::Dep(DepKind::Normal));
            }
            "build" => {
                result.insert(EdgeKind::Dep(DepKind::Build));
            }
            "dev" => {
                result.insert(EdgeKind::Dep(DepKind::Development));
            }
            k => return unknown(k),
        }
    }
    if kinds.len() == 1 && kinds[0] == "features" {
        insert_defaults(&mut result);
    }
    Ok(result)
}
