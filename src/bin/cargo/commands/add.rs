use indexmap::IndexMap;
use indexmap::IndexSet;

use cargo::core::dependency::DepKind;
use cargo::core::FeatureValue;
use cargo::ops::cargo_add::add;
use cargo::ops::cargo_add::AddOptions;
use cargo::ops::cargo_add::DepOp;
use cargo::ops::cargo_add::DepTable;
use cargo::util::command_prelude::*;
use cargo::util::interning::InternedString;
use cargo::CargoResult;

pub fn cli() -> clap::Command<'static> {
    clap::Command::new("add")
        .setting(clap::AppSettings::DeriveDisplayOrder)
        .about("Add dependencies to a Cargo.toml manifest file")
        .override_usage(
            "\
    cargo add [OPTIONS] <DEP>[@<VERSION>] ...
    cargo add [OPTIONS] --path <PATH> ...
    cargo add [OPTIONS] --git <URL> ..."
        )
        .after_help("Run `cargo help add` for more detailed information.\n")
        .group(clap::ArgGroup::new("selected").multiple(true).required(true))
        .args([
            clap::Arg::new("crates")
                .takes_value(true)
                .value_name("DEP_ID")
                .multiple_occurrences(true)
                .help("Reference to a package to add as a dependency")
                .long_help(
                "Reference to a package to add as a dependency

You can reference a package by:
- `<name>`, like `cargo add serde` (latest version will be used)
- `<name>@<version-req>`, like `cargo add serde@1` or `cargo add serde@=1.0.38`"
            )
                .group("selected"),
            clap::Arg::new("no-default-features")
                .long("no-default-features")
                .help("Disable the default features"),
            clap::Arg::new("default-features")
                .long("default-features")
                .help("Re-enable the default features")
                .overrides_with("no-default-features"),
            clap::Arg::new("features")
                .short('F')
                .long("features")
                .takes_value(true)
                .value_name("FEATURES")
                .multiple_occurrences(true)
                .help("Space or comma separated list of features to activate"),
            clap::Arg::new("optional")
                .long("optional")
                .help("Mark the dependency as optional")
                .long_help("Mark the dependency as optional

The package name will be exposed as feature of your crate.")
                .conflicts_with("dev"),
            clap::Arg::new("no-optional")
                .long("no-optional")
                .help("Mark the dependency as required")
                .long_help("Mark the dependency as required

The package will be removed from your features.")
                .conflicts_with("dev")
                .overrides_with("optional"),
            clap::Arg::new("rename")
                .long("rename")
                .takes_value(true)
                .value_name("NAME")
                .help("Rename the dependency")
                .long_help("Rename the dependency

Example uses:
- Depending on multiple versions of a crate
- Depend on crates with the same name from different registries"),
        ])
        .arg_manifest_path()
        .args([
            clap::Arg::new("package")
                .short('p')
                .long("package")
                .takes_value(true)
                .value_name("SPEC")
                .help("Package to modify"),
            clap::Arg::new("offline")
                .long("offline")
                .help("Run without accessing the network")
        ])
        .arg_quiet()
        .arg_dry_run("Don't actually write the manifest")
        .next_help_heading("SOURCE")
        .args([
            clap::Arg::new("path")
                .long("path")
                .takes_value(true)
                .value_name("PATH")
                .help("Filesystem path to local crate to add")
                .group("selected")
                .conflicts_with("git"),
            clap::Arg::new("git")
                .long("git")
                .takes_value(true)
                .value_name("URI")
                .help("Git repository location")
                .long_help("Git repository location

Without any other information, cargo will use latest commit on the main branch.")
                .group("selected"),
            clap::Arg::new("branch")
                .long("branch")
                .takes_value(true)
                .value_name("BRANCH")
                .help("Git branch to download the crate from")
                .requires("git")
                .group("git-ref"),
            clap::Arg::new("tag")
                .long("tag")
                .takes_value(true)
                .value_name("TAG")
                .help("Git tag to download the crate from")
                .requires("git")
                .group("git-ref"),
            clap::Arg::new("rev")
                .long("rev")
                .takes_value(true)
                .value_name("REV")
                .help("Git reference to download the crate from")
                .long_help("Git reference to download the crate from

This is the catch all, handling hashes to named references in remote repositories.")
                .requires("git")
                .group("git-ref"),
            clap::Arg::new("registry")
                .long("registry")
                .takes_value(true)
                .value_name("NAME")
                .help("Package registry for this dependency"),
        ])
        .next_help_heading("SECTION")
        .args([
            clap::Arg::new("dev")
                .long("dev")
                .help("Add as development dependency")
                .long_help("Add as development dependency

Dev-dependencies are not used when compiling a package for building, but are used for compiling tests, examples, and benchmarks.

These dependencies are not propagated to other packages which depend on this package.")
                .group("section"),
            clap::Arg::new("build")
                .long("build")
                .help("Add as build dependency")
                .long_help("Add as build dependency

Build-dependencies are the only dependencies available for use by build scripts (`build.rs` files).")
                .group("section"),
            clap::Arg::new("target")
                .long("target")
                .takes_value(true)
                .value_name("TARGET")
                .forbid_empty_values(true)
                .help("Add as dependency to the given target platform")
        ])
}

pub fn exec(config: &mut Config, args: &ArgMatches) -> CliResult {
    let dry_run = args.is_present("dry-run");
    let section = parse_section(args);

    let ws = args.workspace(config)?;
    let packages = args.packages_from_flags()?;
    let packages = packages.get_packages(&ws)?;
    let spec = match packages.len() {
        0 => {
            return Err(CliError::new(
                anyhow::format_err!("no packages selected.  Please specify one with `-p <PKGID>`"),
                101,
            ));
        }
        1 => packages[0],
        len => {
            return Err(CliError::new(
                anyhow::format_err!(
                    "{len} packages selected.  Please specify one with `-p <PKGID>`",
                ),
                101,
            ));
        }
    };

    let dependencies = parse_dependencies(config, args)?;

    let options = AddOptions {
        config,
        spec,
        dependencies,
        section,
        dry_run,
    };
    add(&ws, &options)?;

    Ok(())
}

fn parse_dependencies(config: &Config, matches: &ArgMatches) -> CargoResult<Vec<DepOp>> {
    let path = matches.value_of("path");
    let git = matches.value_of("git");
    let branch = matches.value_of("branch");
    let rev = matches.value_of("rev");
    let tag = matches.value_of("tag");
    let rename = matches.value_of("rename");
    let registry = matches.registry(config)?;
    let default_features = default_features(matches);
    let optional = optional(matches);

    let mut crates = matches
        .values_of("crates")
        .into_iter()
        .flatten()
        .map(|c| (Some(String::from(c)), None))
        .collect::<IndexMap<_, _>>();
    let mut infer_crate_name = false;
    if crates.is_empty() {
        if path.is_some() || git.is_some() {
            crates.insert(None, None);
            infer_crate_name = true;
        } else {
            unreachable!("clap should ensure we have some source selected");
        }
    }
    for feature in matches
        .values_of("features")
        .into_iter()
        .flatten()
        .flat_map(parse_feature)
    {
        let parsed_value = FeatureValue::new(InternedString::new(feature));
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
                    anyhow::bail!("feature `{feature}` must be qualified by the dependency its being activated for, like {}", candidates.join(", "));
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
                    anyhow::bail!("`{feature}` is unsupported when inferring the crate name, use `{dep_feature}`");
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
            registry: registry.clone(),
            path: path.map(String::from),
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
        matches.is_present("default-features"),
        matches.is_present("no-default-features"),
    )
}

fn optional(matches: &ArgMatches) -> Option<bool> {
    resolve_bool_arg(
        matches.is_present("optional"),
        matches.is_present("no-optional"),
    )
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
    let kind = if matches.is_present("dev") {
        DepKind::Development
    } else if matches.is_present("build") {
        DepKind::Build
    } else {
        DepKind::Normal
    };

    let mut table = DepTable::new().set_kind(kind);

    if let Some(target) = matches.value_of("target") {
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
