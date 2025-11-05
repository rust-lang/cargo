use crate::cli;
use crate::command_prelude::*;
use annotate_snippets::Level;
use anyhow::{bail, format_err};
use cargo::core::dependency::DepKind;
use cargo::ops::Packages;
use cargo::ops::tree::{self, DisplayDepth, EdgeKind};
use cargo::util::CargoResult;
use cargo::util::print_available_packages;
use clap_complete::ArgValueCandidates;
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
        .arg_silent_suggestion()
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
            .short('i')
            .add(clap_complete::ArgValueCandidates::new(
                get_pkg_id_spec_candidates,
            )),
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
                .value_parser(["utf8", "ascii"]),
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
            ArgValueCandidates::new(get_pkg_id_spec_candidates),
        )
        .arg_features()
        .arg(flag("all-targets", "Deprecated, use --target=all instead").hide(true))
        .arg_target_triple(
            "Filter dependencies matching the given target-triple (default host platform). \
            Pass `all` to include all targets.",
        )
        .arg_manifest_path()
        .arg_lockfile_path()
        .after_help(color_print::cstr!(
            "Run `<bright-cyan,bold>cargo help tree</>` for more detailed information.\n"
        ))
}

#[derive(Copy, Clone)]
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

pub fn exec(gctx: &mut GlobalContext, args: &ArgMatches) -> CliResult {
    if args.flag("version") {
        let verbose = args.verbose() > 0;
        let version = cli::get_version_string(verbose);
        cargo::drop_print!(gctx, "{}", version);
        return Ok(());
    }
    let prefix = if args.flag("no-indent") {
        gctx.shell()
            .warn("the --no-indent flag has been changed to --prefix=none")?;
        "none"
    } else if args.flag("prefix-depth") {
        gctx.shell()
            .warn("the --prefix-depth flag has been changed to --prefix=depth")?;
        "depth"
    } else {
        args.get_one::<String>("prefix").unwrap().as_str()
    };
    let prefix = tree::Prefix::from_str(prefix).map_err(|e| anyhow::anyhow!("{}", e))?;

    let no_dedupe = args.flag("no-dedupe") || args.flag("all");
    if args.flag("all") {
        gctx.shell().print_report(
            &[Level::WARNING
                .secondary_title(
                    "the `cargo tree` --all flag has been changed to --no-dedupe, \
                    and may be removed in a future version",
                )
                .element(Level::HELP.message(
                    "if you are looking to display all workspace members, use the --workspace flag",
                ))],
            false,
        )?;
    }

    let targets = if args.flag("all-targets") {
        gctx.shell()
            .warn("the --all-targets flag has been changed to --target=all")?;
        vec!["all".to_string()]
    } else {
        args.targets()?
    };
    let target = tree::Target::from_cli(targets);

    let (edge_kinds, no_proc_macro, public) = parse_edge_kinds(gctx, args)?;
    let graph_features = edge_kinds.contains(&EdgeKind::Feature);

    let pkgs_to_prune = args._values_of("prune");

    let display_depth = args
        ._value_of("depth")
        .map(|s| s.parse::<DisplayDepth>())
        .transpose()?
        .unwrap_or(DisplayDepth::MaxDisplayDepth(u32::MAX));

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

    let ws = args.workspace(gctx)?;

    if args.is_present_with_zero_values("package") {
        print_available_packages(&ws)?;
    }

    let charset = args.get_one::<String>("charset");
    if let Some(charset) = charset
        .map(|c| Charset::from_str(c))
        .transpose()
        .map_err(|e| anyhow::anyhow!("{}", e))?
    {
        match charset {
            Charset::Utf8 => gctx.shell().set_unicode(true)?,
            Charset::Ascii => gctx.shell().set_unicode(false)?,
        }
    }
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
        format: args.get_one::<String>("format").cloned().unwrap(),
        graph_features,
        display_depth,
        no_proc_macro,
        public,
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
fn parse_edge_kinds(
    gctx: &GlobalContext,
    args: &ArgMatches,
) -> CargoResult<(HashSet<EdgeKind>, bool, bool)> {
    let (kinds, no_proc_macro, public) = {
        let mut no_proc_macro = false;
        let mut public = false;
        let mut kinds = args.get_many::<String>("edges").map_or_else(
            || Vec::new(),
            |es| {
                es.flat_map(|e| e.split(','))
                    .filter(|e| {
                        if *e == "no-proc-macro" {
                            no_proc_macro = true;
                            false
                        } else if *e == "public" {
                            public = true;
                            false
                        } else {
                            true
                        }
                    })
                    .collect()
            },
        );

        if args.flag("no-dev-dependencies") {
            gctx.shell()
                .warn("the --no-dev-dependencies flag has changed to -e=no-dev")?;
            kinds.push("no-dev");
        }

        if kinds.is_empty() {
            kinds.extend(&["normal", "build", "dev"]);
        }

        if public && !gctx.cli_unstable().unstable_options {
            anyhow::bail!("`--edges public` requires `-Zunstable-options`");
        }

        (kinds, no_proc_macro, public)
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
        return Ok((result, no_proc_macro, public));
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
    Ok((result, no_proc_macro, public))
}
