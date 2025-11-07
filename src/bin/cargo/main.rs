#![allow(clippy::self_named_module_files)] // false positive in `commands/build.rs`

use cargo::core::features;
use cargo::core::shell::Shell;
use cargo::util::network::http::http_handle;
use cargo::util::network::http::needs_custom_http_transport;
use cargo::util::{self, CargoResult, closest_msg, command_prelude};
use cargo_util::{ProcessBuilder, ProcessError};
use cargo_util_schemas::manifest::StringOrVec;
use std::collections::BTreeMap;
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

mod cli;
mod commands;

use crate::command_prelude::*;

fn main() {
    let _guard = setup_logger();

    let mut gctx = match GlobalContext::default() {
        Ok(gctx) => gctx,
        Err(e) => {
            let mut shell = Shell::new();
            cargo::exit_with_error(e.into(), &mut shell)
        }
    };

    let nightly_features_allowed = matches!(&*features::channel(), "nightly" | "dev");
    if nightly_features_allowed {
        let _span = tracing::span!(tracing::Level::TRACE, "completions").entered();
        let args = std::env::args_os();
        let current_dir = std::env::current_dir().ok();
        let completer = clap_complete::CompleteEnv::with_factory(|| {
            let mut gctx = GlobalContext::default().expect("already loaded without errors");
            cli::cli(&mut gctx)
        })
        .var("CARGO_COMPLETE");
        if completer
            .try_complete(args, current_dir.as_deref())
            .unwrap_or_else(|e| {
                let mut shell = Shell::new();
                cargo::exit_with_error(e.into(), &mut shell)
            })
        {
            return;
        }
    }

    let result = if let Some(lock_addr) = cargo::ops::fix_get_proxy_lock_addr() {
        cargo::ops::fix_exec_rustc(&gctx, &lock_addr).map_err(|e| CliError::from(e))
    } else {
        let _token = cargo::util::job::setup();
        cli::main(&mut gctx)
    };

    match result {
        Err(e) => cargo::exit_with_error(e, &mut *gctx.shell()),
        Ok(()) => {}
    }
}

fn setup_logger() -> Option<ChromeFlushGuard> {
    use tracing_subscriber::prelude::*;

    let env = tracing_subscriber::EnvFilter::from_env("CARGO_LOG");
    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_timer(tracing_subscriber::fmt::time::Uptime::default())
        .with_ansi(std::io::IsTerminal::is_terminal(&std::io::stderr()))
        .with_writer(std::io::stderr)
        .with_filter(env);

    let (profile_layer, profile_guard) = chrome_layer();

    let registry = tracing_subscriber::registry()
        .with(fmt_layer)
        .with(profile_layer);
    registry.init();
    tracing::trace!(start = jiff::Timestamp::now().to_string());
    profile_guard
}

#[cfg(target_has_atomic = "64")]
type ChromeFlushGuard = tracing_chrome::FlushGuard;
#[cfg(target_has_atomic = "64")]
fn chrome_layer<S>() -> (
    Option<tracing_chrome::ChromeLayer<S>>,
    Option<ChromeFlushGuard>,
)
where
    S: tracing::Subscriber
        + for<'span> tracing_subscriber::registry::LookupSpan<'span>
        + Send
        + Sync,
{
    #![allow(clippy::disallowed_methods)]

    if env_to_bool(std::env::var_os("CARGO_LOG_PROFILE").as_deref()) {
        let capture_args =
            env_to_bool(std::env::var_os("CARGO_LOG_PROFILE_CAPTURE_ARGS").as_deref());
        let (layer, guard) = tracing_chrome::ChromeLayerBuilder::new()
            .include_args(capture_args)
            .build();
        (Some(layer), Some(guard))
    } else {
        (None, None)
    }
}

#[cfg(not(target_has_atomic = "64"))]
type ChromeFlushGuard = ();
#[cfg(not(target_has_atomic = "64"))]
fn chrome_layer() -> (
    Option<tracing_subscriber::layer::Identity>,
    Option<ChromeFlushGuard>,
) {
    (None, None)
}

#[cfg(target_has_atomic = "64")]
fn env_to_bool(os: Option<&OsStr>) -> bool {
    match os.and_then(|os| os.to_str()) {
        Some("1") | Some("true") => true,
        _ => false,
    }
}

/// Table for defining the aliases which come builtin in `Cargo`.
/// The contents are structured as: `(alias, aliased_command, description)`.
const BUILTIN_ALIASES: [(&str, &str, &str); 6] = [
    ("b", "build", "alias: build"),
    ("c", "check", "alias: check"),
    ("d", "doc", "alias: doc"),
    ("r", "run", "alias: run"),
    ("t", "test", "alias: test"),
    ("rm", "remove", "alias: remove"),
];

/// Function which contains the list of all of the builtin aliases and it's
/// corresponding execs represented as &str.
fn builtin_aliases_execs(cmd: &str) -> Option<&(&str, &str, &str)> {
    BUILTIN_ALIASES.iter().find(|alias| alias.0 == cmd)
}

/// Resolve the aliased command from the [`GlobalContext`] with a given command string.
///
/// The search fallback chain is:
///
/// 1. Get the aliased command as a string.
/// 2. If an `Err` occurs (missing key, type mismatch, or any possible error),
///    try to get it as an array again.
/// 3. If still cannot find any, finds one insides [`BUILTIN_ALIASES`].
fn aliased_command(gctx: &GlobalContext, command: &str) -> CargoResult<Option<Vec<String>>> {
    let alias_name = format!("alias.{}", command);
    let user_alias = match gctx.get_string(&alias_name) {
        Ok(Some(record)) => Some(
            record
                .val
                .split_whitespace()
                .map(|s| s.to_string())
                .collect(),
        ),
        Ok(None) => None,
        Err(_) => gctx.get::<Option<Vec<String>>>(&alias_name)?,
    };

    let result = user_alias.or_else(|| {
        builtin_aliases_execs(command).map(|command_str| vec![command_str.1.to_string()])
    });
    if result
        .as_ref()
        .map(|alias| alias.is_empty())
        .unwrap_or_default()
    {
        anyhow::bail!("subcommand is required, but `{alias_name}` is empty");
    }
    Ok(result)
}

/// List all runnable commands
fn list_commands(gctx: &GlobalContext) -> BTreeMap<String, CommandInfo> {
    let mut commands = third_party_subcommands(gctx);

    for cmd in commands::builtin() {
        commands.insert(
            cmd.get_name().to_string(),
            CommandInfo::BuiltIn {
                about: cmd.get_about().map(|s| s.to_string()),
            },
        );
    }

    // Add the builtin_aliases and them descriptions to the
    // `commands` `BTreeMap`.
    for command in &BUILTIN_ALIASES {
        commands.insert(
            command.0.to_string(),
            CommandInfo::BuiltIn {
                about: Some(command.2.to_string()),
            },
        );
    }

    // Add the user-defined aliases
    let alias_commands = user_defined_aliases(gctx);
    commands.extend(alias_commands);

    // `help` is special, so it needs to be inserted separately.
    commands.insert(
        "help".to_string(),
        CommandInfo::BuiltIn {
            about: Some("Displays help for a cargo command".to_string()),
        },
    );

    commands
}

fn third_party_subcommands(gctx: &GlobalContext) -> BTreeMap<String, CommandInfo> {
    let prefix = "cargo-";
    let suffix = env::consts::EXE_SUFFIX;
    let mut commands = BTreeMap::new();
    for dir in search_directories(gctx) {
        let entries = match fs::read_dir(dir) {
            Ok(entries) => entries,
            _ => continue,
        };
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            let Some(filename) = path.file_name().and_then(|s| s.to_str()) else {
                continue;
            };
            let Some(name) = filename
                .strip_prefix(prefix)
                .and_then(|s| s.strip_suffix(suffix))
            else {
                continue;
            };
            if is_executable(entry.path()) {
                commands.insert(
                    name.to_string(),
                    CommandInfo::External { path: path.clone() },
                );
            }
        }
    }
    commands
}

fn user_defined_aliases(gctx: &GlobalContext) -> BTreeMap<String, CommandInfo> {
    let mut commands = BTreeMap::new();
    if let Ok(aliases) = gctx.get::<BTreeMap<String, StringOrVec>>("alias") {
        for (name, target) in aliases.iter() {
            commands.insert(
                name.to_string(),
                CommandInfo::Alias {
                    target: target.clone(),
                },
            );
        }
    }
    commands
}

fn find_external_subcommand(gctx: &GlobalContext, cmd: &str) -> Option<PathBuf> {
    let command_exe = format!("cargo-{}{}", cmd, env::consts::EXE_SUFFIX);
    search_directories(gctx)
        .iter()
        .map(|dir| dir.join(&command_exe))
        .find(|file| is_executable(file))
}

fn execute_external_subcommand(gctx: &GlobalContext, cmd: &str, args: &[&OsStr]) -> CliResult {
    let path = find_external_subcommand(gctx, cmd);
    let command = match path {
        Some(command) => command,
        None => {
            let script_suggestion = if gctx.cli_unstable().script
                && std::path::Path::new(cmd).is_file()
            {
                let sep = std::path::MAIN_SEPARATOR;
                format!(
                    "\nhelp: To run the file `{cmd}`, provide a relative path like `.{sep}{cmd}`"
                )
            } else {
                "".to_owned()
            };
            let err = if cmd.starts_with('+') {
                anyhow::format_err!(
                    "no such command: `{cmd}`\n\n\
                    help: invoke `cargo` through `rustup` to handle `+toolchain` directives{script_suggestion}",
                )
            } else {
                let suggestions = list_commands(gctx);
                let did_you_mean = closest_msg(cmd, suggestions.keys(), |c| c, "command");

                anyhow::format_err!(
                    "no such command: `{cmd}`{did_you_mean}\n\n\
                    help: view all installed commands with `cargo --list`\n\
                    help: find a package to install `{cmd}` with `cargo search cargo-{cmd}`{script_suggestion}",
                )
            };

            return Err(CliError::new(err, 101));
        }
    };
    execute_subcommand(gctx, Some(&command), args)
}

fn execute_internal_subcommand(gctx: &GlobalContext, args: &[&OsStr]) -> CliResult {
    execute_subcommand(gctx, None, args)
}

// This function is used to execute a subcommand. It is used to execute both
// internal and external subcommands.
// If `cmd_path` is `None`, then the subcommand is an internal subcommand.
fn execute_subcommand(
    gctx: &GlobalContext,
    cmd_path: Option<&PathBuf>,
    args: &[&OsStr],
) -> CliResult {
    let cargo_exe = gctx.cargo_exe()?;
    let mut cmd = match cmd_path {
        Some(cmd_path) => ProcessBuilder::new(cmd_path),
        None => ProcessBuilder::new(&cargo_exe),
    };
    cmd.env(cargo::CARGO_ENV, cargo_exe).args(args);
    if let Some(client) = gctx.jobserver_from_env() {
        cmd.inherit_jobserver(client);
    }
    let err = match cmd.exec_replace() {
        Ok(()) => return Ok(()),
        Err(e) => e,
    };

    if let Some(perr) = err.downcast_ref::<ProcessError>() {
        if let Some(code) = perr.code {
            return Err(CliError::code(code));
        }
    }
    Err(CliError::new(err, 101))
}

#[cfg(unix)]
fn is_executable<P: AsRef<Path>>(path: P) -> bool {
    use std::os::unix::prelude::*;
    fs::metadata(path)
        .map(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}
#[cfg(windows)]
fn is_executable<P: AsRef<Path>>(path: P) -> bool {
    path.as_ref().is_file()
}

fn search_directories(gctx: &GlobalContext) -> Vec<PathBuf> {
    let mut path_dirs = if let Some(val) = gctx.get_env_os("PATH") {
        env::split_paths(&val).collect()
    } else {
        vec![]
    };

    let home_bin = gctx.home().clone().into_path_unlocked().join("bin");

    // If any of that PATH elements contains `home_bin`, do not
    // add it again. This is so that the users can control priority
    // of it using PATH, while preserving the historical
    // behavior of preferring it over system global directories even
    // when not in PATH at all.
    // See https://github.com/rust-lang/cargo/issues/11020 for details.
    //
    // Note: `p == home_bin` will ignore trailing slash, but we don't
    // `canonicalize` the paths.
    if !path_dirs.iter().any(|p| p == &home_bin) {
        path_dirs.insert(0, home_bin);
    };

    path_dirs
}

/// Initialize libgit2.
#[tracing::instrument(skip_all)]
fn init_git(gctx: &GlobalContext) {
    // Disabling the owner validation in git can, in theory, lead to code execution
    // vulnerabilities. However, libgit2 does not launch executables, which is the foundation of
    // the original security issue. Meanwhile, issues with refusing to load git repos in
    // `CARGO_HOME` for example will likely be very frustrating for users. So, we disable the
    // validation.
    //
    // For further discussion of Cargo's current interactions with git, see
    //
    //   https://github.com/rust-lang/rfcs/pull/3279
    //
    // and in particular the subsection on "Git support".
    //
    // Note that we only disable this when Cargo is run as a binary. If Cargo is used as a library,
    // this code won't be invoked. Instead, developers will need to explicitly disable the
    // validation in their code. This is inconvenient, but won't accidentally open consuming
    // applications up to security issues if they use git2 to open repositories elsewhere in their
    // code.
    unsafe {
        git2::opts::set_verify_owner_validation(false)
            .expect("set_verify_owner_validation should never fail");
    }

    init_git_transports(gctx);
}

/// Configure libgit2 to use libcurl if necessary.
///
/// If the user has a non-default network configuration, then libgit2 will be
/// configured to use libcurl instead of the built-in networking support so
/// that those configuration settings can be used.
#[tracing::instrument(skip_all)]
fn init_git_transports(gctx: &GlobalContext) {
    match needs_custom_http_transport(gctx) {
        Ok(true) => {}
        _ => return,
    }

    let handle = match http_handle(gctx) {
        Ok(handle) => handle,
        Err(..) => return,
    };

    // The unsafety of the registration function derives from two aspects:
    //
    // 1. This call must be synchronized with all other registration calls as
    //    well as construction of new transports.
    // 2. The argument is leaked.
    //
    // We're clear on point (1) because this is only called at the start of this
    // binary (we know what the state of the world looks like) and we're mostly
    // clear on point (2) because we'd only free it after everything is done
    // anyway
    unsafe {
        git2_curl::register(handle);
    }
}
