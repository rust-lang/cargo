#![cfg_attr(unix, feature(fs_ext))]

extern crate cargo;
extern crate env_logger;
extern crate git2_curl;
extern crate rustc_serialize;
extern crate toml;
#[macro_use] extern crate log;

use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::io;
use std::path::{PathBuf, Path};
use std::process::Command;

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
    search      Search registry for crates

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
    $mac!(rustc);
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
    config.shell().set_verbose(flags.flag_verbose);

    init_git_transports(config);

    if flags.flag_list {
        println!("Installed Commands:");
        for command in list_commands().into_iter() {
            println!("    {}", command);
        };
        return Ok(None)
    }

    let args = match &flags.arg_command[..] {
        // For the commands `cargo` and `cargo help`, re-execute ourselves as
        // `cargo -h` so we can go through the normal process of printing the
        // help message.
        "" | "help" if flags.arg_args.is_empty() => {
            config.shell().set_verbose(true);
            let args = &["cargo".to_string(), "-h".to_string()];
            let r = cargo::call_main_without_stdin(execute, config, USAGE, args,
                                                   false);
            cargo::process_executed(r, &mut config.shell());
            return Ok(None)
        }

        // For `cargo help -h` and `cargo help --help`, print out the help
        // message for `cargo help`
        "help" if flags.arg_args[0] == "-h" ||
                  flags.arg_args[0] == "--help" => {
            vec!["cargo".to_string(), "help".to_string(), "-h".to_string()]
        }

        // For `cargo help foo`, print out the usage message for the specified
        // subcommand by executing the command with the `-h` flag.
        "help" => {
            vec!["cargo".to_string(), flags.arg_args[0].clone(),
                 "-h".to_string()]
        }

        // For all other invocations, we're of the form `cargo foo args...`. We
        // use the exact environment arguments to preserve tokens like `--` for
        // example.
        _ => env::args().collect(),
    };

    macro_rules! cmd{ ($name:ident) => (
        if args[1] == stringify!($name).replace("_", "-") {
            mod $name;
            config.shell().set_verbose(true);
            let r = cargo::call_main_without_stdin($name::execute, config,
                                                   $name::USAGE,
                                                   &args,
                                                   false);
            cargo::process_executed(r, &mut config.shell());
            return Ok(None)
        }
    ) }
    each_subcommand!(cmd);

    execute_subcommand(&args[1], &args, &mut config.shell());
    Ok(None)
}

fn find_closest(cmd: &str) -> Option<String> {
    let cmds = list_commands();
    // Only consider candidates with a lev_distance of 3 or less so we don't
    // suggest out-of-the-blue options.
    let mut filtered = cmds.iter().map(|c| (lev_distance(&c, cmd), c))
                                  .filter(|&(d, _)| d < 4)
                                  .collect::<Vec<_>>();
    filtered.sort_by(|a, b| a.0.cmp(&b.0));

    if filtered.len() == 0 {
        None
    } else {
        Some(filtered[0].1.to_string())
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
            return handle_error(CliError::new(&msg, 127), shell)
        }
    };
    match Command::new(&command).args(&args[1..]).status() {
        Ok(ref status) if status.success() => {}
        Ok(ref status) => {
            match status.code() {
                Some(code) => handle_error(CliError::new("", code), shell),
                None => {
                    let msg = format!("subcommand failed with: {}", status);
                    handle_error(CliError::new(&msg, 101), shell)
                }
            }
        }
        Err(ref e) if e.kind() == io::ErrorKind::NotFound => {
            handle_error(CliError::new("No such subcommand", 127), shell)
        }
        Err(err) => {
            let msg = format!("Subcommand failed to run: {}", err);
            handle_error(CliError::new(&msg, 127), shell)
        }
    }
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

/// Get `Command` to run given command.
fn find_command(cmd: &str) -> Option<PathBuf> {
    let command_exe = format!("cargo-{}{}", cmd, env::consts::EXE_SUFFIX);
    let dirs = list_command_directory();
    let mut command_paths = dirs.iter().map(|dir| dir.join(&command_exe));
    command_paths.find(|path| fs::metadata(&path).is_ok())
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

fn init_git_transports(config: &Config) {
    // Only use a custom transport if a proxy is configured, right now libgit2
    // doesn't support proxies and we have to use a custom transport in this
    // case. The custom transport, however, is not as well battle-tested.
    match cargo::ops::http_proxy_exists(config) {
        Ok(true) => {}
        _ => return
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
