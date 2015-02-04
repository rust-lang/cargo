#![feature(collections, core, io, os, path)]

extern crate "rustc-serialize" as rustc_serialize;
extern crate cargo;
#[macro_use] extern crate log;
extern crate env_logger;

use std::collections::BTreeSet;
use std::os;
use std::old_io;
use std::old_io::fs::{self, PathExtensions};
use std::old_io::process::{Command,InheritFd,ExitStatus,ExitSignal};

use cargo::{execute_main_without_stdin, handle_error, shell};
use cargo::core::MultiShell;
use cargo::util::{CliError, CliResult, lev_distance, Config};

#[derive(RustcDecodable)]
struct Flags {
    flag_list: bool,
    flag_verbose: bool,
    arg_command: String,
    arg_args: Vec<String>,
}

const USAGE: &'static str = "
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
";

fn main() {
    env_logger::init().unwrap();
    execute_main_without_stdin(execute, true, USAGE)
}

macro_rules! each_subcommand{ ($mac:ident) => ({
    $mac!(bench);
    $mac!(build);
    $mac!(clean);
    $mac!(doc);
    $mac!(fetch);
    $mac!(generate_lockfile);
    $mac!(git_checkout);
    $mac!(help);
    $mac!(locate_project);
    $mac!(login);
    $mac!(new);
    $mac!(owner);
    $mac!(package);
    $mac!(pkgid);
    $mac!(publish);
    $mac!(read_manifest);
    $mac!(run);
    $mac!(search);
    $mac!(test);
    $mac!(update);
    $mac!(verify_project);
    $mac!(version);
    $mac!(yank);
}) }

/**
  The top-level `cargo` command handles configuration and project location
  because they are fundamental (and intertwined). Other commands can rely
  on this top-level information.
*/
fn execute(flags: Flags, config: &Config) -> CliResult<Option<()>> {
    debug!("executing; cmd=cargo; args={:?}", os::args());
    config.shell().set_verbose(flags.flag_verbose);

    if flags.flag_list {
        println!("Installed Commands:");
        for command in list_commands().into_iter() {
            println!("    {}", command);
        };
        return Ok(None)
    }

    let (mut args, command) = match flags.arg_command.as_slice() {
        "" | "help" if flags.arg_args.len() == 0 => {
            config.shell().set_verbose(true);
            let args = &[os::args()[0].clone(), "-h".to_string()];
            let r = cargo::call_main_without_stdin(execute, config, USAGE, args,
                                                   false);
            cargo::process_executed(r, &mut **config.shell());
            return Ok(None)
        }
        "help" if flags.arg_args[0].as_slice() == "-h" ||
                  flags.arg_args[0].as_slice() == "--help" =>
            (flags.arg_args, "help"),
        "help" => (vec!["-h".to_string()], flags.arg_args[0].as_slice()),
        s => (flags.arg_args.clone(), s),
    };
    args.insert(0, command.to_string());
    args.insert(0, os::args()[0].clone());

    macro_rules! cmd{ ($name:ident) => (
        if command.as_slice() == stringify!($name).replace("_", "-").as_slice() {
            mod $name;
            config.shell().set_verbose(true);
            let r = cargo::call_main_without_stdin($name::execute, config,
                                                   $name::USAGE,
                                                   args.as_slice(),
                                                   false);
            cargo::process_executed(r, &mut **config.shell());
            return Ok(None)
        }
    ) }
    each_subcommand!(cmd);

    execute_subcommand(command.as_slice(), args.as_slice(),
                       &mut **config.shell());
    Ok(None)
}

fn find_closest(cmd: &str) -> Option<String> {
    match list_commands().iter()
                            // doing it this way (instead of just .min_by(|c| c.lev_distance(cmd)))
                            // allows us to only make suggestions that have an edit distance of
                            // 3 or less
                            .map(|c| (lev_distance(c.as_slice(), cmd), c))
                            .filter(|&(d, _): &(usize, &String)| d < 4)
                            .min_by(|&(d, _)| d) {
        Some((_, c)) => {
            Some(c.to_string())
        },
        None => None
    }
}

fn execute_subcommand(cmd: &str, args: &[String], shell: &mut MultiShell) {
    let command = match find_command(cmd) {
        Some(command) => command,
        None => {
            let msg = match find_closest(cmd) {
                Some(closest) => format!("No such subcommand\n\n\t\
                                          Did you mean `{}`?\n", closest),
                None => "No such subcommand".to_string()
            };
            return handle_error(CliError::new(msg, 127), shell)
        }
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
            handle_error(CliError::new("", i as u32), shell)
        }
        Ok(ExitSignal(i)) => {
            let msg = format!("subcommand failed with signal: {}", i);
            handle_error(CliError::new(msg, i as u32), shell)
        }
        Err(old_io::IoError{kind, ..}) if kind == old_io::FileNotFound =>
            handle_error(CliError::new("No such subcommand", 127), shell),
        Err(err) => handle_error(
            CliError::new(
                format!("Subcommand failed to run: {}", err), 127),
            shell)
    }
}

/// List all runnable commands. find_command should always succeed
/// if given one of returned command.
fn list_commands() -> BTreeSet<String> {
    let command_prefix = "cargo-";
    let mut commands = BTreeSet::new();
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
                let command = &filename[
                    command_prefix.len()..
                    filename.len() - os::consts::EXE_SUFFIX.len()];
                commands.insert(String::from_str(command));
            }
        }
    }

    macro_rules! add_cmd{ ($cmd:ident) => ({
        commands.insert(stringify!($cmd).replace("_", "-"));
    }) }
    each_subcommand!(add_cmd);
    commands
}

fn is_executable(path: &Path) -> bool {
    match fs::stat(path) {
        Ok(old_io::FileStat{ kind: old_io::FileType::RegularFile, perm, ..}) =>
            perm.contains(old_io::OTHER_EXECUTE),
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
