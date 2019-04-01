use clap;

use clap::{AppSettings, Arg, ArgMatches};

use cargo::core::features;
use cargo::{self, CliResult, Config};

use super::commands;
use super::list_commands;
use crate::command_prelude::*;

pub fn main(config: &mut Config) -> CliResult {
    let args = match cli().get_matches_safe() {
        Ok(args) => args,
        Err(e) => {
            if e.kind == clap::ErrorKind::UnrecognizedSubcommand {
                // An unrecognized subcommand might be an external subcommand.
                let cmd = &e.info.as_ref().unwrap()[0].to_owned();
                return super::execute_external_subcommand(config, cmd, &[cmd, "--help"])
                    .map_err(|_| e.into());
            } else {
                return Err(e)?;
            }
        }
    };

    if args.value_of("unstable-features") == Some("help") {
        println!(
            "
Available unstable (nightly-only) flags:

    -Z avoid-dev-deps   -- Avoid installing dev-dependencies if possible
    -Z minimal-versions -- Install minimal dependency versions instead of maximum
    -Z no-index-update  -- Do not update the registry, avoids a network request for benchmarking
    -Z offline          -- Offline mode that does not perform network requests
    -Z unstable-options -- Allow the usage of unstable options such as --registry
    -Z config-profile   -- Read profiles from .cargo/config files

Run with 'cargo -Z [FLAG] [SUBCOMMAND]'"
        );
        if !features::nightly_features_allowed() {
            println!(
                "\nUnstable flags are only available on the nightly channel \
                 of Cargo, but this is the `{}` channel.\n\
                 {}",
                features::channel(),
                features::SEE_CHANNELS
            );
        }
        println!(
            "\nSee https://doc.rust-lang.org/nightly/cargo/reference/unstable.html \
             for more information about these flags."
        );
        return Ok(());
    }

    let is_verbose = args.occurrences_of("verbose") > 0;
    if args.is_present("version") {
        let version = get_version_string(is_verbose);
        print!("{}", version);
        return Ok(());
    }

    if let Some(code) = args.value_of("explain") {
        let mut procss = config.load_global_rustc(None)?.process();
        procss.arg("--explain").arg(code).exec()?;
        return Ok(());
    }

    if args.is_present("list") {
        println!("Installed Commands:");
        for command in list_commands(config) {
            match command {
                CommandInfo::BuiltIn { name, about } => {
                    let summary = about.unwrap_or_default();
                    let summary = summary.lines().next().unwrap_or(&summary); // display only the first line
                    println!("    {:<20} {}", name, summary)
                }
                CommandInfo::External { name, path } => {
                    if is_verbose {
                        println!("    {:<20} {}", name, path.display())
                    } else {
                        println!("    {}", name)
                    }
                }
            }
        }
        return Ok(());
    }

    let args = expand_aliases(config, args)?;

    execute_subcommand(config, &args)
}

pub fn get_version_string(is_verbose: bool) -> String {
    let version = cargo::version();
    let mut version_string = version.to_string();
    version_string.push_str("\n");
    if is_verbose {
        version_string.push_str(&format!(
            "release: {}.{}.{}\n",
            version.major, version.minor, version.patch
        ));
        if let Some(ref cfg) = version.cfg_info {
            if let Some(ref ci) = cfg.commit_info {
                version_string.push_str(&format!("commit-hash: {}\n", ci.commit_hash));
                version_string.push_str(&format!("commit-date: {}\n", ci.commit_date));
            }
        }
    }
    version_string
}

fn expand_aliases(
    config: &mut Config,
    args: ArgMatches<'static>,
) -> Result<ArgMatches<'static>, CliError> {
    if let (cmd, Some(args)) = args.subcommand() {
        match (
            commands::builtin_exec(cmd),
            super::aliased_command(config, cmd)?,
        ) {
            (Some(_), Some(_)) => {
                // User alias conflicts with a built-in subcommand
                config.shell().warn(format!(
                    "user-defined alias `{}` is ignored, because it is shadowed by a built-in command",
                    cmd,
                ))?;
            }
            (_, Some(mut alias)) => {
                alias.extend(
                    args.values_of("")
                        .unwrap_or_default()
                        .map(|s| s.to_string()),
                );
                let args = cli()
                    .setting(AppSettings::NoBinaryName)
                    .get_matches_from_safe(alias)?;
                return expand_aliases(config, args);
            }
            (_, None) => {}
        }
    };

    Ok(args)
}

fn execute_subcommand(config: &mut Config, args: &ArgMatches<'_>) -> CliResult {
    let (cmd, subcommand_args) = match args.subcommand() {
        (cmd, Some(args)) => (cmd, args),
        _ => {
            cli().print_help()?;
            return Ok(());
        }
    };

    let arg_target_dir = &subcommand_args.value_of_path("target-dir", config);

    config.configure(
        args.occurrences_of("verbose") as u32,
        if args.is_present("quiet") || subcommand_args.is_present("quiet") {
            Some(true)
        } else {
            None
        },
        &args.value_of("color").map(|s| s.to_string()),
        args.is_present("frozen"),
        args.is_present("locked"),
        arg_target_dir,
        &args
            .values_of_lossy("unstable-features")
            .unwrap_or_default(),
    )?;

    if let Some(exec) = commands::builtin_exec(cmd) {
        return exec(config, subcommand_args);
    }

    let mut ext_args: Vec<&str> = vec![cmd];
    ext_args.extend(subcommand_args.values_of("").unwrap_or_default());
    super::execute_external_subcommand(config, cmd, &ext_args)
}

fn cli() -> App {
    App::new("cargo")
        .settings(&[
            AppSettings::UnifiedHelpMessage,
            AppSettings::DeriveDisplayOrder,
            AppSettings::VersionlessSubcommands,
            AppSettings::AllowExternalSubcommands,
        ])
        .about("")
        .template(
            "\
Rust's package manager

USAGE:
    {usage}

OPTIONS:
{unified}

Some common cargo commands are (see all commands with --list):
    build       Compile the current package
    check       Analyze the current package and report errors, but don't build object files
    clean       Remove the target directory
    doc         Build this package's and its dependencies' documentation
    new         Create a new cargo package
    init        Create a new cargo package in an existing directory
    run         Run a binary or example of the local package
    test        Run the tests
    bench       Run the benchmarks
    update      Update dependencies listed in Cargo.lock
    search      Search registry for crates
    publish     Package and upload this package to the registry
    install     Install a Rust binary. Default location is $HOME/.cargo/bin
    uninstall   Uninstall a Rust binary

See 'cargo help <command>' for more information on a specific command.\n",
        )
        .arg(opt("version", "Print version info and exit").short("V"))
        .arg(opt("list", "List installed commands"))
        .arg(opt("explain", "Run `rustc --explain CODE`").value_name("CODE"))
        .arg(
            opt(
                "verbose",
                "Use verbose output (-vv very verbose/build.rs output)",
            )
            .short("v")
            .multiple(true)
            .global(true),
        )
        .arg(opt("quiet", "No output printed to stdout").short("q"))
        .arg(
            opt("color", "Coloring: auto, always, never")
                .value_name("WHEN")
                .global(true),
        )
        .arg(opt("frozen", "Require Cargo.lock and cache are up to date").global(true))
        .arg(opt("locked", "Require Cargo.lock is up to date").global(true))
        .arg(
            Arg::with_name("unstable-features")
                .help("Unstable (nightly-only) flags to Cargo, see 'cargo -Z help' for details")
                .short("Z")
                .value_name("FLAG")
                .multiple(true)
                .number_of_values(1)
                .global(true),
        )
        .subcommands(commands::builtin())
}
