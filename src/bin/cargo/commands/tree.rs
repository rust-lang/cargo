use crate::cli;
use crate::command_prelude::*;
use anyhow::{bail, format_err};
use cargo::core::dependency::DepKind;
use cargo::ops::tree::{self, EdgeKind};
use cargo::ops::Packages;
use cargo::util::print_available_packages;
use cargo::util::CargoResult;
use std::collections::HashSet;
use std::str::FromStr;

pub fn cli() -> Command {
    subcommand("tree")
        .about("Display a tree visualization of a dependency graph")
        .arg(
            flag("all", "Deprecated, use --no-dedupe instead")
                .short('a')
                .hide(true),
        )
        .arg_quiet()
        .arg(flag("no-dev-dependencies", "Deprecated, use -e=no-dev instead").hide(true))
        .arg(
            multi_opt(
                "edges",
                "KINDS",
                "The kinds of dependencies to display \
                 (features, normal, build, dev, all, \
                 no-normal, no-build, no-dev, no-proc-macro)",
            )
            .short('e'),
        )
        .arg(
            optional_multi_opt(
                "invert",
                "SPEC",
                "Invert the tree direction and focus on the given package",
            )
            .short('i'),
        )
        .arg(multi_opt(
            "prune",
            "SPEC",
            "Prune the given package from the display of the dependency tree",
        ))
        .arg(opt("depth", "Maximum display depth of the dependency tree").value_name("DEPTH"))
        .arg(flag("no-indent", "Deprecated, use --prefix=none instead").hide(true))
        .arg(flag("prefix-depth", "Deprecated, use --prefix=depth instead").hide(true))
        .arg(
            opt(
                "prefix",
                "Change the prefix (indentation) of how each entry is displayed",
            )
            .value_name("PREFIX")
            .value_parser(["depth", "indent", "none"])
            .default_value("indent"),
        )
        .arg(flag(
            "no-dedupe",
            "Do not de-duplicate (repeats all shared dependencies)",
        ))
        .arg(
            flag(
                "duplicates",
                "Show only dependencies which come in multiple versions (implies -i)",
            )
            .short('d')
            .alias("duplicate"),
        )
        .arg(
            opt("charset", "Character set to use in output")
                .value_name("CHARSET")
                .value_parser(["utf8", "ascii"])
                .default_value("utf8"),
        )
        .arg(
            opt("format", "Format string used for printing dependencies")
                .value_name("FORMAT")
                .short('f')
                .default_value("{p}"),
        )
        .arg(
            // Backwards compatibility with old cargo-tree.
            flag("version", "Print version info and exit")
                .short('V')
                .hide(true),
        )
        .arg_package_spec_no_all(
            "Package to be used as the root of the tree",
            "Display the tree for all packages in the workspace",
            "Exclude specific workspace members",
        )
        .arg_features()
        .arg(flag("all-targets", "Deprecated, use --target=all instead").hide(true))
        .arg_target_triple(
            "Filter dependencies matching the given target-triple (default host platform). \
            Pass `all` to include all targets.",
        )
        .arg_manifest_path()
        .after_help(color_print::cstr!(
            "Run `<cyan,bold>cargo help tree</>` for more detailed information.\n"
        ))
}

pub fn exec(config: &mut Config, args: &ArgMatches) -> CliResult {
    if args.flag("version") {
        let verbose = args.verbose() > 0;
        let version = cli::get_version_string(verbose);
        cargo::drop_print!(config, "{}", version);
        return Ok(());
    }
    let prefix = if args.flag("no-indent") {
        config
            .shell()
            .warn("the --no-indent flag has been changed to --prefix=none")?;
        "none"
    } else if args.flag("prefix-depth") {
        config
            .shell()
            .warn("the --prefix-depth flag has been changed to --prefix=depth")?;
        "depth"
    } else {
        args.get_one::<String>("prefix").unwrap().as_str()
    };
    let prefix = tree::Prefix::from_str(prefix).map_err(|e| anyhow::anyhow!("{}", e))?;

    let no_dedupe = args.flag("no-dedupe") || args.flag("all");
    if args.flag("all") {
        config.shell().warn(
            "The `cargo tree` --all flag has been changed to --no-dedupe, \
             and may be removed in a future version.\n\
             If you are looking to display all workspace members, use the --workspace flag.",
        )?;
    }

    let targets = if args.flag("all-targets") {
        config
            .shell()
            .warn("the --all-targets flag has been changed to --target=all")?;
        vec!["all".to_string()]
    } else {
        args.targets()?
    };
    let target = tree::Target::from_cli(targets);

    let (edge_kinds, no_proc_macro) = parse_edge_kinds(config, args)?;
    let graph_features = edge_kinds.contains(&EdgeKind::Feature);

    let pkgs_to_prune = args._values_of("prune");

    let packages = args.packages_from_flags()?;
    let mut invert = args
        .get_many::<String>("invert")
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

    if args.is_present_with_zero_values("package") {
        print_available_packages(&ws)?;
    }

    let charset = tree::Charset::from_str(args.get_one::<String>("charset").unwrap())
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    let opts = tree::TreeOptions {
        cli_features: args.cli_features()?,
        packages,
        target,
        edge_kinds,
        invert,
        pkgs_to_prune,
        prefix,
        no_dedupe,
        duplicates: args.flag("duplicates"),
        charset,
        format: args.get_one::<String>("format").cloned().unwrap(),
        graph_features,
        max_display_depth: args.value_of_u32("depth")?.unwrap_or(u32::MAX),
        no_proc_macro,
    };

    if opts.graph_features && opts.duplicates {
        return Err(format_err!("the `-e features` flag does not support `--duplicates`").into());
    }

    tree::build_and_print(&ws, &opts)?;
    Ok(())
}

/// Parses `--edges` option.
///
/// Returns a tuple of `EdgeKind` map and `no_proc_marco` flag.
fn parse_edge_kinds(config: &Config, args: &ArgMatches) -> CargoResult<(HashSet<EdgeKind>, bool)> {
    let (kinds, no_proc_macro) = {
        let mut no_proc_macro = false;
        let mut kinds = args.get_many::<String>("edges").map_or_else(
            || Vec::new(),
            |es| {
                es.flat_map(|e| e.split(','))
                    .filter(|e| {
                        no_proc_macro = *e == "no-proc-macro";
                        !no_proc_macro
                    })
                    .collect()
            },
        );

        if args.flag("no-dev-dependencies") {
            config
                .shell()
                .warn("the --no-dev-dependencies flag has changed to -e=no-dev")?;
            kinds.push("no-dev");
        }

        if kinds.is_empty() {
            kinds.extend(&["normal", "build", "dev"]);
        }

        (kinds, no_proc_macro)
    };

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
                \"no-normal\", \"no-build\", \"no-dev\", \"no-proc-macro\", \
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
                    bail!(
                        "`{}` dependency kind cannot be mixed with \
                            \"no-normal\", \"no-build\", or \"no-dev\" \
                            dependency kinds",
                        kind
                    )
                }
                k => return unknown(k),
            };
        }
        return Ok((result, no_proc_macro));
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
    Ok((result, no_proc_macro))
}
