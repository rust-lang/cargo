use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::{PathBuf, Path};

use cargo::util::{CliResult, Config};

#[derive(RustcDecodable)]
pub struct Options;

pub const USAGE: &'static str = "
List installed commands

Usage:
    cargo list

Options:
    -h, --help          Print this message
";

pub fn execute(_: Options, _: &Config) -> CliResult<Option<()>> {
    println!("Installed Commands:");
    for command in list_commands().into_iter() {
        println!("    {}", command);
    };
    Ok(None)
}

/// List all runnable commands. find_command should always succeed
/// if given one of returned command.
fn list_commands() -> BTreeSet<String> {
    let command_prefix = "cargo-";
    let mut commands = BTreeSet::new();
    for dir in list_command_directory().iter() {
        let entries = match fs::read_dir(dir) {
            Ok(entries) => entries,
            _ => continue
        };
        for entry in entries {
            let entry = match entry { Ok(e) => e, Err(..) => continue };
            let entry = entry.path();
            let filename = match entry.file_name().and_then(|s| s.to_str()) {
                Some(filename) => filename,
                _ => continue
            };
            if filename.starts_with(command_prefix) &&
               filename.ends_with(env::consts::EXE_SUFFIX) &&
               is_executable(&entry) {
                let command = &filename[
                    command_prefix.len()..
                    filename.len() - env::consts::EXE_SUFFIX.len()];
                commands.insert(command.to_string());
            }
        }
    }

    macro_rules! add_cmd{ ($cmd:ident) => ({
        commands.insert(stringify!($cmd).replace("_", "-"));
    }) }
    each_subcommand!(add_cmd);
    commands
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::prelude::*;
    fs::metadata(path).map(|m| {
        m.permissions().mode() & 0o001 == 0o001
    }).unwrap_or(false)
}
#[cfg(windows)]
fn is_executable(path: &Path) -> bool {
    fs::metadata(path).map(|m| m.is_file()).unwrap_or(false)
}


/// List candidate locations where subcommands might be installed.
fn list_command_directory() -> Vec<PathBuf> {
    let mut dirs = vec![];
    if let Ok(mut path) = env::current_exe() {
        path.pop();
        dirs.push(path.join("../lib/cargo"));
        dirs.push(path);
    }
    if let Some(val) = env::var_os("PATH") {
        dirs.extend(env::split_paths(&val));
    }
    dirs
}
