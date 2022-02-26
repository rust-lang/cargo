use anyhow::anyhow;
use cargo::core::{features, CliUnstable};
use cargo::{self, drop_print, drop_println, CliResult, Config};
use clap::{
    error::{ContextKind, ContextValue},
    AppSettings, Arg, ArgMatches,
};
use itertools::Itertools;
use std::collections::HashMap;
use std::fmt::Write;

use super::commands;
use super::list_commands;
use crate::command_prelude::*;
use cargo::core::features::HIDDEN;

lazy_static::lazy_static! {
    // Maps from commonly known external commands (not builtin to cargo) to their
    // description, for the help page. Reserved for external subcommands that are
    // core within the rust ecosystem (esp ones that might become internal in the future).
    static ref KNOWN_EXTERNAL_COMMAND_DESCRIPTIONS: HashMap<&'static str, &'static str> = HashMap::from([
        ("clippy", "Checks a package to catch common mistakes and improve your Rust code."),
        ("fmt", "Formats all bin and lib files of the current crate using rustfmt."),
    ]);
}

pub fn main(config: &mut Config) -> CliResult {
    // CAUTION: Be careful with using `config` until it is configured below.
    // In general, try to avoid loading config values unless necessary (like
    // the [alias] table).

    if commands::help::handle_embedded_help(config) {
        return Ok(());
    }

    let args = match cli().try_get_matches() {
        Ok(args) => args,
        Err(e) => {
            if e.kind() == clap::ErrorKind::UnrecognizedSubcommand {
                // An unrecognized subcommand might be an external subcommand.
                let cmd = e
                    .context()
                    .find_map(|c| match c {
                        (ContextKind::InvalidSubcommand, &ContextValue::String(ref cmd)) => {
                            Some(cmd)
                        }
                        _ => None,
                    })
                    .expect("UnrecognizedSubcommand implies the presence of InvalidSubcommand");
                return super::execute_external_subcommand(config, cmd, &[cmd, "--help"])
                    .map_err(|_| e.into());
            } else {
                return Err(e.into());
            }
        }
    };

    // Global args need to be extracted before expanding aliases because the
    // clap code for extracting a subcommand discards global options
    // (appearing before the subcommand).
    let (expanded_args, global_args) = expand_aliases(config, args, vec![])?;

    if expanded_args.value_of("unstable-features") == Some("help") {
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
                let padding = " ".repeat(longest_option - option_name.len()); // safe to subtract
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

    let is_verbose = expanded_args.occurrences_of("verbose") > 0;
    if expanded_args.is_present("version") {
        let version = get_version_string(is_verbose);
        drop_print!(config, "{}", version);
        return Ok(());
    }

    if let Some(code) = expanded_args.value_of("explain") {
        let mut procss = config.load_global_rustc(None)?.process();
        procss.arg("--explain").arg(code).exec()?;
        return Ok(());
    }

    if expanded_args.is_present("list") {
        drop_println!(config, "Installed Commands:");
        for (name, command) in list_commands(config) {
            let known_external_desc = KNOWN_EXTERNAL_COMMAND_DESCRIPTIONS.get(name.as_str());
            match command {
                CommandInfo::BuiltIn { about } => {
                    assert!(
                        known_external_desc.is_none(),
                        "KNOWN_EXTERNAL_COMMANDS shouldn't contain builtin \"{}\"",
                        name
                    );
                    let summary = about.unwrap_or_default();
                    let summary = summary.lines().next().unwrap_or(&summary); // display only the first line
                    drop_println!(config, "    {:<20} {}", name, summary);
                }
                CommandInfo::External { path } => {
                    if let Some(desc) = known_external_desc {
                        drop_println!(config, "    {:<20} {}", name, desc);
                    } else if is_verbose {
                        drop_println!(config, "    {:<20} {}", name, path.display());
                    } else {
                        drop_println!(config, "    {}", name);
                    }
                }
                CommandInfo::Alias { target } => {
                    drop_println!(
                        config,
                        "    {:<20} alias: {}",
                        name,
                        target.iter().join(" ")
                    );
                }
            }
        }
        return Ok(());
    }

    let (cmd, subcommand_args) = match expanded_args.subcommand() {
        Some((cmd, args)) => (cmd, args),
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
    let mut version_string = format!("cargo {}\n", version);
    if is_verbose {
        version_string.push_str(&format!("release: {}\n", version.version));
        if let Some(ref ci) = version.commit_info {
            version_string.push_str(&format!("commit-hash: {}\n", ci.commit_hash));
            version_string.push_str(&format!("commit-date: {}\n", ci.commit_date));
        }
        writeln!(version_string, "host: {}", env!("RUST_HOST_TARGET")).unwrap();
        add_libgit2(&mut version_string);
        add_curl(&mut version_string);
        add_ssl(&mut version_string);
        writeln!(version_string, "os: {}", os_info::get()).unwrap();
    }
    version_string
}

fn add_libgit2(version_string: &mut String) {
    let git2_v = git2::Version::get();
    let lib_v = git2_v.libgit2_version();
    let vendored = if git2_v.vendored() {
        format!("vendored")
    } else {
        format!("system")
    };
    writeln!(
        version_string,
        "libgit2: {}.{}.{} (sys:{} {})",
        lib_v.0,
        lib_v.1,
        lib_v.2,
        git2_v.crate_version(),
        vendored
    )
    .unwrap();
}

fn add_curl(version_string: &mut String) {
    let curl_v = curl::Version::get();
    let vendored = if curl_v.vendored() {
        format!("vendored")
    } else {
        format!("system")
    };
    writeln!(
        version_string,
        "libcurl: {} (sys:{} {} ssl:{})",
        curl_v.version(),
        curl_sys::rust_crate_version(),
        vendored,
        curl_v.ssl_version().unwrap_or("none")
    )
    .unwrap();
}

fn add_ssl(version_string: &mut String) {
    #[cfg(feature = "openssl")]
    {
        writeln!(version_string, "ssl: {}", openssl::version::version()).unwrap();
    }
    #[cfg(not(feature = "openssl"))]
    {
        let _ = version_string; // Silence unused warning.
    }
}

fn expand_aliases(
    config: &mut Config,
    args: ArgMatches,
    mut already_expanded: Vec<String>,
) -> Result<(ArgMatches, GlobalArgs), CliError> {
    if let Some((cmd, args)) = args.subcommand() {
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
                // Check if this alias is shadowing an external subcommand
                // (binary of the form `cargo-<subcommand>`)
                // Currently this is only a warning, but after a transition period this will become
                // a hard error.
                if let Some(path) = super::find_external_subcommand(config, cmd) {
                    config.shell().warn(format!(
                        "\
user-defined alias `{}` is shadowing an external subcommand found at: `{}`
This was previously accepted but is being phased out; it will become a hard error in a future release.
For more information, see issue #10049 <https://github.com/rust-lang/cargo/issues/10049>.",
                        cmd,
                        path.display(),
                    ))?;
                }

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
                let new_args = cli().no_binary_name(true).try_get_matches_from(alias)?;

                let new_cmd = new_args.subcommand_name().expect("subcommand is required");
                already_expanded.push(cmd.to_string());
                if already_expanded.contains(&new_cmd.to_string()) {
                    // Crash if the aliases are corecursive / unresolvable
                    return Err(anyhow!(
                        "alias {} has unresolvable recursive definition: {} -> {}",
                        already_expanded[0],
                        already_expanded.join(" -> "),
                        new_cmd,
                    )
                    .into());
                }

                let (expanded_args, _) = expand_aliases(config, new_args, already_expanded)?;
                return Ok((expanded_args, global_args));
            }
        }
    };

    Ok((args, GlobalArgs::default()))
}

fn config_configure(
    config: &mut Config,
    args: &ArgMatches,
    subcommand_args: &ArgMatches,
    global_args: GlobalArgs,
) -> CliResult {
    let arg_target_dir = &subcommand_args
        ._is_valid_arg("target-dir")
        .then(|| subcommand_args.value_of_path("target-dir", config))
        .flatten();
    let verbose = global_args.verbose + args.occurrences_of("verbose") as u32;
    // quiet is unusual because it is redefined in some subcommands in order
    // to provide custom help text.
    let quiet = args.is_present("quiet")
        || subcommand_args.is_valid_and_present("quiet")
        || global_args.quiet;
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

fn execute_subcommand(config: &mut Config, cmd: &str, subcommand_args: &ArgMatches) -> CliResult {
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
    fn new(args: &ArgMatches) -> GlobalArgs {
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
        .allow_external_subcommands(true)
        .setting(AppSettings::DeriveDisplayOrder | AppSettings::NoAutoVersion)
        // Doesn't mix well with our list of common cargo commands.  See clap-rs/clap#3108 for
        // opening clap up to allow us to style our help template
        .disable_colored_help(true)
        .override_usage(usage)
        .help_template(
            "\
Rust's package manager

USAGE:
    {usage}

OPTIONS:
{options}

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
        .arg(opt("version", "Print version info and exit").short('V'))
        .arg(opt("list", "List installed commands"))
        .arg(opt("explain", "Run `rustc --explain CODE`").value_name("CODE"))
        .arg(
            opt(
                "verbose",
                "Use verbose output (-vv very verbose/build.rs output)",
            )
            .short('v')
            .multiple_occurrences(true)
            .global(true),
        )
        .arg_quiet()
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
            Arg::new("unstable-features")
                .help("Unstable (nightly-only) flags to Cargo, see 'cargo -Z help' for details")
                .short('Z')
                .value_name("FLAG")
                .multiple_occurrences(true)
                .global(true),
        )
        .subcommands(commands::builtin())
}

#[test]
fn verify_cli() {
    cli().debug_assert();
}
