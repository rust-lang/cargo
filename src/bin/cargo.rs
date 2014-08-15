#![feature(phase)]

extern crate serialize;
#[phase(plugin, link)] extern crate log;

extern crate cargo;
extern crate docopt;
#[phase(plugin)] extern crate docopt_macros;

use std::collections::TreeSet;
use std::os;
use std::io;
use std::io::fs;
use std::io::process::{Command,InheritFd,ExitStatus,ExitSignal};
use serialize::Encodable;
use docopt::FlagParser;

use cargo::{execute_main_without_stdin, handle_error, shell};
use cargo::core::MultiShell;
use cargo::util::important_paths::find_project;
use cargo::util::{CliError, CliResult, Require, config, human};

fn main() {
    execute_main_without_stdin(execute, true)
}

docopt!(Flags, "
Rust's package manager

Usage:
    cargo <command> [<args>...]
    cargo -h | --help
    cargo -V | --version
    cargo --list

Options:
    -h, --help       Display this message
    -V, --version    Print version info and exit
    --list           List installed commands
    -v, --verbose    Use verbose output

Some common cargo commands are:
    build       Compile the current project
    clean       Remove the target directory
    doc         Build this project's and its dependencies' documentation
    new         Create a new cargo project
    run         Build and execute src/main.rs
    test        Run the tests
    bench       Run the benchmarks
    update      Update dependencies listed in Cargo.lock

See 'cargo help <command>' for more information on a specific command.
")

/**
  The top-level `cargo` command handles configuration and project location
  because they are fundamental (and intertwined). Other commands can rely
  on this top-level information.
*/
fn execute(flags: Flags, shell: &mut MultiShell) -> CliResult<Option<()>> {
    debug!("executing; cmd=cargo; args={}", os::args());
    shell.set_verbose(flags.flag_verbose);
    if flags.flag_list {
        println!("Installed Commands:");
        for command in list_commands().iter() {
            println!("    {}", command);
            // TODO: it might be helpful to add result of -h to each command.
        };
        return Ok(None)
    }
    let mut args = flags.arg_args.clone();
    args.insert(0, flags.arg_command.clone());
    match flags.arg_command.as_slice() {
        "config-for-key" => {
            log!(4, "cmd == config-for-key");
            let r = cargo::call_main_without_stdin(config_for_key, shell,
                                                   args.as_slice(), false);
            cargo::process_executed(r, shell)
        },
        "config-list" => {
            log!(4, "cmd == config-list");
            let r = cargo::call_main_without_stdin(config_list, shell,
                                                   args.as_slice(), false);
            cargo::process_executed(r, shell)
        },
        "locate-project" => {
            log!(4, "cmd == locate-project");
            let r = cargo::call_main_without_stdin(locate_project, shell,
                                                   args.as_slice(), false);
            cargo::process_executed(r, shell)
        },
        // If we have `help` with no arguments, re-invoke ourself with `-h` to
        // get the help message printed
        "help" if flags.arg_args.len() == 0 => {
            shell.set_verbose(true);
            let r = cargo::call_main_without_stdin(execute, shell,
                                                   ["-h".to_string()], false);
            cargo::process_executed(r, shell)
        },
        orig_cmd => {
            let is_help = orig_cmd == "help";
            let cmd = if is_help {
                flags.arg_args[0].as_slice()
            } else {
                orig_cmd
            };
            execute_subcommand(cmd, is_help, &flags, shell)
        }
    }
    Ok(None)
}

fn execute_subcommand(cmd: &str, is_help: bool, flags: &Flags, shell: &mut MultiShell) -> () {
    match find_command(cmd) {
        Some(command) => {
            let mut command = Command::new(command);
            let command = if is_help {
                command.arg("-h")
            } else {
                command.args(flags.arg_args.as_slice())
            };
            let status = command
                .stdin(InheritFd(0))
                .stdout(InheritFd(1))
                .stderr(InheritFd(2))
                .status();

            match status {
                Ok(ExitStatus(0)) => (),
                Ok(ExitStatus(i)) => {
                    handle_error(CliError::new("", i as uint), shell)
                }
                Ok(ExitSignal(i)) => {
                    let msg = format!("subcommand failed with signal: {}", i);
                    handle_error(CliError::new(msg, i as uint), shell)
                }
                Err(io::IoError{kind, ..}) if kind == io::FileNotFound =>
                    handle_error(CliError::new("No such subcommand", 127), shell),
                Err(err) => handle_error(
                    CliError::new(
                        format!("Subcommand failed to run: {}", err), 127),
                    shell)
            }
        },
        None => handle_error(CliError::new("No such subcommand", 127), shell)
    }
}

/// List all runnable commands. find_command should always succeed
/// if given one of returned command.
fn list_commands() -> TreeSet<String> {
    let command_prefix = "cargo-";
    let mut commands = TreeSet::new();
    for dir in list_command_directory().iter() {
        let entries = match fs::readdir(dir) {
            Ok(entries) => entries,
            _ => continue
        };
        for entry in entries.iter() {
            let filename = match entry.filename_str() {
                Some(filename) => filename,
                _ => continue
            };
            if filename.starts_with(command_prefix) &&
                    filename.ends_with(os::consts::EXE_SUFFIX) &&
                    is_executable(entry) {
                let command = filename.slice(
                    command_prefix.len(),
                    filename.len() - os::consts::EXE_SUFFIX.len());
                commands.insert(String::from_str(command));
            }
        }
    }
    commands
}

fn is_executable(path: &Path) -> bool {
    match fs::stat(path) {
        Ok(io::FileStat{kind, perm, ..}) =>
            (kind == io::TypeFile) && perm.contains(io::OtherExecute),
        _ => false
    }
}

/// Get `Command` to run given command.
fn find_command(cmd: &str) -> Option<Path> {
    let command_exe = format!("cargo-{}{}", cmd, os::consts::EXE_SUFFIX);
    let dirs = list_command_directory();
    let mut command_paths = dirs.iter().map(|dir| dir.join(command_exe.as_slice()));
    command_paths.find(|path| path.exists())
}

/// List candidate locations where subcommands might be installed.
fn list_command_directory() -> Vec<Path> {
    let mut dirs = vec![];
    match os::self_exe_path() {
        Some(path) => {
            dirs.push(path.join("../lib/cargo"));
            dirs.push(path);
        },
        None => {}
    };
    match std::os::getenv("PATH") {
        Some(val) => {
            for dir in os::split_paths(val).iter() {
                dirs.push(Path::new(dir))
            }
        },
        None => {}
    };
    dirs
}

#[deriving(Encodable)]
struct ConfigOut {
    values: std::collections::HashMap<String, config::ConfigValue>
}

docopt!(ConfigForKeyFlags, "
Usage: cargo config-for-key --human --key=<key>
")

fn config_for_key(args: ConfigForKeyFlags,
                  _: &mut MultiShell) -> CliResult<Option<ConfigOut>> {
    let value = try!(config::get_config(os::getcwd(),
                                        args.flag_key.as_slice()).map_err(|_| {
        CliError::new("Couldn't load configuration",  1)
    }));

    if args.flag_human {
        println!("{}", value);
        Ok(None)
    } else {
        let mut map = std::collections::HashMap::new();
        map.insert(args.flag_key.clone(), value);
        Ok(Some(ConfigOut { values: map }))
    }
}

docopt!(ConfigListFlags, "
Usage: cargo config-list --human
")

fn config_list(args: ConfigListFlags, _: &mut MultiShell) -> CliResult<Option<ConfigOut>> {
    let configs = try!(config::all_configs(os::getcwd()).map_err(|_|
        CliError::new("Couldn't load configuration", 1)));

    if args.flag_human {
        for (key, value) in configs.iter() {
            println!("{} = {}", key, value);
        }
        Ok(None)
    } else {
        Ok(Some(ConfigOut { values: configs }))
    }
}

docopt!(LocateProjectFlags, "
Usage: cargo locate-project
")

#[deriving(Encodable)]
struct ProjectLocation {
    root: String
}

fn locate_project(_: LocateProjectFlags,
                  _: &mut MultiShell) -> CliResult<Option<ProjectLocation>> {
    let root = try!(find_project(&os::getcwd(), "Cargo.toml").map_err(|e| {
        CliError::from_boxed(e, 1)
    }));

    let string = try!(root.as_str()
                      .require(|| human("Your project path contains characters \
                                         not representable in Unicode"))
                      .map_err(|e| CliError::from_boxed(e, 1)));

    Ok(Some(ProjectLocation { root: string.to_string() }))
}
