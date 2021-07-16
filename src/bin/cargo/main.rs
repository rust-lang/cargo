#![warn(rust_2018_idioms)] // while we're getting used to 2018
#![allow(clippy::all)]
#![warn(clippy::needless_borrow)]
#![warn(clippy::redundant_clone)]

use cargo::core::shell::Shell;
use cargo::util::CliError;
use cargo::util::{self, closest_msg, command_prelude, CargoResult, CliResult, Config};
use cargo_util::{ProcessBuilder, ProcessError};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

mod cli;
mod commands;

use crate::command_prelude::*;

fn main() {
    #[cfg(feature = "pretty-env-logger")]
    pretty_env_logger::init_custom_env("CARGO_LOG");
    #[cfg(not(feature = "pretty-env-logger"))]
    env_logger::init_from_env("CARGO_LOG");

    let mut config = match Config::default() {
        Ok(cfg) => cfg,
        Err(e) => {
            let mut shell = Shell::new();
            cargo::exit_with_error(e.into(), &mut shell)
        }
    };

    let result = match cargo::ops::fix_maybe_exec_rustc(&config) {
        Ok(true) => Ok(()),
        Ok(false) => {
            let _token = cargo::util::job::setup();
            cli::main(&mut config)
        }
        Err(e) => Err(CliError::from(e)),
    };

    match result {
        Err(e) => cargo::exit_with_error(e, &mut *config.shell()),
        Ok(()) => {}
    }
}

/// Table for defining the aliases which come builtin in `Cargo`.
/// The contents are structured as: `(alias, aliased_command, description)`.
const BUILTIN_ALIASES: [(&str, &str, &str); 5] = [
    ("b", "build", "alias: build"),
    ("c", "check", "alias: check"),
    ("d", "doc", "alias: doc"),
    ("r", "run", "alias: run"),
    ("t", "test", "alias: test"),
];

/// Function which contains the list of all of the builtin aliases and it's
/// corresponding execs represented as &str.
fn builtin_aliases_execs(cmd: &str) -> Option<&(&str, &str, &str)> {
    BUILTIN_ALIASES.iter().find(|alias| alias.0 == cmd)
}

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
fn list_commands(config: &Config) -> BTreeSet<CommandInfo> {
    let prefix = "cargo-";
    let suffix = env::consts::EXE_SUFFIX;
    let mut commands = BTreeSet::new();
    for dir in search_directories(config) {
        let entries = match fs::read_dir(dir) {
            Ok(entries) => entries,
            _ => continue,
        };
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            let filename = match path.file_name().and_then(|s| s.to_str()) {
                Some(filename) => filename,
                _ => continue,
            };
            if !filename.starts_with(prefix) || !filename.ends_with(suffix) {
                continue;
            }
            if is_executable(entry.path()) {
                let end = filename.len() - suffix.len();
                commands.insert(CommandInfo::External {
                    name: filename[prefix.len()..end].to_string(),
                    path: path.clone(),
                });
            }
        }
    }

    for cmd in commands::builtin() {
        commands.insert(CommandInfo::BuiltIn {
            name: cmd.get_name().to_string(),
            about: cmd.p.meta.about.map(|s| s.to_string()),
        });
    }

    // Add the builtin_aliases and them descriptions to the
    // `commands` `BTreeSet`.
    for command in &BUILTIN_ALIASES {
        commands.insert(CommandInfo::BuiltIn {
            name: command.0.to_string(),
            about: Some(command.2.to_string()),
        });
    }

    commands
}

/// List all runnable aliases
fn list_aliases(config: &Config) -> Vec<String> {
    match config.get::<BTreeMap<String, String>>("alias") {
        Ok(aliases) => aliases.keys().map(|a| a.to_string()).collect(),
        Err(_) => Vec::new(),
    }
}

fn execute_external_subcommand(config: &Config, cmd: &str, args: &[&str]) -> CliResult {
    let command_exe = format!("cargo-{}{}", cmd, env::consts::EXE_SUFFIX);
    let path = search_directories(config)
        .iter()
        .map(|dir| dir.join(&command_exe))
        .find(|file| is_executable(file));
    let command = match path {
        Some(command) => command,
        None => {
            let commands: Vec<String> = list_commands(config)
                .iter()
                .map(|c| c.name().to_string())
                .collect();
            let aliases = list_aliases(config);
            let suggestions = commands.iter().chain(aliases.iter());
            let did_you_mean = closest_msg(cmd, suggestions, |c| c);
            let err = anyhow::format_err!("no such subcommand: `{}`{}", cmd, did_you_mean);
            return Err(CliError::new(err, 101));
        }
    };

    let cargo_exe = config.cargo_exe()?;
    let err = match ProcessBuilder::new(&command)
        .env(cargo::CARGO_ENV, cargo_exe)
        .args(args)
        .exec_replace()
    {
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
    let mut dirs = vec![config.home().clone().into_path_unlocked().join("bin")];
    if let Some(val) = env::var_os("PATH") {
        dirs.extend(env::split_paths(&val));
    }
    dirs
}

fn init_git_transports(config: &Config) {
    // Only use a custom transport if any HTTP options are specified,
    // such as proxies or custom certificate authorities. The custom
    // transport, however, is not as well battle-tested.

    match cargo::ops::needs_custom_http_transport(config) {
        Ok(true) => {}
        _ => return,
    }

    let handle = match cargo::ops::http_handle(config) {
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
