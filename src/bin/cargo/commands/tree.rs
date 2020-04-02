use crate::command_prelude::*;
use anyhow::{bail, format_err};
use cargo::core::dependency::DepKind;
use cargo::ops::tree;
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
            opt(
                "dep-kinds",
                "Dependency kinds to display \
                 (normal, build, dev, no-dev, no-build, no-normal, all)",
            )
            .value_name("KINDS"),
        )
        .arg(opt("invert", "Invert the tree direction").short("i"))
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
        .arg(opt("graph-features", "Include features in the tree"))
}

pub fn exec(config: &mut Config, args: &ArgMatches<'_>) -> CliResult {
    if args.is_present("no-indent") {
        return Err(format_err!("the --no-indent flag has been changed to --prefix=none").into());
    }
    if args.is_present("prefix-depth") {
        return Err(
            format_err!("the --prefix-depth flag has been changed to --prefix=depth").into(),
        );
    }
    if args.is_present("all") {
        return Err(format_err!(
            "The `cargo tree` --all flag has been changed to --no-dedupe.\n\
            If you are looking to display all workspace members, use the --workspace flag."
        )
        .into());
    }
    if args.is_present("all-targets") {
        return Err(format_err!("the --all-targets flag has been changed to --target=all").into());
    }
    if args.is_present("no-dev-dependencies") {
        return Err(format_err!(
            "the --no-dev-dependencies flag has changed to --dep-kinds=no-dev"
        )
        .into());
    }
    let ws = args.workspace(config)?;
    let charset = tree::Charset::from_str(args.value_of("charset").unwrap())
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    let prefix = tree::Prefix::from_str(args.value_of("prefix").unwrap())
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    let target = tree::Target::from_cli(args.value_of("target"));
    let dep_kinds = parse_dep_kinds(args.value_of("dep-kinds"))?;
    let opts = tree::TreeOptions {
        features: values(args, "features"),
        all_features: args.is_present("all-features"),
        no_default_features: args.is_present("no-default-features"),
        packages: args.packages_from_flags()?,
        target,
        dep_kinds,
        invert: args.is_present("invert"),
        prefix,
        no_dedupe: args.is_present("no-dedupe"),
        duplicates: args.is_present("duplicates"),
        charset,
        format: args.value_of("format").unwrap().to_string(),
        graph_features: args.is_present("graph-features"),
    };

    tree::build_and_print(&ws, &opts)?;
    Ok(())
}

fn parse_dep_kinds(kinds: Option<&str>) -> CargoResult<HashSet<DepKind>> {
    let kinds: Vec<&str> = kinds.unwrap_or("all").split(',').collect();
    let mut result = HashSet::new();
    let insert_all = |result: &mut HashSet<DepKind>| {
        result.insert(DepKind::Normal);
        result.insert(DepKind::Build);
        result.insert(DepKind::Development);
    };
    let unknown = |k| {
        bail!(
            "unknown dependency kind `{}`, valid values are \
                \"normal\", \"build\", \"dev\", \
                \"no-normal\", \"no-build\", \"no-dev\", \
                or \"all\"",
            k
        )
    };
    if kinds.iter().any(|k| k.starts_with("no-")) {
        insert_all(&mut result);
        for kind in &kinds {
            match *kind {
                "no-normal" => result.remove(&DepKind::Normal),
                "no-build" => result.remove(&DepKind::Build),
                "no-dev" => result.remove(&DepKind::Development),
                "normal" | "build" | "dev" | "all" => {
                    bail!("`no-` dependency kinds cannot be mixed with other dependency kinds")
                }
                k => return unknown(k),
            };
        }
        return Ok(result);
    }
    for kind in kinds {
        match kind {
            "all" => insert_all(&mut result),
            "normal" => {
                result.insert(DepKind::Normal);
            }
            "build" => {
                result.insert(DepKind::Build);
            }
            "dev" => {
                result.insert(DepKind::Development);
            }
            k => return unknown(k),
        }
    }
    Ok(result)
}
