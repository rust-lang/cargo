use anyhow::{anyhow, Context as _};
use cargo::core::shell::Shell;
use cargo::core::{features, CliUnstable};
use cargo::{self, drop_print, drop_println, CliResult, Config};
use clap::{Arg, ArgMatches};
use itertools::Itertools;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::ffi::OsString;
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

pub fn main(config: &mut LazyConfig) -> CliResult {
    let args = cli().try_get_matches()?;

    // Update the process-level notion of cwd
    // This must be completed before config is initialized
    assert_eq!(config.is_init(), false);
    if let Some(new_cwd) = args.get_one::<std::path::PathBuf>("directory") {
        // This is a temporary hack. This cannot access `Config`, so this is a bit messy.
        // This does not properly parse `-Z` flags that appear after the subcommand.
        // The error message is not as helpful as the standard one.
        let nightly_features_allowed = matches!(&*features::channel(), "nightly" | "dev");
        if !nightly_features_allowed
            || (nightly_features_allowed
                && !args
                    .get_many("unstable-features")
                    .map(|mut z| z.any(|value: &String| value == "unstable-options"))
                    .unwrap_or(false))
        {
            return Err(anyhow::format_err!(
                "the `-C` flag is unstable, \
                 pass `-Z unstable-options` on the nightly channel to enable it"
            )
            .into());
        }
        std::env::set_current_dir(&new_cwd).context("could not change to requested directory")?;
    }

    // CAUTION: Be careful with using `config` until it is configured below.
    // In general, try to avoid loading config values unless necessary (like
    // the [alias] table).
    let config = config.get_mut();

    let (expanded_args, global_args) = expand_aliases(config, args, vec![])?;

    if expanded_args
        .get_one::<String>("unstable-features")
        .map(String::as_str)
        == Some("help")
    {
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

Run with 'cargo -Z [FLAG] [COMMAND]'",
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

    let is_verbose = expanded_args.verbose() > 0;
    if expanded_args.flag("version") {
        let version = get_version_string(is_verbose);
        drop_print!(config, "{}", version);
        return Ok(());
    }

    if let Some(code) = expanded_args.get_one::<String>("explain") {
        let mut procss = config.load_global_rustc(None)?.process();
        procss.arg("--explain").arg(code).exec()?;
        return Ok(());
    }

    if expanded_args.flag("list") {
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
    super::init_git(config);

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

/// Expands aliases recursively to collect all the command line arguments.
///
/// [`GlobalArgs`] need to be extracted before expanding aliases because the
/// clap code for extracting a subcommand discards global options
/// (appearing before the subcommand).
fn expand_aliases(
    config: &mut Config,
    args: ArgMatches,
    mut already_expanded: Vec<String>,
) -> Result<(ArgMatches, GlobalArgs), CliError> {
    if let Some((cmd, args)) = args.subcommand() {
        let exec = commands::builtin_exec(cmd);
        let aliased_cmd = super::aliased_command(config, cmd);

        match (exec, aliased_cmd) {
            (Some(_), Ok(Some(_))) => {
                // User alias conflicts with a built-in subcommand
                config.shell().warn(format!(
                    "user-defined alias `{}` is ignored, because it is shadowed by a built-in command",
                    cmd,
                ))?;
            }
            (Some(_), Ok(None) | Err(_)) => {
                // Here we ignore errors from aliasing as we already favor built-in command,
                // and alias doesn't involve in this context.

                if let Some(values) = args.get_many::<OsString>("") {
                    // Command is built-in and is not conflicting with alias, but contains ignored values.
                    return Err(anyhow::format_err!(
                        "\
trailing arguments after built-in command `{}` are unsupported: `{}`

To pass the arguments to the subcommand, remove `--`",
                        cmd,
                        values.map(|s| s.to_string_lossy()).join(" "),
                    )
                    .into());
                }
            }
            (None, Ok(None)) => {}
            (None, Ok(Some(alias))) => {
                // Check if a user-defined alias is shadowing an external subcommand
                // (binary of the form `cargo-<subcommand>`)
                // Currently this is only a warning, but after a transition period this will become
                // a hard error.
                if super::builtin_aliases_execs(cmd).is_none() {
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
                }

                let mut alias = alias
                    .into_iter()
                    .map(|s| OsString::from(s))
                    .collect::<Vec<_>>();
                alias.extend(args.get_many::<OsString>("").unwrap_or_default().cloned());
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
            (None, Err(e)) => return Err(e.into()),
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
    let arg_target_dir = &subcommand_args.value_of_path("target-dir", config);
    let verbose = global_args.verbose + args.verbose();
    // quiet is unusual because it is redefined in some subcommands in order
    // to provide custom help text.
    let quiet = args.flag("quiet") || subcommand_args.flag("quiet") || global_args.quiet;
    let global_color = global_args.color; // Extract so it can take reference.
    let color = args
        .get_one::<String>("color")
        .map(String::as_str)
        .or_else(|| global_color.as_deref());
    let frozen = args.flag("frozen") || global_args.frozen;
    let locked = args.flag("locked") || global_args.locked;
    let offline = args.flag("offline") || global_args.offline;
    let mut unstable_flags = global_args.unstable_flags;
    if let Some(values) = args.get_many::<String>("unstable-features") {
        unstable_flags.extend(values.cloned());
    }
    let mut config_args = global_args.config_args;
    if let Some(values) = args.get_many::<String>("config") {
        config_args.extend(values.cloned());
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

    let mut ext_args: Vec<&OsStr> = vec![OsStr::new(cmd)];
    ext_args.extend(
        subcommand_args
            .get_many::<OsString>("")
            .unwrap_or_default()
            .map(OsString::as_os_str),
    );
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
            verbose: args.verbose(),
            quiet: args.flag("quiet"),
            color: args.get_one::<String>("color").cloned(),
            frozen: args.flag("frozen"),
            locked: args.flag("locked"),
            offline: args.flag("offline"),
            unstable_flags: args
                .get_many::<String>("unstable-features")
                .unwrap_or_default()
                .cloned()
                .collect(),
            config_args: args
                .get_many::<String>("config")
                .unwrap_or_default()
                .cloned()
                .collect(),
        }
    }
}

pub fn cli() -> Command {
    // ALLOWED: `RUSTUP_HOME` should only be read from process env, otherwise
    // other tools may point to executables from incompatible distributions.
    #[allow(clippy::disallowed_methods)]
    let is_rustup = std::env::var_os("RUSTUP_HOME").is_some();
    let usage = if is_rustup {
        "cargo [+toolchain] [OPTIONS] [COMMAND]"
    } else {
        "cargo [OPTIONS] [COMMAND]"
    };
    Command::new("cargo")
        // Subcommands all count their args' display order independently (from 0),
        // which makes their args interspersed with global args. This puts global args last.
        .next_display_order(1000)
        .allow_external_subcommands(true)
        // Doesn't mix well with our list of common cargo commands.  See clap-rs/clap#3108 for
        // opening clap up to allow us to style our help template
        .disable_colored_help(true)
        // Provide a custom help subcommand for calling into man pages
        .disable_help_subcommand(true)
        .override_usage(usage)
        .help_template(
            "\
Rust's package manager

Usage: {usage}

Options:
{options}

Some common cargo commands are (see all commands with --list):
    build, b    Compile the current package
    check, c    Analyze the current package and report errors, but don't build object files
    clean       Remove the target directory
    doc, d      Build this package's and its dependencies' documentation
    new         Create a new cargo package
    init        Create a new cargo package in an existing directory
    add         Add dependencies to a manifest file
    remove      Remove dependencies from a manifest file
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
        .arg(flag("version", "Print version info and exit").short('V'))
        .arg(flag("list", "List installed commands"))
        .arg(opt("explain", "Run `rustc --explain CODE`").value_name("CODE"))
        .arg(
            opt(
                "verbose",
                "Use verbose output (-vv very verbose/build.rs output)",
            )
            .short('v')
            .action(ArgAction::Count)
            .global(true),
        )
        .arg_quiet()
        .arg(
            opt("color", "Coloring: auto, always, never")
                .value_name("WHEN")
                .global(true),
        )
        .arg(
            Arg::new("directory")
                .help("Change to DIRECTORY before doing anything (nightly-only)")
                .short('C')
                .value_name("DIRECTORY")
                .value_hint(clap::ValueHint::DirPath)
                .value_parser(clap::builder::ValueParser::path_buf()),
        )
        .arg(flag("frozen", "Require Cargo.lock and cache are up to date").global(true))
        .arg(flag("locked", "Require Cargo.lock is up to date").global(true))
        .arg(flag("offline", "Run without accessing the network").global(true))
        .arg(multi_opt("config", "KEY=VALUE", "Override a configuration value").global(true))
        .arg(
            Arg::new("unstable-features")
                .help("Unstable (nightly-only) flags to Cargo, see 'cargo -Z help' for details")
                .short('Z')
                .value_name("FLAG")
                .action(ArgAction::Append)
                .global(true),
        )
        .subcommands(commands::builtin())
}

/// Delay loading [`Config`] until access.
///
/// In the common path, the [`Config`] is dependent on CLI parsing and shouldn't be loaded until
/// after that is done but some other paths (like fix or earlier errors) might need access to it,
/// so this provides a way to share the instance and the implementation across these different
/// accesses.
pub struct LazyConfig {
    config: Option<Config>,
}

impl LazyConfig {
    pub fn new() -> Self {
        Self { config: None }
    }

    /// Check whether the config is loaded
    ///
    /// This is useful for asserts in case the environment needs to be setup before loading
    pub fn is_init(&self) -> bool {
        self.config.is_some()
    }

    /// Get the config, loading it if needed
    ///
    /// On error, the process is terminated
    pub fn get(&mut self) -> &Config {
        self.get_mut()
    }

    /// Get the config, loading it if needed
    ///
    /// On error, the process is terminated
    pub fn get_mut(&mut self) -> &mut Config {
        self.config.get_or_insert_with(|| match Config::default() {
            Ok(cfg) => cfg,
            Err(e) => {
                let mut shell = Shell::new();
                cargo::exit_with_error(e.into(), &mut shell)
            }
        })
    }
}

#[test]
fn verify_cli() {
    cli().debug_assert();
}
