extern crate clap;

use clap::{AppSettings, Arg, ArgMatches};

use cargo::{self, CliResult, Config};

use super::list_commands;
use super::commands;
use command_prelude::*;

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
    -Z compile-progress -- Display a progress bar while compiling

Run with 'cargo -Z [FLAG] [SUBCOMMAND]'"
        );
        return Ok(());
    }

    let is_verbose = args.occurrences_of("verbose") > 0;
    if args.is_present("version") {
        let version = cargo::version();
        println!("{}", version);
        if is_verbose {
            println!(
                "release: {}.{}.{}",
                version.major, version.minor, version.patch
            );
            if let Some(ref cfg) = version.cfg_info {
                if let Some(ref ci) = cfg.commit_info {
                    println!("commit-hash: {}", ci.commit_hash);
                    println!("commit-date: {}", ci.commit_date);
                }
            }
        }
        return Ok(());
    }

    if let Some(ref code) = args.value_of("explain") {
        let mut procss = config.rustc(None)?.process();
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

fn expand_aliases(
    config: &mut Config,
    args: ArgMatches<'static>,
) -> Result<ArgMatches<'static>, CliError> {
    if let (cmd, Some(args)) = args.subcommand() {
        match (
            commands::builtin_exec(cmd),
            super::aliased_command(config, cmd)?,
        ) {
            (None, Some(mut alias)) => {
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
            (Some(_), Some(_)) => {
                config.shell().warn(format!(
                    "alias `{}` is ignored, because it is shadowed by a built in command",
                    cmd
                ))?;
            }
            (_, None) => {}
        }
    };
    Ok(args)
}

fn execute_subcommand(config: &mut Config, args: &ArgMatches) -> CliResult {
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
        if args.is_present("quiet") {
            Some(true)
        } else {
            None
        },
        &args.value_of("color").map(|s| s.to_string()),
        args.is_present("frozen"),
        args.is_present("locked"),
        arg_target_dir,
        &args.values_of_lossy("unstable-features")
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
    build       Compile the current project
    check       Analyze the current project and report errors, but don't build object files
    clean       Remove the target directory
    doc         Build this project's and its dependencies' documentation
    new         Create a new cargo project
    init        Create a new cargo project in an existing directory
    run         Build and execute src/main.rs
    test        Run the tests
    bench       Run the benchmarks
    update      Update dependencies listed in Cargo.lock
    search      Search registry for crates
    publish     Package and upload this project to the registry
    install     Install a Rust binary
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
            ).short("v")
                .multiple(true)
                .global(true),
        )
        .arg(
            opt("quiet", "No output printed to stdout")
                .short("q")
                .global(true),
        )
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
