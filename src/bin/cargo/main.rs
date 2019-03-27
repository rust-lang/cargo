#![warn(rust_2018_idioms)] // while we're getting used to 2018
#![allow(clippy::too_many_arguments)] // large project
#![allow(clippy::redundant_closure)] // there's a false positive
#![warn(clippy::needless_borrow)]
#![warn(clippy::redundant_clone)]

use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use cargo::core::shell::Shell;
use cargo::util::{self, command_prelude, lev_distance, CargoResult, CliResult, Config};
use cargo::util::{CliError, ProcessError};

mod cli;
mod commands;

use crate::command_prelude::*;

fn main() {
    #[cfg(feature = "pretty-env-logger")]
    pretty_env_logger::init();
    #[cfg(not(feature = "pretty-env-logger"))]
    env_logger::init();
    cargo::core::maybe_allow_nightly_features();

    let mut config = match Config::default() {
        Ok(cfg) => cfg,
        Err(e) => {
            let mut shell = Shell::new();
            cargo::exit_with_error(e.into(), &mut shell)
        }
    };

    let result = match cargo::ops::fix_maybe_exec_rustc() {
        Ok(true) => Ok(()),
        Ok(false) => {
            init_git_transports(&config);
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
        Err(_) => config
            .get_list(&alias_name)?
            .map(|record| record.val.iter().map(|s| s.0.to_string()).collect()),
    };
    let result = user_alias.or_else(|| match command {
        "b" => Some(vec!["build".to_string()]),
        "c" => Some(vec!["check".to_string()]),
        "r" => Some(vec!["run".to_string()]),
        "t" => Some(vec!["test".to_string()]),
        _ => None,
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

    commands
}

fn find_closest(config: &Config, cmd: &str) -> Option<String> {
    let cmds = list_commands(config);
    // Only consider candidates with a lev_distance of 3 or less so we don't
    // suggest out-of-the-blue options.
    cmds.into_iter()
        .map(|c| c.name())
        .map(|c| (lev_distance(&c, cmd), c))
        .filter(|&(d, _)| d < 4)
        .min_by_key(|a| a.0)
        .map(|slot| slot.1)
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
            let err = match find_closest(config, cmd) {
                Some(closest) => failure::format_err!(
                    "no such subcommand: `{}`\n\n\tDid you mean `{}`?\n",
                    cmd,
                    closest
                ),
                None => failure::format_err!("no such subcommand: `{}`", cmd),
            };
            return Err(CliError::new(err, 101));
        }
    };

    let cargo_exe = config.cargo_exe()?;
    let err = match util::process(&command)
        .env(cargo::CARGO_ENV, cargo_exe)
        .args(args)
        .exec_replace()
    {
        Ok(()) => return Ok(()),
        Err(e) => e,
    };

    if let Some(perr) = err.downcast_ref::<ProcessError>() {
        if let Some(code) = perr.exit.as_ref().and_then(|c| c.code()) {
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
    fs::metadata(path)
        .map(|metadata| metadata.is_file())
        .unwrap_or(false)
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
