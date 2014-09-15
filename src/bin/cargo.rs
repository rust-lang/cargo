#![feature(phase, macro_rules)]

extern crate serialize;
#[phase(plugin, link)] extern crate log;

extern crate cargo;
extern crate docopt;
#[phase(plugin)] extern crate docopt_macros;

use std::collections::TreeSet;
use std::os;
use std::io;
use std::io::fs::{mod, PathExtensions};
use std::io::process::{Command,InheritFd,ExitStatus,ExitSignal};
use docopt::FlagParser;

use cargo::{execute_main_without_stdin, handle_error, shell};
use cargo::core::MultiShell;
use cargo::util::{CliError, CliResult};

fn main() {
    execute_main_without_stdin(execute, true)
}

docopt!(Flags, "
Rust's package manager

Usage:
    cargo <command> [<args>...]
    cargo [options]

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

macro_rules! each_subcommand( ($macro:ident) => ({
    $macro!(bench)
    $macro!(build)
    $macro!(clean)
    $macro!(config_for_key)
    $macro!(config_list)
    $macro!(doc)
    $macro!(generate_lockfile)
    $macro!(git_checkout)
    $macro!(locate_project)
    $macro!(new)
    $macro!(package)
    $macro!(read_manifest)
    $macro!(run)
    $macro!(test)
    $macro!(update)
    $macro!(verify_project)
    $macro!(version)
}) )

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
        for command in list_commands().move_iter() {
            println!("    {}", command);
        };
        return Ok(None)
    }

    let (mut args, command) = match flags.arg_command.as_slice() {
        "" | "help" if flags.arg_args.len() == 0 => {
            shell.set_verbose(true);
            let r = cargo::call_main_without_stdin(execute, shell,
                                                   ["-h".to_string()], false);
            cargo::process_executed(r, shell);
            return Ok(None)
        }
        "help" => (vec!["-h".to_string()], flags.arg_args[0].as_slice()),
        s => (flags.arg_args.clone(), s),
    };
    args.insert(0, command.to_string());

    macro_rules! cmd( ($name:ident) => (
        if command.as_slice() == stringify!($name).replace("_", "-").as_slice() {
            mod $name;
            shell.set_verbose(true);
            let r = cargo::call_main_without_stdin($name::execute, shell,
                                                   args.as_slice(),
                                                   false);
            cargo::process_executed(r, shell);
            return Ok(None)
        }
    ) )
    each_subcommand!(cmd)

    execute_subcommand(command.as_slice(), args.as_slice(), shell);
    Ok(None)
}

fn execute_subcommand(cmd: &str, args: &[String], shell: &mut MultiShell) {
    let command = match find_command(cmd) {
        Some(command) => command,
        None => return handle_error(CliError::new("No such subcommand", 127),
                                    shell)
    };
    let status = Command::new(command)
                         .args(args)
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

    macro_rules! add_cmd( ($cmd:ident) => ({
        commands.insert(stringify!($cmd).replace("_", "-"));
    }) )
    each_subcommand!(add_cmd);
    commands
}

fn is_executable(path: &Path) -> bool {
    match fs::stat(path) {
        Ok(io::FileStat{ kind: io::TypeFile, perm, ..}) =>
            perm.contains(io::OtherExecute),
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
