#![warn(rust_2018_idioms)] // while we're getting used to 2018
#![allow(clippy::all)]
#![warn(clippy::disallowed_methods)]

use cargo::util::network::http::http_handle;
use cargo::util::network::http::needs_custom_http_transport;
use cargo::util::toml::StringOrVec;
use cargo::util::CliError;
use cargo::util::{self, closest_msg, command_prelude, CargoResult, CliResult, Config};
use cargo_util::{ProcessBuilder, ProcessError};
use std::collections::BTreeMap;
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

mod cli;
mod commands;

use crate::command_prelude::*;

fn main() {
    setup_logger();

    let mut config = cli::LazyConfig::new();

    let result = if let Some(lock_addr) = cargo::ops::fix_get_proxy_lock_addr() {
        cargo::ops::fix_exec_rustc(config.get(), &lock_addr).map_err(|e| CliError::from(e))
    } else {
        let _token = cargo::util::job::setup();
        cli::main(&mut config)
    };

    match result {
        Err(e) => cargo::exit_with_error(e, &mut config.get_mut().shell()),
        Ok(()) => {}
    }
}

fn setup_logger() {
    let env = tracing_subscriber::EnvFilter::from_env("CARGO_LOG");

    tracing_subscriber::fmt()
        .with_timer(tracing_subscriber::fmt::time::Uptime::default())
        .with_ansi(std::io::IsTerminal::is_terminal(&std::io::stderr()))
        .with_writer(std::io::stderr)
        .with_env_filter(env)
        .init();
    tracing::trace!(start = humantime::format_rfc3339(std::time::SystemTime::now()).to_string());
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

/// Resolve the aliased command from the [`Config`] with a given command string.
///
/// The search fallback chain is:
///
/// 1. Get the aliased command as a string.
/// 2. If an `Err` occurs (missing key, type mismatch, or any possible error),
///    try to get it as an array again.
/// 3. If still cannot find any, finds one insides [`BUILTIN_ALIASES`].
fn aliased_command(config: &Config, command: &str) -> CargoResult<Option<Vec<String>>> {
    let alias_name = format!("alias.{}", command);
    let user_alias = match config.get_string(&alias_name) {
        Ok(Some(record)) => Some(
            record
                .val
                .split_whitespace()
                .map(|s| s.to_string())
                .collect(),
        ),
        Ok(None) => None,
        Err(_) => config.get::<Option<Vec<String>>>(&alias_name)?,
    };

    let result = user_alias.or_else(|| {
        builtin_aliases_execs(command).map(|command_str| vec![command_str.1.to_string()])
    });
    Ok(result)
}

/// List all runnable commands
fn list_commands(config: &Config) -> BTreeMap<String, CommandInfo> {
    let prefix = "cargo-";
    let suffix = env::consts::EXE_SUFFIX;
    let mut commands = BTreeMap::new();
    for dir in search_directories(config) {
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
    if let Ok(aliases) = config.get::<BTreeMap<String, StringOrVec>>("alias") {
        for (name, target) in aliases.iter() {
            commands.insert(
                name.to_string(),
                CommandInfo::Alias {
                    target: target.clone(),
                },
            );
        }
    }

    // `help` is special, so it needs to be inserted separately.
    commands.insert(
        "help".to_string(),
        CommandInfo::BuiltIn {
            about: Some("Displays help for a cargo subcommand".to_string()),
        },
    );

    commands
}

fn find_external_subcommand(config: &Config, cmd: &str) -> Option<PathBuf> {
    let command_exe = format!("cargo-{}{}", cmd, env::consts::EXE_SUFFIX);
    search_directories(config)
        .iter()
        .map(|dir| dir.join(&command_exe))
        .find(|file| is_executable(file))
}

fn execute_external_subcommand(config: &Config, cmd: &str, args: &[&OsStr]) -> CliResult {
    let path = find_external_subcommand(config, cmd);
    let command = match path {
        Some(command) => command,
        None => {
            let err = if cmd.starts_with('+') {
                anyhow::format_err!(
                    "no such command: `{}`\n\n\t\
                    Cargo does not handle `+toolchain` directives.\n\t\
                    Did you mean to invoke `cargo` through `rustup` instead?",
                    cmd
                )
            } else {
                let suggestions = list_commands(config);
                let did_you_mean = closest_msg(cmd, suggestions.keys(), |c| c);

                anyhow::format_err!(
                    "no such command: `{cmd}`{did_you_mean}\n\n\t\
                    View all installed commands with `cargo --list`\n\t\
                    Find a package to install `{cmd}` with `cargo search cargo-{cmd}`",
                )
            };

            return Err(CliError::new(err, 101));
        }
    };
    execute_subcommand(config, Some(&command), args)
}

fn execute_internal_subcommand(config: &Config, args: &[&OsStr]) -> CliResult {
    execute_subcommand(config, None, args)
}

// This function is used to execute a subcommand. It is used to execute both
// internal and external subcommands.
// If `cmd_path` is `None`, then the subcommand is an internal subcommand.
fn execute_subcommand(config: &Config, cmd_path: Option<&PathBuf>, args: &[&OsStr]) -> CliResult {
    let cargo_exe = config.cargo_exe()?;
    let mut cmd = match cmd_path {
        Some(cmd_path) => ProcessBuilder::new(cmd_path),
        None => ProcessBuilder::new(&cargo_exe),
    };
    cmd.env(cargo::CARGO_ENV, cargo_exe).args(args);
    if let Some(client) = config.jobserver_from_env() {
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

fn search_directories(config: &Config) -> Vec<PathBuf> {
    let mut path_dirs = if let Some(val) = config.get_env_os("PATH") {
        env::split_paths(&val).collect()
    } else {
        vec![]
    };

    let home_bin = config.home().clone().into_path_unlocked().join("bin");

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
fn init_git(config: &Config) {
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

    init_git_transports(config);
}

/// Configure libgit2 to use libcurl if necessary.
///
/// If the user has a non-default network configuration, then libgit2 will be
/// configured to use libcurl instead of the built-in networking support so
/// that those configuration settings can be used.
fn init_git_transports(config: &Config) {
    match needs_custom_http_transport(config) {
        Ok(true) => {}
        _ => return,
    }

    let handle = match http_handle(config) {
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
