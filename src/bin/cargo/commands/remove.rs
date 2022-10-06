use cargo::core::dependency::DepKind;
use cargo::ops::cargo_remove::remove;
use cargo::ops::cargo_remove::RemoveOptions;
use cargo::ops::resolve_ws;
use cargo::util::command_prelude::*;
use cargo::util::toml_mut::manifest::DepTable;

pub fn cli() -> clap::Command {
    clap::Command::new("remove")
        // Subcommand aliases are handled in `aliased_command()`.
        // .alias("rm")
        .about("Remove dependencies from a Cargo.toml manifest file")
        .args([clap::Arg::new("dependencies")
            .action(clap::ArgAction::Append)
            .required(true)
            .num_args(1..)
            .value_name("DEP_ID")
            .help("Dependencies to be removed")])
        .arg_package("Package to remove from")
        .arg_manifest_path()
        .arg_quiet()
        .arg_dry_run("Don't actually write the manifest")
        .next_help_heading("SECTION")
        .args([
            clap::Arg::new("dev")
                .long("dev")
                .conflicts_with("build")
                .action(clap::ArgAction::SetTrue)
                .group("section")
                .help("Remove as development dependency"),
            clap::Arg::new("build")
                .long("build")
                .conflicts_with("dev")
                .action(clap::ArgAction::SetTrue)
                .group("section")
                .help("Remove as build dependency"),
            clap::Arg::new("target")
                .long("target")
                .num_args(1)
                .value_name("TARGET")
                .value_parser(clap::builder::NonEmptyStringValueParser::new())
                .help("Remove as dependency from the given target platform"),
        ])
}

pub fn exec(config: &mut Config, args: &ArgMatches) -> CliResult {
    let dry_run = args.dry_run();

    let workspace = args.workspace(config)?;
    let packages = args.packages_from_flags()?;
    let packages = packages.get_packages(&workspace)?;
    let spec = match packages.len() {
        0 => {
            return Err(CliError::new(
                anyhow::format_err!("no packages selected. Please specify one with `-p <PKG_ID>`"),
                101,
            ));
        }
        1 => packages[0],
        len => {
            return Err(CliError::new(
                anyhow::format_err!(
                    "{len} packages selected. Please specify one with `-p <PKG_ID>`",
                ),
                101,
            ));
        }
    };

    let dependencies = args
        .get_many::<String>("dependencies")
        .expect("required(true)")
        .cloned()
        .collect();

    let section = parse_section(args);

    let options = RemoveOptions {
        config,
        spec,
        dependencies,
        section,
        dry_run,
    };
    remove(&options)?;

    if !dry_run {
        // Reload the workspace since we've changed dependencies
        let ws = args.workspace(config)?;
        resolve_ws(&ws)?;
    }

    Ok(())
}

fn parse_section(args: &ArgMatches) -> DepTable {
    let dev = args.flag("dev");
    let build = args.flag("build");

    let kind = if dev {
        DepKind::Development
    } else if build {
        DepKind::Build
    } else {
        DepKind::Normal
    };

    let mut table = DepTable::new().set_kind(kind);

    if let Some(target) = args.get_one::<String>("target") {
        assert!(!target.is_empty(), "Target specification may not be empty");
        table = table.set_target(target);
    }

    table
}
