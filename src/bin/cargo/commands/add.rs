use cargo::sources::CRATES_IO_REGISTRY;
use cargo::util::print_available_packages;
use indexmap::IndexMap;
use indexmap::IndexSet;

use cargo::CargoResult;
use cargo::core::FeatureValue;
use cargo::core::dependency::DepKind;
use cargo::ops::cargo_add::AddOptions;
use cargo::ops::cargo_add::DepOp;
use cargo::ops::cargo_add::add;
use cargo::ops::resolve_ws;
use cargo::util::command_prelude::*;
use cargo::util::toml_mut::manifest::DepTable;

pub fn cli() -> Command {
    clap::Command::new("add")
        .about("Add dependencies to a Cargo.toml manifest file")
        .override_usage(
            color_print::cstr!("\
       <bright-cyan,bold>cargo add</> <cyan>[OPTIONS] <<DEP>>[@<<VERSION>>] ...</>
       <bright-cyan,bold>cargo add</> <cyan>[OPTIONS]</> <bright-cyan,bold>--path</> <cyan><<PATH>> ...</>
       <bright-cyan,bold>cargo add</> <cyan>[OPTIONS]</> <bright-cyan,bold>--git</> <cyan><<URL>> ...</>"
        ))
        .after_help(color_print::cstr!("Run `<bright-cyan,bold>cargo help add</>` for more detailed information.\n"))
        .group(clap::ArgGroup::new("selected").multiple(true).required(true))
        .args([
            clap::Arg::new("crates")
                .value_name("DEP_ID")
                .num_args(0..)
                .help("Reference to a package to add as a dependency")
                .long_help(
                "Reference to a package to add as a dependency

You can reference a package by:
- `<name>`, like `cargo add serde` (latest version will be used)
- `<name>@<version-req>`, like `cargo add serde@1` or `cargo add serde@=1.0.38`"
            )
                .group("selected"),
            flag("no-default-features",
                "Disable the default features"),
            flag("default-features",
                "Re-enable the default features")
                .overrides_with("no-default-features"),
            clap::Arg::new("features")
                .short('F')
                .long("features")
                .value_name("FEATURES")
                .action(ArgAction::Append)
                .help("Space or comma separated list of features to activate"),
            flag("optional",
                "Mark the dependency as optional")
                .long_help("Mark the dependency as optional

The package name will be exposed as feature of your crate.")
                .conflicts_with("dev"),
            flag("no-optional",
                "Mark the dependency as required")
                .long_help("Mark the dependency as required

The package will be removed from your features.")
                .conflicts_with("dev")
                .overrides_with("optional"),
            flag("public", "Mark the dependency as public (unstable)")
                .conflicts_with("dev")
                .conflicts_with("build")
                .long_help("Mark the dependency as public (unstable)

The dependency can be referenced in your library's public API."),
            flag("no-public", "Mark the dependency as private (unstable)")
                .conflicts_with("dev")
                .conflicts_with("build")
                .overrides_with("public")
                .long_help("Mark the dependency as private (unstable)

While you can use the crate in your implementation, it cannot be referenced in your public API."),
            clap::Arg::new("rename")
                .long("rename")
                .action(ArgAction::Set)
                .value_name("NAME")
                .help("Rename the dependency")
                .long_help("Rename the dependency

Example uses:
- Depending on multiple versions of a crate
- Depend on crates with the same name from different registries"),
        ])
        .arg_manifest_path_without_unsupported_path_tip()
        .arg_lockfile_path()
        .arg_package("Package to modify")
        .arg_ignore_rust_version()
        .arg_dry_run("Don't actually write the manifest")
        .arg_silent_suggestion()
        .next_help_heading("Source")
        .args([
            clap::Arg::new("path")
                .long("path")
                .action(ArgAction::Set)
                .value_name("PATH")
                .help("Filesystem path to local crate to add")
                .group("selected")
                .conflicts_with("git")
                .add(clap_complete::engine::ArgValueCompleter::new(
                    clap_complete::engine::PathCompleter::any()
                        .filter(|path| path.join("Cargo.toml").exists()),
                )),
            clap::Arg::new("base")
                .long("base")
                .action(ArgAction::Set)
                .value_name("BASE")
                .help("The path base to use when adding from a local crate (unstable).")
                .requires("path"),
            clap::Arg::new("git")
                .long("git")
                .action(ArgAction::Set)
                .value_name("URI")
                .help("Git repository location")
                .long_help("Git repository location

Without any other information, cargo will use latest commit on the main branch.")
                .group("selected"),
            clap::Arg::new("branch")
                .long("branch")
                .action(ArgAction::Set)
                .value_name("BRANCH")
                .help("Git branch to download the crate from")
                .requires("git")
                .group("git-ref"),
            clap::Arg::new("tag")
                .long("tag")
                .action(ArgAction::Set)
                .value_name("TAG")
                .help("Git tag to download the crate from")
                .requires("git")
                .group("git-ref"),
            clap::Arg::new("rev")
                .long("rev")
                .action(ArgAction::Set)
                .value_name("REV")
                .help("Git reference to download the crate from")
                .long_help("Git reference to download the crate from

This is the catch all, handling hashes to named references in remote repositories.")
                .requires("git")
                .group("git-ref"),
            clap::Arg::new("registry")
                .long("registry")
                .action(ArgAction::Set)
                .value_name("NAME")
                .help("Package registry for this dependency")
                .add(clap_complete::ArgValueCandidates::new(|| {
                    let candidates = get_registry_candidates();
                    candidates.unwrap_or_default()
                })),
        ])
        .next_help_heading("Section")
        .args([
            flag("dev",
                "Add as development dependency")
                .long_help("Add as development dependency

Dev-dependencies are not used when compiling a package for building, but are used for compiling tests, examples, and benchmarks.

These dependencies are not propagated to other packages which depend on this package.")
                .group("section"),
            flag("build",
                "Add as build dependency")
                .long_help("Add as build dependency

Build-dependencies are the only dependencies available for use by build scripts (`build.rs` files).")
                .group("section"),
            clap::Arg::new("target")
                .long("target")
                .action(ArgAction::Set)
                .value_name("TARGET")
                .value_parser(clap::builder::NonEmptyStringValueParser::new())
                .help("Add as dependency to the given target platform")
        ])
}

pub fn exec(gctx: &mut GlobalContext, args: &ArgMatches) -> CliResult {
    let dry_run = args.dry_run();
    let section = parse_section(args);

    let ws = args.workspace(gctx)?;

    if args.is_present_with_zero_values("package") {
        print_available_packages(&ws)?;
    }

    let packages = args.packages_from_flags()?;
    let packages = packages.get_packages(&ws)?;
    let spec = match packages.len() {
        0 => {
            return Err(CliError::new(
                anyhow::format_err!(
                    "no packages selected to modify.  Please specify one with `-p <PKGID>`"
                ),
                101,
            ));
        }
        1 => packages[0],
        _ => {
            let names = packages.iter().map(|p| p.name()).collect::<Vec<_>>();
            return Err(CliError::new(
                anyhow::format_err!(
                    "`cargo add` could not determine which package to modify. \
                    Use the `--package` option to specify a package. \n\
                    available packages: {}",
                    names.join(", ")
                ),
                101,
            ));
        }
    };

    let dependencies = parse_dependencies(gctx, args)?;

    let honor_rust_version = args.honor_rust_version();

    let options = AddOptions {
        gctx,
        spec,
        dependencies,
        section,
        dry_run,
        honor_rust_version,
    };
    add(&ws, &options)?;

    // Reload the workspace since we've changed dependencies
    let ws = args.workspace(gctx)?;
    resolve_ws(&ws, dry_run)?;

    Ok(())
}

fn parse_dependencies(gctx: &GlobalContext, matches: &ArgMatches) -> CargoResult<Vec<DepOp>> {
    let path = matches.get_one::<String>("path");
    let base = matches.get_one::<String>("base");
    let git = matches.get_one::<String>("git");
    let branch = matches.get_one::<String>("branch");
    let rev = matches.get_one::<String>("rev");
    let tag = matches.get_one::<String>("tag");
    let rename = matches.get_one::<String>("rename");
    let registry = match matches.registry(gctx)? {
        Some(reg) if reg == CRATES_IO_REGISTRY => None,
        reg => reg,
    };
    let default_features = default_features(matches);
    let optional = optional(matches);
    let public = public(matches);

    let mut crates = matches
        .get_many::<String>("crates")
        .into_iter()
        .flatten()
        .map(|c| (Some(c.clone()), None))
        .collect::<IndexMap<_, _>>();

    let mut infer_crate_name = false;

    for (crate_name, _) in crates.iter() {
        let crate_name = crate_name.as_ref().unwrap();

        if let Some(toolchain) = crate_name.strip_prefix("+") {
            anyhow::bail!(
                "invalid character `+` in dependency name: `+{toolchain}`
    Use `cargo +{toolchain} add` if you meant to use the `{toolchain}` toolchain."
            );
        }
    }

    if crates.is_empty() {
        if path.is_some() || git.is_some() {
            crates.insert(None, None);
            infer_crate_name = true;
        } else {
            unreachable!("clap should ensure we have some source selected");
        }
    }
    for feature in matches
        .get_many::<String>("features")
        .into_iter()
        .flatten()
        .map(String::as_str)
        .flat_map(parse_feature)
    {
        let parsed_value = FeatureValue::new(feature.into());
        match parsed_value {
            FeatureValue::Feature(_) => {
                if 1 < crates.len() {
                    let candidates = crates
                        .keys()
                        .map(|c| {
                            format!(
                                "`{}/{}`",
                                c.as_deref().expect("only none when there is 1"),
                                feature
                            )
                        })
                        .collect::<Vec<_>>();
                    anyhow::bail!(
                        "feature `{feature}` must be qualified by the dependency it's being activated for, like {}",
                        candidates.join(", ")
                    );
                }
                crates
                    .first_mut()
                    .expect("always at least one crate")
                    .1
                    .get_or_insert_with(IndexSet::new)
                    .insert(feature.to_owned());
            }
            FeatureValue::Dep { .. } => {
                anyhow::bail!("feature `{feature}` is not allowed to use explicit `dep:` syntax",)
            }
            FeatureValue::DepFeature {
                dep_name,
                dep_feature,
                ..
            } => {
                if infer_crate_name {
                    anyhow::bail!(
                        "`{feature}` is unsupported when inferring the crate name, use `{dep_feature}`"
                    );
                }
                if dep_feature.contains('/') {
                    anyhow::bail!("multiple slashes in feature `{feature}` is not allowed");
                }
                crates.get_mut(&Some(dep_name.as_str().to_owned())).ok_or_else(|| {
                    anyhow::format_err!("feature `{dep_feature}` activated for crate `{dep_name}` but the crate wasn't specified")
                })?
                    .get_or_insert_with(IndexSet::new)
                    .insert(dep_feature.as_str().to_owned());
            }
        }
    }

    let mut deps: Vec<DepOp> = Vec::new();
    for (crate_spec, features) in crates {
        let dep = DepOp {
            crate_spec,
            rename: rename.map(String::from),
            features,
            default_features,
            optional,
            public,
            registry: registry.clone(),
            path: path.map(String::from),
            base: base.map(String::from),
            git: git.map(String::from),
            branch: branch.map(String::from),
            rev: rev.map(String::from),
            tag: tag.map(String::from),
        };
        deps.push(dep);
    }

    if deps.len() > 1 && rename.is_some() {
        anyhow::bail!("cannot specify multiple crates with `--rename`");
    }

    Ok(deps)
}

fn default_features(matches: &ArgMatches) -> Option<bool> {
    resolve_bool_arg(
        matches.flag("default-features"),
        matches.flag("no-default-features"),
    )
}

fn optional(matches: &ArgMatches) -> Option<bool> {
    resolve_bool_arg(matches.flag("optional"), matches.flag("no-optional"))
}

fn public(matches: &ArgMatches) -> Option<bool> {
    resolve_bool_arg(matches.flag("public"), matches.flag("no-public"))
}

fn resolve_bool_arg(yes: bool, no: bool) -> Option<bool> {
    match (yes, no) {
        (true, false) => Some(true),
        (false, true) => Some(false),
        (false, false) => None,
        (_, _) => unreachable!("clap should make this impossible"),
    }
}

fn parse_section(matches: &ArgMatches) -> DepTable {
    let kind = if matches.flag("dev") {
        DepKind::Development
    } else if matches.flag("build") {
        DepKind::Build
    } else {
        DepKind::Normal
    };

    let mut table = DepTable::new().set_kind(kind);

    if let Some(target) = matches.get_one::<String>("target") {
        assert!(!target.is_empty(), "Target specification may not be empty");
        table = table.set_target(target);
    }

    table
}

/// Split feature flag list
fn parse_feature(feature: &str) -> impl Iterator<Item = &str> {
    // Not re-using `CliFeatures` because it uses a BTreeSet and loses user's ordering
    feature
        .split_whitespace()
        .flat_map(|s| s.split(','))
        .filter(|s| !s.is_empty())
}
