use cargo::core::{features, CliUnstable};
use cargo::{self, drop_print, drop_println, CliResult, Config};
use clap::{AppSettings, Arg, ArgMatches};
use itertools::Itertools;

use super::commands;
use super::list_commands;
use crate::command_prelude::*;
use cargo::core::features::HIDDEN;

pub fn main(config: &mut Config) -> CliResult {
    // CAUTION: Be careful with using `config` until it is configured below.
    // In general, try to avoid loading config values unless necessary (like
    // the [alias] table).

    if commands::help::handle_embedded_help(config) {
        return Ok(());
    }

    let args = match cli().get_matches_safe() {
        Ok(args) => args,
        Err(e) => {
            if e.kind == clap::ErrorKind::UnrecognizedSubcommand {
                // An unrecognized subcommand might be an external subcommand.
                let cmd = &e.info.as_ref().unwrap()[0].to_owned();
                return super::execute_external_subcommand(config, cmd, &[cmd, "--help"])
                    .map_err(|_| e.into());
            } else {
                return Err(e.into());
            }
        }
    };

    if args.value_of("unstable-features") == Some("help") {
        let options = CliUnstable::help();
        let non_hidden_options: Vec<(String, String)> = options
            .iter()
            .filter(|(_, help_message)| *help_message != HIDDEN)
            .map(|(name, help)| (name.to_string(), help.to_string()))
            .collect();
        let longest_option = non_hidden_options
            .iter()
            .map(|(option_name, _)| option_name.len())
            .max()
            .unwrap_or(0);
        let help_lines: Vec<String> = non_hidden_options
            .iter()
            .map(|(option_name, option_help_message)| {
                let option_name_kebab_case = option_name.replace("_", "-");
                let padding = " ".repeat(longest_option - option_name.len()); // safe to substract
                format!(
                    "    -Z {}{} -- {}",
                    option_name_kebab_case, padding, option_help_message
                )
            })
            .collect();
        let joined = help_lines.join("\n");
        drop_println!(
            config,
            "
Available unstable (nightly-only) flags:

{}

Run with 'cargo -Z [FLAG] [SUBCOMMAND]'",
            joined
        );
        if !config.nightly_features_allowed {
            drop_println!(
                config,
                "\nUnstable flags are only available on the nightly channel \
                 of Cargo, but this is the `{}` channel.\n\
                 {}",
                features::channel(),
                features::SEE_CHANNELS
            );
        }
        drop_println!(
            config,
            "\nSee https://doc.rust-lang.org/nightly/cargo/reference/unstable.html \
             for more information about these flags."
        );
        return Ok(());
    }

    let is_verbose = args.occurrences_of("verbose") > 0;
    if args.is_present("version") {
        let version = get_version_string(is_verbose);
        drop_print!(config, "{}", version);
        return Ok(());
    }

    if let Some(code) = args.value_of("explain") {
        let mut procss = config.load_global_rustc(None)?.process();
        procss.arg("--explain").arg(code).exec()?;
        return Ok(());
    }

    if args.is_present("list") {
        drop_println!(config, "Installed Commands:");
        for command in list_commands(config) {
            match command {
                CommandInfo::BuiltIn { name, about } => {
                    let summary = about.unwrap_or_default();
                    let summary = summary.lines().next().unwrap_or(&summary); // display only the first line
                    drop_println!(config, "    {:<20} {}", name, summary);
                }
                CommandInfo::External { name, path } => {
                    if is_verbose {
                        drop_println!(config, "    {:<20} {}", name, path.display());
                    } else {
                        drop_println!(config, "    {}", name);
                    }
                }
            }
        }
        return Ok(());
    }

    // Global args need to be extracted before expanding aliases because the
    // clap code for extracting a subcommand discards global options
    // (appearing before the subcommand).
    let (expanded_args, global_args) = expand_aliases(config, args)?;
    let (cmd, subcommand_args) = match expanded_args.subcommand() {
        (cmd, Some(args)) => (cmd, args),
        _ => {
            // No subcommand provided.
            cli().print_help()?;
            return Ok(());
        }
    };
    config_configure(config, &expanded_args, subcommand_args, global_args)?;
    super::init_git_transports(config);

    execute_subcommand(config, cmd, subcommand_args)
}

pub fn get_version_string(is_verbose: bool) -> String {
    let version = cargo::version();
    let mut version_string = version.to_string();
    version_string.push('\n');
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
) -> Result<(ArgMatches<'static>, GlobalArgs), CliError> {
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
            (Some(_), None) => {
                // Command is built-in and is not conflicting with alias, but contains ignored values.
                if let Some(mut values) = args.values_of("") {
                    config.shell().warn(format!(
                        "trailing arguments after built-in command `{}` are ignored: `{}`",
                        cmd,
                        values.join(" "),
                    ))?;
                }
            }
            (None, None) => {}
            (_, Some(mut alias)) => {
                alias.extend(
                    args.values_of("")
                        .unwrap_or_default()
                        .map(|s| s.to_string()),
                );
                // new_args strips out everything before the subcommand, so
                // capture those global options now.
                // Note that an alias to an external command will not receive
                // these arguments. That may be confusing, but such is life.
                let global_args = GlobalArgs::new(args);
                let new_args = cli()
                    .setting(AppSettings::NoBinaryName)
                    .get_matches_from_safe(alias)?;
                let (expanded_args, _) = expand_aliases(config, new_args)?;
                return Ok((expanded_args, global_args));
            }
        }
    };

    Ok((args, GlobalArgs::default()))
}

fn config_configure(
    config: &mut Config,
    args: &ArgMatches<'_>,
    subcommand_args: &ArgMatches<'_>,
    global_args: GlobalArgs,
) -> CliResult {
    let arg_target_dir = &subcommand_args.value_of_path("target-dir", config);
    let verbose = global_args.verbose + args.occurrences_of("verbose") as u32;
    // quiet is unusual because it is redefined in some subcommands in order
    // to provide custom help text.
    let quiet =
        args.is_present("quiet") || subcommand_args.is_present("quiet") || global_args.quiet;
    let global_color = global_args.color; // Extract so it can take reference.
    let color = args.value_of("color").or_else(|| global_color.as_deref());
    let frozen = args.is_present("frozen") || global_args.frozen;
    let locked = args.is_present("locked") || global_args.locked;
    let offline = args.is_present("offline") || global_args.offline;
    let mut unstable_flags = global_args.unstable_flags;
    if let Some(values) = args.values_of("unstable-features") {
        unstable_flags.extend(values.map(|s| s.to_string()));
    }
    let mut config_args = global_args.config_args;
    if let Some(values) = args.values_of("config") {
        config_args.extend(values.map(|s| s.to_string()));
    }
    config.configure(
        verbose,
        quiet,
        color,
        frozen,
        locked,
        offline,
        arg_target_dir,
        &unstable_flags,
        &config_args,
    )?;
    Ok(())
}

fn execute_subcommand(
    config: &mut Config,
    cmd: &str,
    subcommand_args: &ArgMatches<'_>,
) -> CliResult {
    if let Some(exec) = commands::builtin_exec(cmd) {
        return exec(config, subcommand_args);
    }

    let mut ext_args: Vec<&str> = vec![cmd];
    ext_args.extend(subcommand_args.values_of("").unwrap_or_default());
    super::execute_external_subcommand(config, cmd, &ext_args)
}

#[derive(Default)]
struct GlobalArgs {
    verbose: u32,
    quiet: bool,
    color: Option<String>,
    frozen: bool,
    locked: bool,
    offline: bool,
    unstable_flags: Vec<String>,
    config_args: Vec<String>,
}

impl GlobalArgs {
    fn new(args: &ArgMatches<'_>) -> GlobalArgs {
        GlobalArgs {
            verbose: args.occurrences_of("verbose") as u32,
            quiet: args.is_present("quiet"),
            color: args.value_of("color").map(|s| s.to_string()),
            frozen: args.is_present("frozen"),
            locked: args.is_present("locked"),
            offline: args.is_present("offline"),
            unstable_flags: args
                .values_of_lossy("unstable-features")
                .unwrap_or_default(),
            config_args: args
                .values_of("config")
                .unwrap_or_default()
                .map(|s| s.to_string())
                .collect(),
        }
    }
}

fn cli() -> App {
    let is_rustup = std::env::var_os("RUSTUP_HOME").is_some();
    let usage = if is_rustup {
        "cargo [+toolchain] [OPTIONS] [SUBCOMMAND]"
    } else {
        "cargo [OPTIONS] [SUBCOMMAND]"
    };
    App::new("cargo")
        .settings(&[
            AppSettings::UnifiedHelpMessage,
            AppSettings::DeriveDisplayOrder,
            AppSettings::VersionlessSubcommands,
            AppSettings::AllowExternalSubcommands,
        ])
        .usage(usage)
        .template(
            "\
Rust's package manager

USAGE:
    {usage}

OPTIONS:
{unified}

Some common cargo commands are (see all commands with --list):
    build, b    Compile the current package
    check, c    Analyze the current package and report errors, but don't build object files
    clean       Remove the target directory
    doc, d      Build this package's and its dependencies' documentation
    new         Create a new cargo package
    init        Create a new cargo package in an existing directory
    run, r      Run a binary or example of the local package
    test, t     Run the tests
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
        .arg(opt("offline", "Run without accessing the network").global(true))
        .arg(
            multi_opt(
                "config",
                "KEY=VALUE",
                "Override a configuration value (unstable)",
            )
            .global(true),
        )
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
