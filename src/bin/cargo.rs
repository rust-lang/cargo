extern crate cargo;
extern crate url;
extern crate env_logger;
extern crate git2_curl;
extern crate rustc_serialize;
extern crate toml;
#[macro_use] extern crate log;

use std::collections::BTreeSet;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path,PathBuf};

use cargo::core::shell::Verbosity;
use cargo::execute_main_without_stdin;
use cargo::util::{self, CliResult, lev_distance, Config, human, CargoResult};
use cargo::util::CliError;

#[derive(RustcDecodable)]
pub struct Flags {
    flag_list: bool,
    flag_version: bool,
    flag_verbose: u32,
    flag_quiet: Option<bool>,
    flag_color: Option<String>,
    flag_explain: Option<String>,
    arg_command: String,
    arg_args: Vec<String>,
    flag_locked: bool,
    flag_frozen: bool,
}

const USAGE: &'static str = "
Rust's package manager

Usage:
    cargo <command> [<args>...]
    cargo [options]

Options:
    -h, --help          Display this message
    -V, --version       Print version info and exit
    --list              List installed commands
    --explain CODE      Run `rustc --explain CODE`
    -v, --verbose ...   Use verbose output
    -q, --quiet         No output printed to stdout
    --color WHEN        Coloring: auto, always, never
    --frozen            Require Cargo.lock and cache are up to date
    --locked            Require Cargo.lock is up to date

Some common cargo commands are (see all commands with --list):
    build       Compile the current project
    clean       Remove the target directory
    doc         Build this project's and its dependencies' documentation
    new         Create a new cargo project
    init        Create a new cargo project in an existing directory
    run         Build and execute src/main.rs
    test        Run the tests
    bench       Run the benchmarks
    update      Update dependencies listed in Cargo.lock
    search      Search registry for crates
    publish     Package and upload this project to the registry
    install     Install a Rust binary

See 'cargo help <command>' for more information on a specific command.
";

fn main() {
    env_logger::init().unwrap();
    execute_main_without_stdin(execute, true, USAGE)
}

macro_rules! configure_shell {
    ($config:expr, $options:expr) => (
        try!($config.configure($options.flag_verbose,
                               $options.flag_quiet,
                               &$options.flag_color,
                               $options.flag_frozen,
                               $options.flag_locked));
    )
}

macro_rules! each_subcommand{
    ($mac:ident) => {
        $mac!(bench);
        $mac!(build);
        $mac!(clean);
        $mac!(doc);
        $mac!(fetch);
        $mac!(generate_lockfile);
        $mac!(git_checkout);
        $mac!(help);
        $mac!(init);
        $mac!(install);
        $mac!(locate_project);
        $mac!(login);
        $mac!(metadata);
        $mac!(new);
        $mac!(owner);
        $mac!(package);
        $mac!(pkgid);
        $mac!(publish);
        $mac!(read_manifest);
        $mac!(run);
        $mac!(rustc);
        $mac!(rustdoc);
        $mac!(search);
        $mac!(test);
        $mac!(uninstall);
        $mac!(update);
        $mac!(verify_project);
        $mac!(version);
        $mac!(yank);
    }
}

macro_rules! declare_mod {
    ($name:ident) => ( pub mod $name; )
}
each_subcommand!(declare_mod);

/**
  The top-level `cargo` command handles configuration and project location
  because they are fundamental (and intertwined). Other commands can rely
  on this top-level information.
*/
fn execute(flags: Flags, config: &Config) -> CliResult<Option<()>> {
    try!(config.configure(flags.flag_verbose,
                          flags.flag_quiet,
                          &flags.flag_color,
                          flags.flag_frozen,
                          flags.flag_locked));

    init_git_transports(config);
    cargo::util::job::setup();

    if flags.flag_version {
        println!("{}", cargo::version());
        return Ok(None)
    }

    if flags.flag_list {
        println!("Installed Commands:");
        for command in list_commands(config) {
            println!("    {}", command);
        };
        return Ok(None)
    }

    if let Some(ref code) = flags.flag_explain {
        let mut procss = try!(config.rustc()).process();
        try!(procss.arg("--explain").arg(code).exec().map_err(human));
        return Ok(None)
    }

    let args = match &flags.arg_command[..] {
        // For the commands `cargo` and `cargo help`, re-execute ourselves as
        // `cargo -h` so we can go through the normal process of printing the
        // help message.
        "" | "help" if flags.arg_args.is_empty() => {
            config.shell().set_verbosity(Verbosity::Verbose);
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
        "help" => vec!["cargo".to_string(), flags.arg_args[0].clone(),
                       "-h".to_string()],

        // For all other invocations, we're of the form `cargo foo args...`. We
        // use the exact environment arguments to preserve tokens like `--` for
        // example.
        _ => {
            let mut default_alias = HashMap::new();
            default_alias.insert("b", "build".to_string());
            default_alias.insert("t", "test".to_string());
            default_alias.insert("r", "run".to_string());
            let mut args: Vec<String> = env::args().collect();
            if let Some(new_command) = default_alias.get(&args[1][..]){
                args[1] = new_command.clone();
            }
            args
        }
    };

    if try_execute(&config, &args) {
        return Ok(None)
    }

    let alias_list = try!(aliased_command(&config, &args[1]));
    let args = match alias_list {
        Some(alias_command) => {
            let chain = args.iter().take(1)
                .chain(alias_command.iter())
                .chain(args.iter().skip(2))
                .map(|s| s.to_string())
                .collect::<Vec<_>>();
            if try_execute(&config, &chain) {
                return Ok(None)
            } else {
                chain
            }
        }
        None => args,
    };
    try!(execute_subcommand(config, &args[1], &args));
    Ok(None)
}

fn try_execute(config: &Config, args: &[String]) -> bool {
    macro_rules! cmd {
        ($name:ident) => (if args[1] == stringify!($name).replace("_", "-") {
            config.shell().set_verbosity(Verbosity::Verbose);
            let r = cargo::call_main_without_stdin($name::execute, config,
                                                   $name::USAGE,
                                                   &args,
                                                   false);
            cargo::process_executed(r, &mut config.shell());
            return true
        })
    }
    each_subcommand!(cmd);

    return false
}

fn aliased_command(config: &Config, command: &String) -> CargoResult<Option<Vec<String>>> {
    let alias_name = format!("alias.{}", command);
    let mut result = Ok(None);
    match config.get_string(&alias_name) {
        Ok(value) => {
            if let Some(record) = value {
                let alias_commands = record.val.split_whitespace()
                                               .map(|s| s.to_string())
                                               .collect();
                result = Ok(Some(alias_commands));
            }
        },
        Err(_) => {
            let value = try!(config.get_list(&alias_name));
            if let Some(record) = value {
                let alias_commands: Vec<String> = record.val.iter()
                                .map(|s| s.0.to_string()).collect();
                result = Ok(Some(alias_commands));
            }
        }
    }
    result
}

fn find_closest(config: &Config, cmd: &str) -> Option<String> {
    let cmds = list_commands(config);
    // Only consider candidates with a lev_distance of 3 or less so we don't
    // suggest out-of-the-blue options.
    let mut filtered = cmds.iter().map(|c| (lev_distance(&c, cmd), c))
                                  .filter(|&(d, _)| d < 4)
                                  .collect::<Vec<_>>();
    filtered.sort_by(|a, b| a.0.cmp(&b.0));
    filtered.get(0).map(|slot| slot.1.clone())
}

fn execute_subcommand(config: &Config,
                      cmd: &str,
                      args: &[String]) -> CliResult<()> {
    let command_exe = format!("cargo-{}{}", cmd, env::consts::EXE_SUFFIX);
    let path = search_directories(config)
                    .iter()
                    .map(|dir| dir.join(&command_exe))
                    .find(|file| is_executable(file));
    let command = match path {
        Some(command) => command,
        None => {
            return Err(human(match find_closest(config, cmd) {
                Some(closest) => format!("no such subcommand: `{}`\n\n\t\
                                          Did you mean `{}`?\n", cmd, closest),
                None => format!("no such subcommand: `{}`", cmd)
            }).into())
        }
    };
    let err = match util::process(&command).args(&args[1..]).exec() {
        Ok(()) => return Ok(()),
        Err(e) => e,
    };

    if let Some(code) = err.exit.as_ref().and_then(|c| c.code()) {
        Err(CliError::code(code))
    } else {
        Err(CliError::new(Box::new(err), 101))
    }
}

/// List all runnable commands. find_command should always succeed
/// if given one of returned command.
fn list_commands(config: &Config) -> BTreeSet<String> {
    let prefix = "cargo-";
    let suffix = env::consts::EXE_SUFFIX;
    let mut commands = BTreeSet::new();
    for dir in search_directories(config) {
        let entries = match fs::read_dir(dir) {
            Ok(entries) => entries,
            _ => continue
        };
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            let filename = match path.file_name().and_then(|s| s.to_str()) {
                Some(filename) => filename,
                _ => continue
            };
            if !filename.starts_with(prefix) || !filename.ends_with(suffix) {
                continue
            }
            if is_executable(entry.path()) {
                let end = filename.len() - suffix.len();
                commands.insert(filename[prefix.len()..end].to_string());
            }
        }
    }

    macro_rules! add_cmd {
        ($cmd:ident) => ({ commands.insert(stringify!($cmd).replace("_", "-")); })
    }
    each_subcommand!(add_cmd);
    commands
}

#[cfg(unix)]
fn is_executable<P: AsRef<Path>>(path: P) -> bool {
    use std::os::unix::prelude::*;
    fs::metadata(path).map(|metadata| {
        metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
    }).unwrap_or(false)
}
#[cfg(windows)]
fn is_executable<P: AsRef<Path>>(path: P) -> bool {
    fs::metadata(path).map(|metadata| metadata.is_file()).unwrap_or(false)
}

fn search_directories(config: &Config) -> Vec<PathBuf> {
    let mut dirs = vec![config.home().clone().into_path_unlocked().join("bin")];
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
